//! Global Descriptor Table, TSS and privilege-transition stacks.

use core::arch::asm;

use crate::sync::SyncUnsafeCell;

pub const KERNEL_CODE: u16 = 0x08;
pub const KERNEL_DATA: u16 = 0x10;
pub const USER_DATA: u16 = 0x18;
pub const USER_CODE: u16 = 0x20;
pub const TSS_SELECTOR: u16 = 0x28;

pub const KERNEL_RPL: u16 = 0;
pub const USER_RPL: u16 = 3;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Tss {
    reserved0: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved1: u64,
    ist: [u64; 7],
    reserved2: u64,
    iomap_base: u16,
}

impl Tss {
    const fn new() -> Self {
        Self {
            reserved0: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved1: 0,
            ist: [0; 7],
            reserved2: 0,
            iomap_base: core::mem::size_of::<Tss>() as u16,
        }
    }
}

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

static TSS: SyncUnsafeCell<Tss> = SyncUnsafeCell::new(Tss::new());

const GDT_ENTRIES: usize = 7;
static GDT: SyncUnsafeCell<[u64; GDT_ENTRIES]> = SyncUnsafeCell::new([0; GDT_ENTRIES]);

static DOUBLE_FAULT_STACK: SyncUnsafeCell<[u8; 16 * 1024]> =
    SyncUnsafeCell::new([0; 16 * 1024]);

static BOOT_STACK: SyncUnsafeCell<[u8; 32 * 1024]> = SyncUnsafeCell::new([0; 32 * 1024]);

fn tss_descriptor(base: u64, limit: u32) -> (u64, u64) {
    let limit = limit as u64;
    let low = (limit & 0xFFFF)
        | ((base & 0xFF_FFFF) << 16)
        | (0x89u64 << 40) // present, DPL0, 64-bit available TSS (type 9)
        | (((limit >> 16) & 0xF) << 48)
        | (((base >> 24) & 0xFF) << 56);
    let high = base >> 32;
    (low, high)
}

pub fn double_fault_stack_top() -> u64 {
    DOUBLE_FAULT_STACK.get() as u64 + 16 * 1024
}

/// Update the ring-3 -> ring-0 stack used by the next privilege transition.
pub fn set_kernel_stack(top: u64) {
    unsafe {
        let tss = TSS.get();
        core::ptr::addr_of_mut!((*tss).rsp0).write_unaligned(top & !0xF);
    }
}

pub fn kernel_stack_top() -> u64 {
    BOOT_STACK.get() as u64 + 32 * 1024
}

pub unsafe fn init() {
    let gdt = GDT.get();
    (*gdt)[0] = 0;
    (*gdt)[1] = 0x00AF_9A00_0000_FFFF; // kernel code: L=1, DPL0, execute/read
    (*gdt)[2] = 0x00CF_9200_0000_FFFF; // kernel data: D/B=1, DPL0, read/write
    (*gdt)[3] = 0x00CF_F200_0000_FFFF; // user data: DPL3
    (*gdt)[4] = 0x00AF_FA00_0000_FFFF; // user code: L=1, DPL3

    let tss = TSS.get();
    let tss_address = tss as u64;
    let (low, high) = tss_descriptor(tss_address, (core::mem::size_of::<Tss>() - 1) as u32);
    (*gdt)[5] = low;
    (*gdt)[6] = high;

    let pointer = DescriptorTablePointer {
        limit: (core::mem::size_of::<[u64; GDT_ENTRIES]>() - 1) as u16,
        base: gdt as u64,
    };
    asm!("lgdt [{}]", in(reg) &pointer, options(nostack));

    // Reload CS with a far return.
    asm!(
        "push 0x08",
        "lea {tmp}, [rip + 2f]",
        "push {tmp}",
        "retfq",
        "2:",
        tmp = out(reg) _,
        options(preserves_flags)
    );
    asm!(
        "mov ds, ax",
        "mov es, ax",
        "mov ss, ax",
        "xor ax, ax",
        "mov fs, ax",
        "mov gs, ax",
        in("ax") KERNEL_DATA,
        options(nomem, nostack, preserves_flags)
    );

    // Install the TSS and the double-fault IST stack.
    (*tss).ist[0] = double_fault_stack_top();
    (*tss).rsp0 = BOOT_STACK.get() as u64 + 32 * 1024;
    asm!("ltr ax", in("ax") TSS_SELECTOR, options(nomem, nostack, preserves_flags));
}
