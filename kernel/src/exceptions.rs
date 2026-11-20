//! Exception diagnosis.
//!
//! Fatal exceptions print all relevant state and stop the machine. Page faults
//! from ring 3 are offered to the scheduler first, which can terminate the
//! offending process without taking down the kernel.

use crate::arch::cpu;
use crate::arch::idt::InterruptFrame;

fn name(vector: u64) -> &'static str {
    match vector {
        0 => "#DE divide error",
        1 => "#DB debug",
        2 => "NMI",
        3 => "#BP breakpoint",
        4 => "#OF overflow",
        5 => "#BR bound range",
        6 => "#UD invalid opcode",
        7 => "#NM device not available",
        8 => "#DF double fault",
        10 => "#TS invalid TSS",
        11 => "#NP segment not present",
        12 => "#SS stack segment fault",
        13 => "#GP general protection fault",
        14 => "#PF page fault",
        16 => "#MF x87 floating point",
        17 => "#AC alignment check",
        18 => "#MC machine check",
        19 => "#XM SIMD floating point",
        20 => "#VE virtualisation",
        21 => "#CP control protection",
        _ => "unknown exception",
    }
}

pub fn handle(frame: *mut InterruptFrame) {
    let f = unsafe { &*frame };
    let cr2 = if f.vector == 14 { cpu::read_cr2() } else { 0 };

    if f.vector == 14 && f.from_user() {
        if crate::sched::handle_user_page_fault(f, cr2) {
            return;
        }
    }

    // Any other exception raised by ring 3 belongs to the process, not to the
    // kernel: kill the offender and keep the system alive. A user `ud2`, a
    // divide error or a general-protection fault must never halt the machine.
    if f.from_user() {
        crate::error!(
            "user exception: pid {} tid {} {} at rip {:#x} (error {:#x})",
            crate::sched::current_pid(),
            crate::sched::current_tid(),
            name(f.vector),
            f.rip,
            f.error
        );
        crate::sched::exit_current(-11);
    }

    crate::error!("================================================");
    crate::error!("{} (vector {})", name(f.vector), f.vector);
    crate::error!("error code {:#x}", f.error);
    crate::error!(
        "rip={:#018x} cs={:#x} rflags={:#018x}",
        f.rip,
        f.cs,
        f.rflags
    );
    if f.from_user() {
        crate::error!("rsp={:#018x} ss={:#x}", f.rsp, f.ss);
    } else {
        crate::error!("rsp={:#018x} (kernel)", frame as u64);
    }
    if f.vector == 14 {
        crate::error!(
            "cr2={:#018x} ({})",
            cr2,
            if f.error & 1 == 0 {
                "not present"
            } else {
                "protection"
            }
        );
    }
    crate::error!(
        "rax={:#x} rbx={:#x} rcx={:#x} rdx={:#x}",
        f.rax,
        f.rbx,
        f.rcx,
        f.rdx
    );
    crate::error!("rsi={:#x} rdi={:#x} rbp={:#x}", f.rsi, f.rdi, f.rbp);
    crate::error!(
        "r8={:#x} r9={:#x} r10={:#x} r11={:#x}",
        f.r8,
        f.r9,
        f.r10,
        f.r11
    );
    crate::error!(
        "r12={:#x} r13={:#x} r14={:#x} r15={:#x}",
        f.r12,
        f.r13,
        f.r14,
        f.r15
    );
    crate::error!("================================================");
    crate::error!("system halted after fatal exception");
    loop {
        cpu::halt();
    }
}
