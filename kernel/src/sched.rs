//! Preemptive round-robin scheduler with kernel threads and ring-3 processes.
//!
//! Design constraints (documented in docs/architecture/KERNEL.md):
//!   - single CPU in this milestone,
//!   - user threads are preempted by the PIT,
//!   - kernel threads are cooperative except the idle thread,
//!   - every thread owns a kernel stack; TSS.RSP0 is updated on every switch,
//!   - context switches save/restore the full FXSAVE state.

use alloc::alloc::{alloc_zeroed, dealloc, Layout};
use core::sync::atomic::{AtomicU64, Ordering};

use crate::arch::cpu;
use crate::arch::gdt;
use crate::arch::idt::InterruptFrame;
use crate::mm::vmm;
use crate::sync::{IrqLock, SyncUnsafeCell};

pub const KERNEL_STACK_SIZE: usize = 32 * 1024;
pub const USER_STACK_SIZE: u64 = 64 * 1024;
const QUANTUM_TICKS: u32 = 10;
const FXSAVE_SIZE: usize = 512;
const FXSAVE_SLOT: usize = 520;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThreadState {
    Ready,
    Running,
    Blocked,
    Dead,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProcessState {
    Alive,
    Zombie,
}

#[repr(C)]
pub struct Process {
    pub pid: u64,
    pub parent: u64,
    pub name: [u8; 24],
    pub pml4: u64,
    pub heap_cursor: u64,
    pub fb_va: u64,
    pub exit_code: i64,
    pub state: ProcessState,
    pub threads_alive: u32,
    pub channels: alloc::vec::Vec<Option<u64>>,
    pub fds: alloc::vec::Vec<Option<crate::fs::OpenFile>>,
}

#[repr(C)]
pub struct Thread {
    pub tid: u64,
    pub pid: u64,
    pub name: [u8; 20],
    pub state: ThreadState,
    pub kernel_rsp: u64,
    pub kernel_stack_base: u64,
    pub kernel_stack_size: usize,
    pub stack_top: u64,
    pub cr3: u64,
    pub user_rip: u64,
    pub user_rsp: u64,
    pub is_user: bool,
    pub quantum_left: u32,
    pub wake_at: u64,
    pub exit_code: i64,
    pub process: *mut Process,
    pub total_ticks: u64,
}

struct Scheduler {
    threads: alloc::vec::Vec<*mut Thread>,
    processes: alloc::vec::Vec<*mut Process>,
    current: *mut Thread,
    idle: *mut Thread,
    next_tid: u64,
    next_pid: u64,
    switches: u64,
}

unsafe impl Send for Scheduler {}

static SCHED: IrqLock<Scheduler> = IrqLock::new(Scheduler {
    threads: alloc::vec::Vec::new(),
    processes: alloc::vec::Vec::new(),
    current: core::ptr::null_mut(),
    idle: core::ptr::null_mut(),
    next_tid: 1,
    next_pid: 1,
    switches: 0,
});

static CURRENT: AtomicU64 = AtomicU64::new(0);
static NEED_RESCHED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);
static STARTED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

static BOOT_RSP: SyncUnsafeCell<u64> = SyncUnsafeCell::new(0);

extern "C" {
    fn aster_context_switch(old_rsp: *mut u64, new_rsp: u64, new_cr3: u64);
}

core::arch::global_asm!(
    r#"
    .section .text
    .global aster_context_switch
aster_context_switch:
    push rbp
    push rbx
    push r12
    push r13
    push r14
    push r15
    sub rsp, 520
    lea rax, [rsp + 15]
    and rax, -16
    fxsave [rax]
    mov [rdi], rsp

    mov rsp, rsi
    mov rax, cr3
    cmp rax, rdx
    je 1f
    mov cr3, rdx
1:
    lea rax, [rsp + 15]
    and rax, -16
    fxrstor [rax]
    add rsp, 520
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbx
    pop rbp
    ret
"#
);

#[inline]
unsafe fn write_u64(address: u64, value: u64) {
    (address as *mut u64).write_unaligned(value);
}

unsafe fn allocate_stack(size: usize) -> Option<(u64, u64)> {
    let layout = Layout::from_size_align(size, 16).ok()?;
    let base = alloc_zeroed(layout);
    if base.is_null() {
        return None;
    }
    Some((base as u64, base as u64 + size as u64))
}

unsafe fn prepare_stack(stack_top: u64, entry: u64) -> u64 {
    let mut sp = stack_top;
    sp -= 8;
    write_u64(sp, entry);
    for _ in 0..6 {
        sp -= 8;
        write_u64(sp, 0);
    }
    sp -= FXSAVE_SLOT as u64;
    // Default x87/MXCSR control words so the first FXSAVE/FXRSTOR is sane.
    let fxsave = (sp + 15) & !15;
    core::ptr::write_bytes(fxsave as *mut u8, 0, FXSAVE_SIZE);
    core::ptr::write_unaligned(fxsave as *mut u16, 0x037F); // x87 control word
    core::ptr::write_unaligned((fxsave + 24) as *mut u32, 0x1F80); // MXCSR
    sp
}

unsafe fn name_bytes(name: &str) -> [u8; 20] {
    let mut out = [0u8; 20];
    for (i, byte) in name.as_bytes().iter().take(19).enumerate() {
        out[i] = *byte;
    }
    out
}

pub fn init() {
    let idle = unsafe {
        create_thread_raw(
            "idle",
            idle_entry as usize as u64,
            false,
            0,
            0,
            vmm::kernel_pml4(),
            core::ptr::null_mut(),
        )
    };
    let mut sched = SCHED.lock();
    sched.idle = idle;
    sched.current = idle;
    unsafe {
        (*idle).state = ThreadState::Running;
    }
    let (heap_mapped, _) = crate::mm::heap::stats_kib();
    crate::info!(
        "sched: idle thread {} created ({} KiB kernel heap)",
        unsafe { (*idle).tid },
        heap_mapped
    );
}

unsafe fn create_thread_raw(
    name: &str,
    entry: u64,
    is_user: bool,
    user_rip: u64,
    user_rsp: u64,
    cr3: u64,
    process: *mut Process,
) -> *mut Thread {
    let (stack_base, stack_top) =
        allocate_stack(KERNEL_STACK_SIZE).expect("cannot allocate a kernel stack");
    let rsp = prepare_stack(stack_top, entry);

    let thread = alloc::boxed::Box::into_raw(alloc::boxed::Box::new(Thread {
        tid: 0,
        pid: if process.is_null() { 0 } else { (*process).pid },
        name: name_bytes(name),
        state: ThreadState::Ready,
        kernel_rsp: rsp,
        kernel_stack_base: stack_base,
        kernel_stack_size: KERNEL_STACK_SIZE,
        stack_top,
        cr3,
        user_rip,
        user_rsp,
        is_user,
        quantum_left: QUANTUM_TICKS,
        wake_at: 0,
        exit_code: 0,
        process,
        total_ticks: 0,
    }));

    let mut sched = SCHED.lock();
    (*thread).tid = sched.next_tid;
    sched.next_tid += 1;
    sched.threads.push(thread);
    if !process.is_null() {
        (*process).threads_alive += 1;
    }
    thread
}

pub fn create_kernel_thread(name: &str, entry: extern "C" fn()) -> u64 {
    let thread = unsafe {
        create_thread_raw(
            name,
            entry as usize as u64,
            false,
            0,
            0,
            vmm::kernel_pml4(),
            core::ptr::null_mut(),
        )
    };
    unsafe { (*thread).tid }
}

/// Register a ring-3 process created by `user::spawn`.
pub unsafe fn register_process(
    name: &str,
    pml4: u64,
    entry: u64,
    stack_top: u64,
    heap_cursor: u64,
    parent: u64,
) -> u64 {
    let process = alloc::boxed::Box::into_raw(alloc::boxed::Box::new(Process {
        pid: 0,
        parent,
        name: [0; 24],
        pml4,
        heap_cursor,
        fb_va: 0,
        exit_code: 0,
        state: ProcessState::Alive,
        threads_alive: 0,
        channels: {
            let mut table = alloc::vec::Vec::new();
            table.resize(16, None);
            table
        },
        fds: {
            let mut table = alloc::vec::Vec::new();
            table.resize(24, None);
            table
        },
    }));
    for (i, byte) in name.as_bytes().iter().take(23).enumerate() {
        (*process).name[i] = *byte;
    }

    // Assign the pid and publish the process before the thread can run. The
    // boot path and syscalls both execute with interrupts disabled, so the
    // thread is not scheduled between these statements.
    let pid = {
        let mut sched = SCHED.lock();
        let pid = sched.next_pid;
        sched.next_pid += 1;
        (*process).pid = pid;
        sched.processes.push(process);
        pid
    };

    let thread = create_thread_raw(
        name,
        user_thread_entry as usize as u64,
        true,
        entry,
        stack_top,
        pml4,
        process,
    );
    let _ = thread;
    crate::info!(
        "spawn: pid {} '{}' entry {:#x} stack {:#x} cr3 {:#x} parent {}",
        pid,
        name,
        entry,
        stack_top,
        pml4,
        parent
    );
    pid
}

extern "C" fn idle_entry() {
    loop {
        cpu::enable_interrupts();
        cpu::halt();
    }
}

extern "C" fn user_thread_entry() -> ! {
    let thread = current_thread();
    if thread.is_null() {
        crate::error!("user thread without a current thread");
        cpu::halt();
    }
    unsafe {
        gdt::set_kernel_stack((*thread).stack_top);
        enter_user_mode((*thread).user_rip, (*thread).user_rsp);
    }
}

unsafe fn enter_user_mode(rip: u64, rsp: u64) -> ! {
    core::arch::asm!(
        "push {ss}",
        "push {rsp}",
        "push {rflags}",
        "push {cs}",
        "push {rip}",
        "iretq",
        ss = in(reg) (gdt::USER_DATA | gdt::USER_RPL) as u64,
        rsp = in(reg) rsp,
        rflags = in(reg) 0x202u64,
        cs = in(reg) (gdt::USER_CODE | gdt::USER_RPL) as u64,
        rip = in(reg) rip,
        options(noreturn)
    );
}

pub fn current_thread() -> *mut Thread {
    CURRENT.load(Ordering::Relaxed) as *mut Thread
}

pub fn current_process() -> *mut Process {
    let thread = current_thread();
    if thread.is_null() {
        core::ptr::null_mut()
    } else {
        unsafe { (*thread).process }
    }
}

pub fn current_tid() -> u64 {
    let thread = current_thread();
    if thread.is_null() {
        0
    } else {
        unsafe { (*thread).tid }
    }
}

pub fn current_pid() -> u64 {
    let thread = current_thread();
    if thread.is_null() {
        0
    } else {
        unsafe { (*thread).pid }
    }
}

pub fn process_count() -> usize {
    SCHED.lock().processes.len()
}

/// Find a process by pid. The returned pointer is stable while the scheduler
/// lock is not held for long; callers run with interrupts disabled and only
/// use it for a short, non-blocking operation.
pub fn process_by_pid(pid: u64) -> *mut Process {
    let sched = SCHED.lock();
    sched
        .processes
        .iter()
        .find(|&&process| unsafe { (*process).pid == pid })
        .copied()
        .unwrap_or(core::ptr::null_mut())
}

/// Block the current thread until an event wakes it. Used by IPC and WAIT.
pub fn block_current() {
    block_current_until(u64::MAX);
}

/// Block until an event or the given uptime deadline, whichever comes first.
pub fn block_current_until(wake_at: u64) {
    let thread = current_thread();
    if thread.is_null() {
        return;
    }
    unsafe {
        (*thread).state = ThreadState::Blocked;
        (*thread).wake_at = wake_at;
    }
    schedule();
}

/// Wake one specific thread (used by channel wait queues).
pub fn wake_tid(tid: u64) {
    let sched = SCHED.lock();
    for &thread in &sched.threads {
        unsafe {
            if (*thread).tid == tid && (*thread).state == ThreadState::Blocked {
                (*thread).state = ThreadState::Ready;
                NEED_RESCHED.store(true, Ordering::Relaxed);
                break;
            }
        }
    }
}

/// Runtime information for `SystemInfo`.
pub fn counts() -> (u32, u32, u64) {
    let sched = SCHED.lock();
    let threads = sched
        .threads
        .iter()
        .filter(|t| unsafe { (*(**t)).state != ThreadState::Dead })
        .count() as u32;
    let processes = sched.processes.len() as u32;
    let switches = sched.switches;
    (threads, processes, switches)
}

/// Fill a caller-provided array with a snapshot of user processes.
pub fn list_processes(out: &mut [aster_abi::ProcessInfo]) -> usize {
    let sched = SCHED.lock();
    let mut count = 0;
    for &process in &sched.processes {
        if count >= out.len() {
            break;
        }
        unsafe {
            let threads = sched
                .threads
                .iter()
                .filter(|t| {
                    let thread = **t;
                    !(*thread).process.is_null()
                        && (*(*thread).process).pid == (*process).pid
                        && (*thread).state != ThreadState::Dead
                })
                .count() as u32;
            out[count] = aster_abi::ProcessInfo {
                pid: (*process).pid,
                threads,
                state: if (*process).state == ProcessState::Zombie {
                    1
                } else {
                    0
                },
                name: (*process).name,
            };
        }
        count += 1;
    }
    count
}

fn pick_next(sched: &mut Scheduler) -> *mut Thread {
    let current = sched.current;
    let count = sched.threads.len();
    if count == 0 {
        return sched.idle;
    }
    let start_index = sched
        .threads
        .iter()
        .position(|&thread| thread == current)
        .map(|index| index + 1)
        .unwrap_or(0);
    for offset in 0..count {
        let thread = sched.threads[(start_index + offset) % count];
        if thread == current {
            continue;
        }
        if unsafe { (*thread).state } == ThreadState::Ready {
            return thread;
        }
    }
    sched.idle
}

unsafe fn reap(sched: &mut Scheduler) {
    let current = sched.current;
    let mut index = 0;
    while index < sched.threads.len() {
        let thread = sched.threads[index];
        if thread != current && (*thread).state == ThreadState::Dead {
            let layout = Layout::from_size_align((*thread).kernel_stack_size, 16).unwrap();
            dealloc((*thread).kernel_stack_base as *mut u8, layout);
            drop(alloc::boxed::Box::from_raw(thread));
            sched.threads.remove(index);
        } else {
            index += 1;
        }
    }

    // A zombie stays in the process table until its parent collects it. If the
    // parent has already exited (or the process is PID 1), it is an orphan and
    // is destroyed here.
    let mut destroy: alloc::vec::Vec<u64> = alloc::vec::Vec::new();
    for &process in &sched.processes {
        if (*process).threads_alive == 0 && (*process).state == ProcessState::Zombie {
            let parent = (*process).parent;
            let parent_alive = parent != 0
                && sched
                    .processes
                    .iter()
                    .any(|&other| (*other).pid == parent && (*other).state == ProcessState::Alive);
            if !parent_alive {
                destroy.push((*process).pid);
            }
        }
    }
    for pid in destroy {
        if let Some(index) = sched.processes.iter().position(|&p| (*p).pid == pid) {
            let process = sched.processes.remove(index);
            crate::info!("reaped process {} '{}'", pid, name_of(process));
            vmm::destroy_user_pml4((*process).pml4);
            drop(alloc::boxed::Box::from_raw(process));
        }
    }
}

/// Wake a process blocked in `WAIT`. The caller schedules immediately after.
pub fn wake_pid(pid: u64) {
    if pid == 0 {
        return;
    }
    let sched = SCHED.lock();
    let mut woke = false;
    for &thread in &sched.threads {
        unsafe {
            if (*thread).pid == pid
                && (*thread).state == ThreadState::Blocked
                && (*thread).wake_at == u64::MAX
            {
                (*thread).state = ThreadState::Ready;
                woke = true;
            }
        }
    }
    if woke {
        NEED_RESCHED.store(true, Ordering::Relaxed);
    }
}

/// Wait for a child process to exit. `target == 0` waits for any child.
/// Returns `(pid, status)`; the zombie's address space is reclaimed here.
pub fn wait_child(target: u64) -> Result<(u64, i64), i64> {
    use crate::syscall::errno;
    loop {
        let parent = current_pid();
        let mut sched = SCHED.lock();

        if let Some(index) = sched.processes.iter().position(|&process| unsafe {
            (*process).parent == parent
                && (*process).state == ProcessState::Zombie
                && (target == 0 || (*process).pid == target)
        }) {
            let process = sched.processes.remove(index);
            let (pid, status, pml4) =
                unsafe { ((*process).pid, (*process).exit_code, (*process).pml4) };
            drop(sched);
            crate::info!(
                "wait: pid {} collected by {} with status {}",
                pid,
                parent,
                status
            );
            vmm::destroy_user_pml4(pml4);
            unsafe { drop(alloc::boxed::Box::from_raw(process)) };
            return Ok((pid, status));
        }

        if target != 0
            && !sched
                .processes
                .iter()
                .any(|&process| unsafe { (*process).pid == target && (*process).parent == parent })
        {
            return Err(errno::ECHILD);
        }

        let thread = current_thread();
        if thread.is_null() {
            return Err(errno::EINVAL);
        }
        unsafe {
            (*thread).state = ThreadState::Blocked;
            // u64::MAX is the "wait for an event" sentinel: the timer must not
            // wake this thread, only wake_pid may.
            (*thread).wake_at = u64::MAX;
        }
        drop(sched);
        schedule();
    }
}

fn name_of(process: *mut Process) -> &'static str {
    unsafe {
        let name = &(*process).name;
        let len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        core::str::from_utf8(&name[..len]).unwrap_or("?")
    }
}

/// Select the next thread and switch to it. Must be called with interrupts
/// disabled; returns with interrupts still disabled.
pub fn schedule() {
    cpu::disable_interrupts();
    NEED_RESCHED.store(false, Ordering::Relaxed);

    // Wake sleepers before choosing.
    let now = crate::time::uptime_ms();

    let (old_rsp, new_rsp, new_cr3, next_top) = {
        let mut sched = unsafe { SCHED.lock_no_irq() };
        for &thread in &sched.threads {
            unsafe {
                if (*thread).state == ThreadState::Blocked && (*thread).wake_at <= now {
                    (*thread).state = ThreadState::Ready;
                }
            }
        }

        let current = sched.current;
        if !current.is_null() {
            unsafe {
                if (*current).state == ThreadState::Running && (*current).is_user {
                    (*current).state = ThreadState::Ready;
                }
                (*current).quantum_left = QUANTUM_TICKS;
            }
        }

        let next = pick_next(&mut sched);
        unsafe {
            if next.is_null() {
                return;
            }
            if next == current {
                (*next).state = ThreadState::Running;
                (*next).quantum_left = QUANTUM_TICKS;
                return;
            }
            (*next).state = ThreadState::Running;
            (*next).quantum_left = QUANTUM_TICKS;
            if (*next).is_user && (*next).process.is_null() == false {
                (*next).pid = (*next).process.as_ref().map(|p| p.pid).unwrap_or(0);
            }
            sched.current = next;
            sched.switches += 1;
            CURRENT.store(next as u64, Ordering::Relaxed);
            gdt::set_kernel_stack((*next).stack_top);
            let old = &mut (*current).kernel_rsp as *mut u64;
            let new = (*next).kernel_rsp;
            let cr3 = (*next).cr3;
            let top = (*next).stack_top;
            (*current).quantum_left = QUANTUM_TICKS;
            (old, new, cr3, top)
        }
    };

    let _ = next_top;
    unsafe {
        // Switch page tables before the stack so the kernel stack (heap VA)
        // stays valid; kernel mappings are shared across all address spaces.
        let current_cr3 = cpu::read_cr3() & vmm::ADDRESS_MASK;
        if current_cr3 != new_cr3 {
            cpu::write_cr3(new_cr3);
        }
        aster_context_switch(old_rsp, new_rsp, new_cr3);
    }

    // Reap dead threads and processes now that we are on a different stack.
    {
        let mut sched = unsafe { SCHED.lock_no_irq() };
        unsafe { reap(&mut sched) };
    }
}

pub fn start() -> ! {
    let idle = { SCHED.lock().idle };
    STARTED.store(true, Ordering::Relaxed);
    CURRENT.store(idle as u64, Ordering::Relaxed);
    unsafe {
        gdt::set_kernel_stack((*idle).stack_top);
        cpu::write_cr3((*idle).cr3);
        aster_context_switch(BOOT_RSP.get(), (*idle).kernel_rsp, (*idle).cr3);
    }
    loop {
        cpu::halt();
    }
}

pub fn yield_now() {
    let thread = current_thread();
    if thread.is_null() {
        return;
    }
    unsafe {
        if (*thread).state == ThreadState::Running {
            (*thread).state = ThreadState::Ready;
        }
    }
    schedule();
    unsafe {
        if !thread.is_null() && (*thread).state != ThreadState::Dead {
            (*thread).state = ThreadState::Running;
        }
    }
}

pub fn sleep_ms(ms: u64) {
    let thread = current_thread();
    if thread.is_null() || ms == 0 {
        yield_now();
        return;
    }
    unsafe {
        (*thread).state = ThreadState::Blocked;
        (*thread).wake_at = crate::time::uptime_ms() + ms;
    }
    schedule();
    unsafe {
        if !thread.is_null() && (*thread).state == ThreadState::Blocked {
            (*thread).state = ThreadState::Running;
        }
    }
}

pub fn exit_current(code: i64) -> ! {
    let thread = current_thread();
    if thread.is_null() {
        cpu::halt();
    }
    unsafe {
        (*thread).state = ThreadState::Dead;
        (*thread).exit_code = code;
        let process = (*thread).process;
        let mut parent = 0;
        if !process.is_null() {
            // Close every IPC handle so peers blocked on this process wake up
            // and observe a closed channel instead of waiting forever.
            for slot in (*process).channels.iter_mut() {
                if let Some(endpoint) = slot.take() {
                    crate::ipc::release_endpoint(endpoint);
                }
            }
            crate::net::close_owner((*process).pid);
            (*process).exit_code = code;
            (*process).threads_alive = (*process).threads_alive.saturating_sub(1);
            if (*process).threads_alive == 0 {
                (*process).state = ProcessState::Zombie;
                parent = (*process).parent;
            }
            crate::info!(
                "exit: pid {} tid {} status {}",
                (*process).pid,
                (*thread).tid,
                code
            );
        }
        if parent != 0 {
            wake_pid(parent);
        }
    }
    schedule();
    // A dead thread is never selected again.
    loop {
        cpu::halt();
    }
}

pub fn timer_tick(frame: *mut InterruptFrame) {
    if !STARTED.load(Ordering::Relaxed) {
        return;
    }
    let current = current_thread();
    if current.is_null() {
        return;
    }
    let now = crate::time::uptime_ms();
    {
        let sched = SCHED.lock();
        for &thread in &sched.threads {
            unsafe {
                if (*thread).state == ThreadState::Blocked && (*thread).wake_at <= now {
                    (*thread).state = ThreadState::Ready;
                }
            }
        }
    }

    unsafe {
        (*current).total_ticks += 1;
        let preemptible = (*current).is_user || current == SCHED.lock().idle;
        if preemptible {
            if (*current).quantum_left > 0 {
                (*current).quantum_left -= 1;
            }
            if (*current).quantum_left == 0 {
                NEED_RESCHED.store(true, Ordering::Relaxed);
            }
        }
        if NEED_RESCHED.load(Ordering::Relaxed) && (*frame).from_user() {
            schedule();
        } else if NEED_RESCHED.load(Ordering::Relaxed) && current == SCHED.lock().idle {
            schedule();
        }
    }
}

pub fn handle_user_page_fault(frame: &InterruptFrame, address: u64) -> bool {
    if !frame.from_user() {
        return false;
    }
    crate::error!(
        "user fault: pid {} tid {} at rip {:#x}, address {:#x} ({})",
        current_pid(),
        current_tid(),
        frame.rip,
        address,
        if frame.error & 1 == 0 {
            "not present"
        } else {
            "protection"
        }
    );
    crate::error!("terminating the offending process");
    exit_current(-11)
}
