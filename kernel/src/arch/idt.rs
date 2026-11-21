//! Interrupt Descriptor Table, exception dispatch and timer IRQ.

use core::arch::{asm, global_asm};

use crate::sync::SyncUnsafeCell;

use super::gdt;
use super::pic;

pub const VECTOR_TIMER: u64 = 32;
pub const VECTOR_KEYBOARD: u64 = 33;
pub const VECTOR_MOUSE: u64 = 44;
pub const VECTOR_APIC_TIMER: u64 = 63;
pub const VECTOR_SYSCALL: u64 = 0x80;

const VECTOR_FIRST_DYNAMIC: u64 = 48;
const VECTOR_LAST_DYNAMIC: u64 = 63;
const DYNAMIC_COUNT: usize = (VECTOR_LAST_DYNAMIC - VECTOR_FIRST_DYNAMIC) as usize;

static DYNAMIC_HANDLERS: [core::sync::atomic::AtomicUsize; DYNAMIC_COUNT] =
    [const { core::sync::atomic::AtomicUsize::new(0) }; DYNAMIC_COUNT];
static DYNAMIC_CLAIMED: core::sync::atomic::AtomicU16 = core::sync::atomic::AtomicU16::new(0);
static IRQ_COUNTS: [core::sync::atomic::AtomicU32; 64] =
    [const { core::sync::atomic::AtomicU32::new(0) }; 64];

/// Allocate a dynamic vector for a driver. Vector 63 is reserved for the APIC
/// timer, so MSI can use 48..=62. The caller must register a handler and must
/// free the vector when the device is removed.
pub fn allocate_vector() -> Option<u8> {
    use core::sync::atomic::Ordering;
    for index in 0..DYNAMIC_COUNT - 1 {
        let bit = 1u16 << index;
        loop {
            let current = DYNAMIC_CLAIMED.load(Ordering::Acquire);
            if current & bit != 0 {
                break;
            }
            if DYNAMIC_CLAIMED
                .compare_exchange(current, current | bit, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some((VECTOR_FIRST_DYNAMIC + index as u64) as u8);
            }
        }
    }
    None
}

pub fn free_vector(vector: u8) {
    use core::sync::atomic::Ordering;
    if (VECTOR_FIRST_DYNAMIC..=VECTOR_LAST_DYNAMIC).contains(&(vector as u64)) {
        let bit = 1u16 << (vector as u64 - VECTOR_FIRST_DYNAMIC);
        DYNAMIC_CLAIMED.fetch_and(!bit, Ordering::AcqRel);
        DYNAMIC_HANDLERS[(vector as u64 - VECTOR_FIRST_DYNAMIC) as usize]
            .store(0, Ordering::Release);
    }
}

pub fn register_dynamic(vector: u8, handler: fn()) -> bool {
    use core::sync::atomic::Ordering;
    if !(VECTOR_FIRST_DYNAMIC..VECTOR_LAST_DYNAMIC).contains(&(vector as u64)) {
        return false;
    }
    let slot = (vector as u64 - VECTOR_FIRST_DYNAMIC) as usize;
    DYNAMIC_HANDLERS[slot].store(handler as usize, Ordering::Release);
    true
}

pub fn irq_counts() -> [u32; 64] {
    use core::sync::atomic::Ordering;
    let mut counts = [0u32; 64];
    for (index, counter) in IRQ_COUNTS.iter().enumerate() {
        counts[index] = counter.load(Ordering::Relaxed);
    }
    counts
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct InterruptFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub vector: u64,
    pub error: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

impl InterruptFrame {
    pub fn from_user(&self) -> bool {
        self.cs & 3 == 3
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attr: u8,
    offset_mid: u16,
    offset_high: u32,
    zero: u32,
}

impl IdtEntry {
    const fn empty() -> Self {
        Self {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_mid: 0,
            offset_high: 0,
            zero: 0,
        }
    }
}

static IDT: SyncUnsafeCell<[IdtEntry; 256]> = SyncUnsafeCell::new([IdtEntry::empty(); 256]);

fn set_gate(vector: usize, handler: unsafe extern "C" fn(), dpl: u8, ist: u8) {
    let address = handler as usize as u64;
    let entry = IdtEntry {
        offset_low: address as u16,
        selector: gdt::KERNEL_CODE,
        ist,
        type_attr: 0x8E | (dpl << 5),
        offset_mid: (address >> 16) as u16,
        offset_high: (address >> 32) as u32,
        zero: 0,
    };
    unsafe {
        let idt = IDT.get();
        (*idt)[vector] = entry;
    }
}

extern "C" {
    fn isr128();
}

macro_rules! isr_table {
    ($($vector:literal => $name:ident),* $(,)?) => {
        extern "C" {
            $(fn $name();)*
        }

        fn install_handlers() {
            $(set_gate($vector, $name, 0, if $vector == 8 { 1 } else { 0 });)*
        }
    };
}

isr_table! {
    0 => isr0, 1 => isr1, 2 => isr2, 3 => isr3, 4 => isr4, 5 => isr5,
    6 => isr6, 7 => isr7, 8 => isr8, 9 => isr9, 10 => isr10, 11 => isr11,
    12 => isr12, 13 => isr13, 14 => isr14, 15 => isr15, 16 => isr16,
    17 => isr17, 18 => isr18, 19 => isr19, 20 => isr20, 21 => isr21,
    22 => isr22, 23 => isr23, 24 => isr24, 25 => isr25, 26 => isr26,
    27 => isr27, 28 => isr28, 29 => isr29, 30 => isr30, 31 => isr31,
    32 => isr32, 33 => isr33, 34 => isr34, 35 => isr35, 36 => isr36,
    37 => isr37, 38 => isr38, 39 => isr39, 40 => isr40, 41 => isr41,
    42 => isr42, 43 => isr43, 44 => isr44, 45 => isr45, 46 => isr46,
    47 => isr47, 48 => isr48, 49 => isr49, 50 => isr50, 51 => isr51,
    52 => isr52, 53 => isr53, 54 => isr54, 55 => isr55, 56 => isr56,
    57 => isr57, 58 => isr58, 59 => isr59, 60 => isr60, 61 => isr61,
    62 => isr62, 63 => isr63,
}

global_asm!(
    r#"
    .macro ISR_NOERR num
    .global isr\num
    isr\num:
        push 0
        push \num
        jmp isr_common
    .endm

    .macro ISR_ERR num
    .global isr\num
    isr\num:
        push \num
        jmp isr_common
    .endm

    .section .text
    ISR_NOERR 0
    ISR_NOERR 1
    ISR_NOERR 2
    ISR_NOERR 3
    ISR_NOERR 4
    ISR_NOERR 5
    ISR_NOERR 6
    ISR_NOERR 7
    ISR_ERR   8
    ISR_NOERR 9
    ISR_ERR   10
    ISR_ERR   11
    ISR_ERR   12
    ISR_ERR   13
    ISR_ERR   14
    ISR_NOERR 15
    ISR_NOERR 16
    ISR_ERR   17
    ISR_NOERR 18
    ISR_NOERR 19
    ISR_NOERR 20
    ISR_ERR   21
    ISR_NOERR 22
    ISR_NOERR 23
    ISR_NOERR 24
    ISR_NOERR 25
    ISR_NOERR 26
    ISR_NOERR 27
    ISR_NOERR 28
    ISR_ERR   29
    ISR_ERR   30
    ISR_NOERR 31
    ISR_NOERR 32
    ISR_NOERR 33
    ISR_NOERR 34
    ISR_NOERR 35
    ISR_NOERR 36
    ISR_NOERR 37
    ISR_NOERR 38
    ISR_NOERR 39
    ISR_NOERR 40
    ISR_NOERR 41
    ISR_NOERR 42
    ISR_NOERR 43
    ISR_NOERR 44
    ISR_NOERR 45
    ISR_NOERR 46
    ISR_NOERR 47
    ISR_NOERR 48
    ISR_NOERR 49
    ISR_NOERR 50
    ISR_NOERR 51
    ISR_NOERR 52
    ISR_NOERR 53
    ISR_NOERR 54
    ISR_NOERR 55
    ISR_NOERR 56
    ISR_NOERR 57
    ISR_NOERR 58
    ISR_NOERR 59
    ISR_NOERR 60
    ISR_NOERR 61
    ISR_NOERR 62
    ISR_NOERR 63

    .global isr128
    isr128:
        push 0
        push 128
        jmp isr_common

    isr_common:
        push rax
        push rbx
        push rcx
        push rdx
        push rsi
        push rdi
        push rbp
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15

        mov rdi, rsp
        cld
        call isr_dispatch

        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rbp
        pop rdi
        pop rsi
        pop rdx
        pop rcx
        pop rbx
        pop rax
        add rsp, 16
        iretq
"#
);

#[no_mangle]
extern "C" fn isr_dispatch(frame: *mut InterruptFrame) {
    let vector = unsafe { (*frame).vector };
    if vector < 32 {
        crate::exceptions::handle(frame);
        return;
    }
    if vector == VECTOR_SYSCALL {
        crate::syscall::handle(unsafe { &mut *frame });
        return;
    }
    if vector == VECTOR_APIC_TIMER {
        use core::sync::atomic::Ordering;
        IRQ_COUNTS[vector as usize].fetch_add(1, Ordering::Relaxed);
        crate::time::tick();
        crate::arch::apic::note_apic_timer();
        crate::arch::apic::eoi();
        crate::sched::timer_tick(frame);
        return;
    }
    if (VECTOR_FIRST_DYNAMIC..VECTOR_LAST_DYNAMIC).contains(&vector) {
        use core::sync::atomic::Ordering;
        IRQ_COUNTS[vector as usize].fetch_add(1, Ordering::Relaxed);
        let handler =
            DYNAMIC_HANDLERS[(vector - VECTOR_FIRST_DYNAMIC) as usize].load(Ordering::Acquire);
        if handler != 0 {
            let handler: fn() = unsafe { core::mem::transmute(handler) };
            handler();
        }
        if crate::arch::apic::active() {
            crate::arch::apic::eoi();
        }
        return;
    }
    match vector {
        VECTOR_TIMER => {
            crate::time::tick();
            if crate::arch::apic::active() {
                crate::arch::apic::note_vector(VECTOR_TIMER);
                crate::arch::apic::eoi();
            } else {
                unsafe { pic::eoi(0) };
            }
            crate::sched::timer_tick(frame);
        }
        VECTOR_KEYBOARD => {
            crate::drivers::ps2::handle_keyboard();
            if crate::arch::apic::active() {
                crate::arch::apic::note_vector(VECTOR_KEYBOARD);
                crate::arch::apic::eoi();
            } else {
                unsafe { pic::eoi(1) };
            }
        }
        VECTOR_MOUSE => {
            crate::drivers::ps2::handle_mouse();
            if crate::arch::apic::active() {
                crate::arch::apic::note_vector(VECTOR_MOUSE);
                crate::arch::apic::eoi();
            } else {
                unsafe { pic::eoi(12) };
            }
        }
        _ => {
            if crate::arch::apic::active() {
                if (32..48).contains(&vector) {
                    crate::arch::apic::eoi();
                }
            } else if (32..48).contains(&vector) {
                unsafe { pic::eoi((vector - 32) as u8) };
            }
        }
    }
}

pub unsafe fn init() {
    let idt = IDT.get();
    for i in 0..256 {
        (*idt)[i] = IdtEntry::empty();
    }

    install_handlers();
    set_gate(VECTOR_SYSCALL as usize, isr128, 3, 0);

    #[repr(C, packed)]
    struct Pointer {
        limit: u16,
        base: u64,
    }
    let pointer = Pointer {
        limit: (core::mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
        base: idt as u64,
    };
    asm!("lidt [{}]", in(reg) &pointer, options(nostack));
}
