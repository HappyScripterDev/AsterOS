//! System call dispatch (`int 0x80`).
//!
//! Argument registers follow the platform ABI:
//!   rax = number, rdi, rsi, rdx, r10, r8, r9 = arguments.
//! Return value: rax, negative `errno` values on failure.
//!
//! Every user pointer is validated through the process page tables before the
//! kernel reads or writes it. Syscalls run with interrupts disabled (interrupt
//! gate), so blocking calls must yield explicitly.

use aster_abi::{sys, FramebufferInfo, SystemInfo};

use crate::arch::idt::InterruptFrame;
use crate::mm::vmm;
use crate::sched;
use crate::{boot, logging, time};

fn error(code: i64) -> u64 {
    code as u64
}

pub fn handle(frame: &mut InterruptFrame) {
    let number = frame.rax;
    let args = [frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8, frame.r9];
    let process = sched::current_process();
    if process.is_null() {
        frame.rax = error(errno::EPERM);
        return;
    }
    let pml4 = unsafe { (*process).pml4 };

    let result = match number {
        sys::EXIT => {
            sched::exit_current(args[0] as i64);
        }
        sys::WRITE => write_console(pml4, args[0] as u32, args[1], args[2] as usize),
        sys::GETPID => sched::current_pid() as u64,
        sys::YIELD => {
            sched::yield_now();
            0
        }
        sys::SLEEP_MS => {
            sched::sleep_ms(args[0]);
            0
        }
        sys::TIME_MS => time::uptime_ms(),
        sys::FB_INFO => framebuffer_info(pml4, args[0]),
        sys::FB_MAP => framebuffer_map(pml4, process),
        sys::INPUT_POLL => crate::drivers::ps2::poll(args[0], args[1] as usize, pml4),
        sys::VM_ALLOC => vm_alloc(pml4, process, args[0] as usize),
        sys::LOG => write_log(pml4, args[0], args[1] as usize),
        sys::SYSTEM_INFO => system_info(pml4, args[0]),
        sys::OPEN => open_file(pml4, process, args[0], args[1] as u32),
        sys::READ => read_file(pml4, process, args[0] as u32, args[1], args[2] as usize),
        sys::WRITE_FD => write_file(pml4, process, args[0] as u32, args[1], args[2] as usize),
        sys::CLOSE => close_file(process, args[0] as u32),
        sys::SEEK => seek_file(process, args[0] as u32, args[1] as i64, args[2] as u32),
        sys::READDIR => read_directory(pml4, process, args[0] as u32, args[1], args[2] as usize),
        sys::STAT => stat_path(pml4, args[0], args[1]),
        sys::UNLINK => unlink_path(pml4, args[0]),
        sys::MKDIR => mkdir_path(pml4, args[0]),
        sys::RTC => rtc_time(pml4, args[0]),
        sys::PROC_LIST => process_list(pml4, args[0], args[1] as usize),
        sys::SHUTDOWN => shutdown(),
        _ => error(errno::ENOSYS),
    };
    frame.rax = result;
}

fn write_console(pml4: u64, fd: u32, pointer: u64, length: usize) -> u64 {
    if fd != 1 && fd != 2 {
        return error(errno::EBADF);
    }
    let length = length.min(8192);
    if length == 0 {
        return 0;
    }
    let mut buffer = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, pointer, &mut buffer).is_err() {
        return error(errno::EINVAL);
    }
    logging::write_raw(&buffer);
    length as u64
}

fn write_log(pml4: u64, pointer: u64, length: usize) -> u64 {
    let length = length.min(4096);
    if length == 0 {
        return 0;
    }
    let mut buffer = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, pointer, &mut buffer).is_err() {
        return error(errno::EINVAL);
    }
    logging::write_raw(b"[user] ");
    logging::write_raw(&buffer);
    logging::write_raw(b"\n");
    length as u64
}

fn framebuffer_info(pml4: u64, pointer: u64) -> u64 {
    let info: FramebufferInfo = boot::info().framebuffer;
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const FramebufferInfo as *const u8,
            core::mem::size_of::<FramebufferInfo>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    core::mem::size_of::<FramebufferInfo>() as u64
}

fn framebuffer_map(pml4: u64, process: *mut sched::Process) -> u64 {
    unsafe {
        if (*process).fb_va != 0 {
            return (*process).fb_va;
        }
        let info = boot::info().framebuffer;
        if info.address == 0 || info.size == 0 {
            return error(errno::EIO);
        }
        match vmm::map_framebuffer(pml4, info.address, info.size) {
            Ok(address) => {
                (*process).fb_va = address;
                address
            }
            Err(e) => {
                crate::error!("fb_map failed: {}", e);
                error(errno::ENOMEM)
            }
        }
    }
}

fn vm_alloc(pml4: u64, process: *mut sched::Process, length: usize) -> u64 {
    unsafe {
        let cursor = &mut (*process).heap_cursor;
        match vmm::user_alloc(pml4, cursor, length, vmm::PTE_WRITE | vmm::PTE_NX) {
            Ok(address) => address,
            Err(e) => {
                crate::warn!("vm_alloc({}) failed: {}", length, e);
                error(errno::ENOMEM)
            }
        }
    }
}

fn system_info(pml4: u64, pointer: u64) -> u64 {
    let (threads, processes, switches) = sched::counts();
    let (total_kib, free_kib) = crate::mm::pmm::stats();
    let (_, heap_used) = crate::mm::heap::stats_kib();
    let info = SystemInfo {
        uptime_ms: time::uptime_ms(),
        total_memory_kib: total_kib,
        free_memory_kib: free_kib,
        kernel_heap_used_kib: heap_used,
        thread_count: threads,
        process_count: processes,
        cpu_count: 1,
        timer_hz: time::TIMER_HZ,
        context_switches: switches,
        ticks: time::uptime_ticks(),
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const SystemInfo as *const u8,
            core::mem::size_of::<SystemInfo>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    core::mem::size_of::<SystemInfo>() as u64
}

pub use aster_abi::errno;

fn read_path(pml4: u64, pointer: u64) -> Result<alloc::string::String, i64> {
    if pointer == 0 {
        return Err(errno::EINVAL);
    }
    let mut buffer = [0u8; 256];
    if vmm::copy_from_user(pml4, pointer, &mut buffer).is_err() {
        return Err(errno::EINVAL);
    }
    let length = buffer
        .iter()
        .position(|&byte| byte == 0)
        .ok_or(errno::ENAMETOOLONG)?;
    alloc::string::String::from_utf8(buffer[..length].to_vec()).map_err(|_| errno::EINVAL)
}

unsafe fn fd_slot(process: *mut sched::Process, fd: u32) -> Option<&'static mut crate::fs::OpenFile> {
    let fds = &mut (*process).fds;
    fds.get_mut(fd as usize).and_then(|slot| slot.as_mut())
}

fn open_file(pml4: u64, process: *mut sched::Process, path_pointer: u64, flags: u32) -> u64 {
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    let inode = match crate::fs::open(&path, flags) {
        Ok(inode) => inode,
        Err(code) => return error(code),
    };
    unsafe {
        let fds = &mut (*process).fds;
        for index in 3..crate::fs::MAX_FDS {
            if fds[index].is_none() {
                fds[index] = Some(crate::fs::OpenFile {
                    inode,
                    offset: 0,
                    flags,
                    dir_index: 0,
                });
                return index as u64;
            }
        }
    }
    error(errno::EBADF)
}

fn read_file(pml4: u64, process: *mut sched::Process, fd: u32, pointer: u64, length: usize) -> u64 {
    let file = unsafe {
        match fd_slot(process, fd) {
            Some(file) => file,
            None => return error(errno::EBADF),
        }
    };
    let length = length.min(1024 * 1024);
    let data = match crate::fs::read(file.inode, file.offset, length) {
        Ok(data) => data,
        Err(code) => return error(code),
    };
    if vmm::copy_to_user(pml4, pointer, &data).is_err() {
        return error(errno::EINVAL);
    }
    file.offset += data.len() as u64;
    data.len() as u64
}

fn write_file(pml4: u64, process: *mut sched::Process, fd: u32, pointer: u64, length: usize) -> u64 {
    let file = unsafe {
        match fd_slot(process, fd) {
            Some(file) => file,
            None => return error(errno::EBADF),
        }
    };
    if file.flags & aster_abi::open_flags::WRITE == 0 {
        return error(errno::EPERM);
    }
    let length = length.min(1024 * 1024);
    let mut buffer = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, pointer, &mut buffer).is_err() {
        return error(errno::EINVAL);
    }
    match crate::fs::write(file.inode, file.offset, &buffer) {
        Ok(written) => {
            file.offset += written as u64;
            written as u64
        }
        Err(code) => error(code),
    }
}

fn close_file(process: *mut sched::Process, fd: u32) -> u64 {
    if fd < 3 {
        return 0;
    }
    unsafe {
        let fds: &mut alloc::vec::Vec<Option<crate::fs::OpenFile>> = &mut (*process).fds;
        if let Some(slot) = fds.get_mut(fd as usize) {
            *slot = None;
            return 0;
        }
    }
    error(errno::EBADF)
}

fn seek_file(process: *mut sched::Process, fd: u32, offset: i64, whence: u32) -> u64 {
    let file = unsafe {
        match fd_slot(process, fd) {
            Some(file) => file,
            None => return error(errno::EBADF),
        }
    };
    let base = match whence {
        0 => 0i64,
        1 => file.offset as i64,
        2 => crate::fs::size(file.inode).unwrap_or(0) as i64,
        _ => return error(errno::EINVAL),
    };
    let new_offset = base + offset;
    if new_offset < 0 {
        return error(errno::EINVAL);
    }
    file.offset = new_offset as u64;
    file.offset
}

fn read_directory(
    pml4: u64,
    process: *mut sched::Process,
    fd: u32,
    pointer: u64,
    maximum: usize,
) -> u64 {
    let file = unsafe {
        match fd_slot(process, fd) {
            Some(file) => file,
            None => return error(errno::EBADF),
        }
    };
    let maximum = maximum.min(64);
    let items = match crate::fs::list(file.inode, file.dir_index, maximum) {
        Ok(items) => items,
        Err(code) => return error(code),
    };
    let entry_size = core::mem::size_of::<aster_abi::DirEntry>();
    let mut buffer = alloc::vec![0u8; items.len() * entry_size];
    for (index, item) in items.iter().enumerate() {
        let mut entry = aster_abi::DirEntry {
            inode: item.inode,
            ty: if item.kind == crate::fs::NodeKind::Dir { 1 } else { 0 },
            name_len: item.name.len() as u32,
            name: [0; aster_abi::DIRENT_NAME_LEN],
        };
        for (i, byte) in item.name.as_bytes().iter().take(aster_abi::DIRENT_NAME_LEN).enumerate() {
            entry.name[i] = *byte;
        }
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &entry as *const aster_abi::DirEntry as *const u8,
                entry_size,
            )
        };
        buffer[index * entry_size..(index + 1) * entry_size].copy_from_slice(bytes);
    }
    if vmm::copy_to_user(pml4, pointer, &buffer).is_err() {
        return error(errno::EINVAL);
    }
    file.dir_index += items.len();
    items.len() as u64
}

fn stat_path(pml4: u64, path_pointer: u64, stat_pointer: u64) -> u64 {
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    let stat = match crate::fs::stat_path(&path) {
        Ok(stat) => stat,
        Err(code) => return error(code),
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &stat as *const aster_abi::FileStat as *const u8,
            core::mem::size_of::<aster_abi::FileStat>(),
        )
    };
    if vmm::copy_to_user(pml4, stat_pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    0
}

fn unlink_path(pml4: u64, path_pointer: u64) -> u64 {
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    match crate::fs::unlink(&path) {
        Ok(()) => 0,
        Err(code) => error(code),
    }
}

fn mkdir_path(pml4: u64, path_pointer: u64) -> u64 {
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    match crate::fs::mkdir(&path) {
        Ok(_) => 0,
        Err(code) => error(code),
    }
}

fn rtc_time(pml4: u64, pointer: u64) -> u64 {
    let time = crate::arch::rtc::read();
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &time as *const aster_abi::RtcTime as *const u8,
            core::mem::size_of::<aster_abi::RtcTime>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    0
}

fn process_list(pml4: u64, pointer: u64, maximum: usize) -> u64 {
    let maximum = maximum.min(64);
    let mut entries = alloc::vec![aster_abi::ProcessInfo {
        pid: 0,
        threads: 0,
        state: 0,
        name: [0; 24],
    }; maximum];
    let count = sched::list_processes(&mut entries);
    let bytes = unsafe {
        core::slice::from_raw_parts(
            entries.as_ptr() as *const u8,
            count * core::mem::size_of::<aster_abi::ProcessInfo>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    count as u64
}

fn shutdown() -> u64 {    unsafe {
        // QEMU ACPI poweroff (q35) and the older i440fx mechanism.
        core::arch::asm!("out dx, ax", in("dx") 0x604u16, in("ax") 0x2000u16, options(nomem, nostack));
        core::arch::asm!("out dx, ax", in("dx") 0xB004u16, in("ax") 0x2000u16, options(nomem, nostack));
    }
    crate::info!("shutdown requested by user space");
    crate::arch::cpu::halt();
    0
}
