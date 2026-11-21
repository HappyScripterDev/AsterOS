//! ACPI fixed-table discovery and power management.
//!
//! Only fixed tables are parsed. There is no AML interpreter. Every table is
//! bounds-checked against its declared length and its checksum verified before
//! any field is trusted. Tables are read through the bootloader direct map
//! during early boot and the fields we need are copied into `ACPI_STATE`.

use crate::boot;
use crate::sync::IrqLock;

const MAX_TABLE_LEN: u32 = 1024 * 1024;
const MAX_IOAPICS: usize = 4;
const MAX_ISOS: usize = 16;

#[derive(Clone, Copy)]
pub struct IoApic {
    pub id: u8,
    pub address: u32,
    pub gsi_base: u32,
}

#[derive(Clone, Copy)]
pub struct Iso {
    pub irq: u8,
    pub gsi: u32,
    pub flags: u16,
}

#[derive(Clone, Copy)]
pub struct AcpiState {
    pub present: bool,
    pub revision: u8,
    pub table_count: u32,
    pub xsdt: u64,
    pub fadt: u64,
    pub dsdt: u64,
    pub pm1a_control: u16,
    pub pm1b_control: u16,
    pub pm1_control_len: u8,
    pub slp_typ_a: u8,
    pub slp_typ_b: u8,
    pub reset_space: u8,
    pub reset_address: u64,
    pub reset_value: u8,
    pub sci_irq: u16,
    pub lapic_base: u64,
    pub cpu_count: u32,
    pub ioapics: [IoApic; MAX_IOAPICS],
    pub ioapic_count: usize,
    pub isos: [Iso; MAX_ISOS],
    pub iso_count: usize,
    pub mcfg_base: u64,
    pub mcfg_segment: u16,
    pub pci_start_bus: u8,
    pub pci_end_bus: u8,
    pub hpet_base: u64,
    pub hpet_period_fs: u32,
    pub hpet_comparators: u8,
    pub ssdt_count: u32,
}

const EMPTY_IOAPIC: IoApic = IoApic {
    id: 0,
    address: 0,
    gsi_base: 0,
};
const EMPTY_ISO: Iso = Iso {
    irq: 0,
    gsi: 0,
    flags: 0,
};

const EMPTY: AcpiState = AcpiState {
    present: false,
    revision: 0,
    table_count: 0,
    xsdt: 0,
    fadt: 0,
    dsdt: 0,
    pm1a_control: 0,
    pm1b_control: 0,
    pm1_control_len: 0,
    slp_typ_a: 0,
    slp_typ_b: 0,
    reset_space: 0,
    reset_address: 0,
    reset_value: 0,
    sci_irq: 0,
    lapic_base: 0,
    cpu_count: 0,
    ioapics: [EMPTY_IOAPIC; MAX_IOAPICS],
    ioapic_count: 0,
    isos: [EMPTY_ISO; MAX_ISOS],
    iso_count: 0,
    mcfg_base: 0,
    mcfg_segment: 0,
    pci_start_bus: 0,
    pci_end_bus: 0,
    hpet_base: 0,
    hpet_period_fs: 0,
    hpet_comparators: 0,
    ssdt_count: 0,
};

static ACPI_STATE: IrqLock<AcpiState> = IrqLock::new(EMPTY);

pub fn state() -> AcpiState {
    *ACPI_STATE.lock()
}

pub fn present() -> bool {
    ACPI_STATE.lock().present
}

pub fn local_apic_base() -> Option<u64> {
    let state = state();
    if state.lapic_base != 0 {
        Some(state.lapic_base)
    } else {
        None
    }
}

pub fn ioapic_entries() -> ([IoApic; MAX_IOAPICS], usize) {
    let state = state();
    (state.ioapics, state.ioapic_count)
}

/// Resolve an ISA IRQ to its GSI, honouring MADT interrupt source overrides.
pub fn gsi_for_irq(irq: u8) -> (u32, u16) {
    let state = state();
    for index in 0..state.iso_count {
        let iso = state.isos[index];
        if iso.irq == irq {
            return (iso.gsi, iso.flags);
        }
    }
    (irq as u32, 0)
}

pub fn mcfg() -> Option<(u64, u16, u8, u8)> {
    let state = state();
    if state.mcfg_base != 0 {
        Some((
            state.mcfg_base,
            state.mcfg_segment,
            state.pci_start_bus,
            state.pci_end_bus,
        ))
    } else {
        None
    }
}

pub fn hpet() -> Option<(u64, u32, u8)> {
    let state = state();
    if state.hpet_base != 0 {
        Some((
            state.hpet_base,
            state.hpet_period_fs,
            state.hpet_comparators,
        ))
    } else {
        None
    }
}

fn physical(physical: u64) -> u64 {
    boot::physical_to_virtual(physical)
}

unsafe fn read_u8(address: u64) -> u8 {
    (address as *const u8).read_volatile()
}

unsafe fn read_u16(address: u64) -> u16 {
    ((address as *const u16).read_unaligned()).to_le()
}

unsafe fn read_u32(address: u64) -> u32 {
    ((address as *const u32).read_unaligned()).to_le()
}

unsafe fn read_u64(address: u64) -> u64 {
    ((address as *const u64).read_unaligned()).to_le()
}

fn signature(address: u64, expected: &[u8]) -> bool {
    if address == 0 {
        return false;
    }
    let signature = unsafe { core::slice::from_raw_parts(address as *const u8, expected.len()) };
    signature == expected
}

fn checksum(address: u64, length: usize) -> bool {
    if length == 0 || length > MAX_TABLE_LEN as usize || address == 0 {
        return false;
    }
    let mut sum = 0u8;
    for offset in 0..length {
        sum = sum.wrapping_add(unsafe { read_u8(address + offset as u64) });
    }
    sum == 0
}

/// Validate an ACPI system description table and return its length.
fn table_length(address: u64, expected: &[u8; 4]) -> Option<usize> {
    if !signature(address, expected) {
        return None;
    }
    let length = unsafe { read_u32(address + 4) } as usize;
    if length < 36 || length > MAX_TABLE_LEN as usize {
        return None;
    }
    if !checksum(address, length) {
        return None;
    }
    Some(length)
}

fn find_rsdp() -> Option<u64> {
    let from_boot = boot::info().rsdp_address;
    if from_boot != 0 {
        let address = physical(from_boot);
        if signature(address, b"RSD PTR ") && checksum(address, 20) {
            return Some(address);
        }
    }
    // Firmware sometimes omits the pointer: scan the EBDA and BIOS ROM area.
    for base in [0x000E_0000u64, 0x000F_0000u64] {
        let mut offset = 0u64;
        while offset < 0x2_0000 {
            let address = physical(base + offset);
            if signature(address, b"RSD PTR ") && checksum(address, 20) {
                return Some(address);
            }
            offset += 16;
        }
    }
    None
}

fn parse_fadt(address: u64, length: usize, state: &mut AcpiState) {
    if length < 116 {
        crate::warn!("acpi: FADT too short ({} bytes)", length);
        return;
    }
    state.fadt = address;
    if length >= 44 {
        let dsdt = unsafe { read_u32(address + 40) } as u64;
        if dsdt != 0 {
            state.dsdt = dsdt;
        }
    }
    state.sci_irq = unsafe { read_u16(address + 46) };
    state.pm1a_control = unsafe { read_u16(address + 64) };
    state.pm1b_control = unsafe { read_u16(address + 66) };
    if length >= 90 {
        state.pm1_control_len = unsafe { read_u8(address + 89) };
    }
    if state.pm1_control_len == 0 {
        state.pm1_control_len = 2;
    }
    if length >= 140 {
        state.slp_typ_a = unsafe { read_u8(address + 138) };
        state.slp_typ_b = unsafe { read_u8(address + 139) };
    }
    if length >= 129 {
        state.reset_space = unsafe { read_u8(address + 116) };
        // Generic Address Structure: register address at +4.
        state.reset_address = unsafe { read_u64(address + 120) };
        state.reset_value = unsafe { read_u8(address + 128) };
    }
    if length >= 148 {
        let x_dsdt = unsafe { read_u64(address + 140) };
        if x_dsdt != 0 {
            state.dsdt = x_dsdt;
        }
    }
    crate::info!(
        "acpi: FADT {} bytes, pm1a={:#x} pm1b={:#x} len={} slp_a={} slp_b={} reset={}:{:#x}={:#x} sci={}",
        length,
        state.pm1a_control,
        state.pm1b_control,
        state.pm1_control_len,
        state.slp_typ_a,
        state.slp_typ_b,
        state.reset_space,
        state.reset_address,
        state.reset_value,
        state.sci_irq
    );
}

fn parse_madt(address: u64, length: usize, state: &mut AcpiState) {
    state.lapic_base = unsafe { read_u32(address + 36) } as u64;
    let mut offset = 44usize;
    while offset + 2 <= length {
        let entry_type = unsafe { read_u8(address + offset as u64) };
        let entry_len = unsafe { read_u8(address + offset as u64 + 1) } as usize;
        if entry_len < 2 || offset + entry_len > length {
            crate::warn!(
                "acpi: MADT entry {} has invalid length {}",
                entry_type,
                entry_len
            );
            break;
        }
        let entry = address + offset as u64;
        match entry_type {
            0 if entry_len >= 8 => {
                let flags = unsafe { read_u32(entry + 4) };
                if flags & 1 != 0 {
                    state.cpu_count += 1;
                }
            }
            1 if entry_len >= 12 => {
                if state.ioapic_count < MAX_IOAPICS {
                    state.ioapics[state.ioapic_count] = IoApic {
                        id: unsafe { read_u8(entry + 2) },
                        address: unsafe { read_u32(entry + 4) },
                        gsi_base: unsafe { read_u32(entry + 8) },
                    };
                    state.ioapic_count += 1;
                }
            }
            2 if entry_len >= 10 => {
                if state.iso_count < MAX_ISOS {
                    state.isos[state.iso_count] = Iso {
                        irq: unsafe { read_u8(entry + 3) },
                        gsi: unsafe { read_u32(entry + 4) },
                        flags: unsafe { read_u16(entry + 8) },
                    };
                    state.iso_count += 1;
                }
            }
            9 if entry_len >= 16 => {
                // x2APIC entries also describe enabled CPUs; the LAPIC base is
                // the same MMIO window for this single-CPU release.
                let flags = unsafe { read_u32(entry + 8) };
                if flags & 1 != 0 {
                    state.cpu_count += 1;
                }
            }
            _ => {}
        }
        offset += entry_len;
    }
    crate::info!(
        "acpi: MADT {} bytes, lapic={:#x}, cpus={}, ioapics={}, overrides={}",
        length,
        state.lapic_base,
        state.cpu_count,
        state.ioapic_count,
        state.iso_count
    );
    for index in 0..state.ioapic_count {
        let ioapic = state.ioapics[index];
        crate::info!(
            "acpi: IOAPIC id {} at {:#x}, gsi base {}",
            ioapic.id,
            ioapic.address,
            ioapic.gsi_base
        );
    }
    for index in 0..state.iso_count {
        let iso = state.isos[index];
        crate::info!(
            "acpi: ISO irq {} -> gsi {}, flags {:#x}",
            iso.irq,
            iso.gsi,
            iso.flags
        );
    }
}

fn parse_mcfg(address: u64, length: usize, state: &mut AcpiState) {
    if length < 44 {
        crate::warn!("acpi: MCFG too short");
        return;
    }
    // Header 36 bytes, 8 reserved, then the first allocation entry at 44.
    state.mcfg_base = unsafe { read_u64(address + 44) };
    state.mcfg_segment = unsafe { read_u16(address + 52) };
    state.pci_start_bus = unsafe { read_u8(address + 54) };
    state.pci_end_bus = unsafe { read_u8(address + 55) };
    crate::info!(
        "acpi: MCFG {} bytes, base {:#x}, segment {}, buses {}..={}",
        length,
        state.mcfg_base,
        state.mcfg_segment,
        state.pci_start_bus,
        state.pci_end_bus
    );
}

fn parse_hpet(address: u64, length: usize, state: &mut AcpiState) {
    if length < 56 {
        crate::warn!("acpi: HPET too short");
        return;
    }
    let block_id = unsafe { read_u32(address + 36) };
    state.hpet_comparators = ((block_id >> 8) & 0x1F) as u8 + 1;
    state.hpet_base = unsafe { read_u64(address + 44) };
    // The timer period lives in the HPET capabilities register (MMIO), not in
    // this table. It is read during APIC-timer calibration in phase 2.
    state.hpet_period_fs = 0;
    let hpet_number = unsafe { read_u8(address + 52) };
    crate::info!(
        "acpi: HPET {} bytes, base {:#x}, comparators {}, number {} (period read from MMIO later)",
        length,
        state.hpet_base,
        state.hpet_comparators,
        hpet_number
    );
}

pub fn init() {
    let Some(rsdp) = find_rsdp() else {
        crate::warn!("acpi: no valid RSDP found; PIC/PIT fallback remains active");
        return;
    };
    let revision = unsafe { read_u8(rsdp + 15) };
    if revision >= 2 && !checksum(rsdp, 36) {
        crate::warn!("acpi: RSDP extended checksum invalid; ignoring XSDT");
    }
    let use_xsdt = revision >= 2 && checksum(rsdp, 36);
    let root = if use_xsdt {
        unsafe { read_u64(rsdp + 24) }
    } else {
        (unsafe { read_u32(rsdp + 16) }) as u64
    };
    if root == 0 {
        crate::warn!("acpi: RSDP has no root table");
        return;
    }
    let root = physical(root);
    let root_signature = if use_xsdt { b"XSDT" } else { b"RSDT" };
    let Some(root_length) = table_length(root, root_signature) else {
        crate::warn!(
            "acpi: root table {} failed validation",
            if use_xsdt { "XSDT" } else { "RSDT" }
        );
        return;
    };

    let mut state = EMPTY;
    state.present = true;
    state.revision = revision;
    state.xsdt = if use_xsdt { root } else { 0 };
    crate::info!(
        "acpi: RSDP rev {} at {:#x}, {} {} bytes",
        revision,
        rsdp,
        if use_xsdt { "XSDT" } else { "RSDT" },
        root_length
    );

    let entry_size = if use_xsdt { 8 } else { 4 };
    let count = (root_length - 36) / entry_size;
    for index in 0..count {
        let entry = root + 36 + (index * entry_size) as u64;
        let address = if use_xsdt {
            unsafe { read_u64(entry) }
        } else {
            (unsafe { read_u32(entry) }) as u64
        };
        if address == 0 {
            continue;
        }
        let table = physical(address);
        // Every table must at least have a valid SDT header/checksum.
        let header_len = unsafe { read_u32(table + 4) } as usize;
        if header_len < 36 || header_len > MAX_TABLE_LEN as usize || !checksum(table, header_len) {
            crate::warn!("acpi: table {} at {:#x} failed validation", index, table);
            continue;
        }
        state.table_count += 1;
        if signature(table, b"FACP") {
            parse_fadt(table, header_len, &mut state);
        } else if signature(table, b"APIC") {
            parse_madt(table, header_len, &mut state);
        } else if signature(table, b"MCFG") {
            parse_mcfg(table, header_len, &mut state);
        } else if signature(table, b"HPET") {
            parse_hpet(table, header_len, &mut state);
        } else if signature(table, b"SSDT") {
            state.ssdt_count += 1;
        } else if signature(table, b"DSDT") {
            if state.dsdt == 0 {
                state.dsdt = table;
            }
        }
    }

    if state.ioapic_count == 0 {
        crate::warn!("acpi: MADT has no IOAPIC; PCI interrupt routing remains unavailable");
    }
    if state.cpu_count == 0 && state.lapic_base != 0 {
        state.cpu_count = 1;
    }
    if state.dsdt != 0 {
        crate::info!(
            "acpi: DSDT at {:#x}, {} SSDT(s) present; AML is not parsed",
            state.dsdt,
            state.ssdt_count
        );
    }
    crate::info!(
        "acpi: parsed {} tables, {} CPU(s)",
        state.table_count,
        state.cpu_count
    );
    *ACPI_STATE.lock() = state;
}

fn outb(port: u16, value: u8) {
    unsafe {
        core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack));
    }
}

fn outw(port: u16, value: u16) {
    unsafe {
        core::arch::asm!("out dx, ax", in("dx") port, in("ax") value, options(nomem, nostack));
    }
}

fn outl(port: u16, value: u32) {
    unsafe {
        core::arch::asm!("out dx, eax", in("dx") port, in("eax") value, options(nomem, nostack));
    }
}

/// Write the FADT reset register. Only system I/O and system memory spaces are
/// supported; other spaces fall through to the legacy reset paths.
fn reset_register(state: AcpiState) {
    if state.reset_address == 0 {
        return;
    }
    match state.reset_space {
        1 => outb(state.reset_address as u16, state.reset_value),
        0 => {
            let address = physical(state.reset_address);
            unsafe {
                (address as *mut u8).write_volatile(state.reset_value);
            }
        }
        _ => crate::warn!(
            "acpi: unsupported reset address space {}",
            state.reset_space
        ),
    }
}

pub fn reset() -> ! {
    let state = state();
    crate::info!(
        "acpi: reset requested (space {} address {:#x})",
        state.reset_space,
        state.reset_address
    );
    reset_register(state);
    // Legacy chipset and keyboard-controller reset pulses.
    outb(0xCF9, 0x02);
    outb(0xCF9, 0x06);
    outb(0x64, 0xFE);
    loop {
        crate::arch::cpu::halt();
    }
}

pub fn shutdown() -> ! {
    let state = state();
    crate::info!(
        "acpi: S5 shutdown (pm1a={:#x} slp_a={} pm1b={:#x} slp_b={})",
        state.pm1a_control,
        state.slp_typ_a,
        state.pm1b_control,
        state.slp_typ_b
    );
    if state.pm1a_control != 0 {
        let value = ((state.slp_typ_a as u16) << 10) | 0x2000;
        outw(state.pm1a_control, value);
        if state.pm1b_control != 0 {
            let value_b = ((state.slp_typ_b as u16) << 10) | 0x2000;
            outw(state.pm1b_control, value_b);
        }
    }
    // QEMU q35 and older i440fx ACPI shutdown ports as a last resort.
    outw(0x604, 0x2000);
    outw(0xB004, 0x2000);
    // If firmware did not enter S5, stop instead of pretending to be off.
    crate::error!("acpi: S5 did not power off the machine");
    loop {
        crate::arch::cpu::halt();
    }
}

/// Called by the POWER syscall; keeps the ABI mode mapping in one place.
pub fn power(mode: u64) -> ! {
    match mode {
        0 => shutdown(),
        1 => reset(),
        _ => {
            crate::warn!("acpi: invalid POWER mode {}", mode);
            loop {
                crate::arch::cpu::halt();
            }
        }
    }
}
