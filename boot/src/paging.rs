//! Page-table construction for the higher-half kernel.
//!
//! Layout (documented in docs/decisions/0004-address-space-layout.md):
//!   0x0000_0000_0000_0000 .. 0x0000_7FFF_FFFF_FFFF  user space
//!   0xFFFF_8000_0000_0000 .. 0xFFFF_FFFF_7FFF_FFFF  direct physical map
//!   0xFFFF_FFFF_8000_0000 .. 0xFFFF_FFFF_FFFF_FFFF  kernel image
//!
//! The loader builds the tables, maps the kernel image RX/RW as appropriate,
//! maps all physical memory NX (except the trampoline), then switches CR3 and
//! jumps to the kernel.

use crate::efi::{allocate_pages, BootServices, Status};

pub const HHDM_OFFSET: u64 = 0xFFFF_8000_0000_0000;
pub const KERNEL_BASE: u64 = 0xFFFF_FFFF_8000_0000;

pub const PTE_PRESENT: u64 = 1 << 0;
pub const PTE_WRITE: u64 = 1 << 1;
pub const PTE_USER: u64 = 1 << 2;
pub const PTE_GLOBAL: u64 = 1 << 8;
pub const PTE_HUGE: u64 = 1 << 7;
pub const PTE_NX: u64 = 1 << 63;

const ADDRESS_MASK: u64 = 0x000F_FFFF_FFFF_F000;

pub struct PageTables {
    pub pml4: u64,
    bs: *mut BootServices,
}

impl PageTables {
    pub unsafe fn new(bs: *mut BootServices) -> Result<Self, &'static str> {
        let pml4 = allocate_pages(bs, 1).map_err(|_| "cannot allocate PML4")?;
        Ok(Self { pml4, bs })
    }

    unsafe fn table(&self) -> *mut u64 {
        self.pml4 as *mut u64
    }

    /// Ensure the next-level table referenced by `entry` exists and return it.
    unsafe fn ensure_table(&mut self, entry: *mut u64, label: &'static str) -> Result<*mut u64, &'static str> {
        let value = *entry;
        if value & PTE_PRESENT != 0 {
            if value & PTE_HUGE != 0 {
                return Err("page table walk hit a huge page");
            }
            return Ok((value & ADDRESS_MASK) as *mut u64);
        }
        let table = allocate_pages(self.bs, 1).map_err(|_: Status| label)?;
        *entry = table | PTE_PRESENT | PTE_WRITE;
        Ok(table as *mut u64)
    }

    pub unsafe fn map_page(
        &mut self,
        virt: u64,
        phys: u64,
        flags: u64,
    ) -> Result<(), &'static str> {
        let i4 = ((virt >> 39) & 0x1FF) as usize;
        let i3 = ((virt >> 30) & 0x1FF) as usize;
        let i2 = ((virt >> 21) & 0x1FF) as usize;
        let i1 = ((virt >> 12) & 0x1FF) as usize;

        let pml4e = self.table().add(i4);
        let pdpt = self.ensure_table(pml4e, "cannot allocate PDPT")?;
        let pdpte = pdpt.add(i3);
        let pd = self.ensure_table(pdpte, "cannot allocate PD")?;
        let pde = pd.add(i2);
        let pt = self.ensure_table(pde, "cannot allocate PT")?;
        let pte = pt.add(i1);
        if *pte & PTE_PRESENT != 0 {
            return Err("duplicate page mapping");
        }
        *pte = (phys & ADDRESS_MASK) | flags | PTE_PRESENT;
        Ok(())
    }

    pub unsafe fn map_2m(
        &mut self,
        virt: u64,
        phys: u64,
        flags: u64,
    ) -> Result<(), &'static str> {
        let i4 = ((virt >> 39) & 0x1FF) as usize;
        let i3 = ((virt >> 30) & 0x1FF) as usize;
        let i2 = ((virt >> 21) & 0x1FF) as usize;
        let pml4e = self.table().add(i4);
        let pdpt = self.ensure_table(pml4e, "cannot allocate PDPT")?;
        let pdpte = pdpt.add(i3);
        let pd = self.ensure_table(pdpte, "cannot allocate PD")?;
        let pde = pd.add(i2);
        if *pde & PTE_PRESENT != 0 {
            return Err("duplicate huge page mapping");
        }
        *pde = (phys & ADDRESS_MASK) | flags | PTE_HUGE | PTE_PRESENT;
        Ok(())
    }

    /// Identity-free direct map of all physical memory at `HHDM_OFFSET`,
    /// mapped read/write, non-executable. 2 MiB pages keep table size small.
    pub unsafe fn map_physical(&mut self, max_phys: u64) -> Result<(), &'static str> {
        let mut phys = 0u64;
        while phys < max_phys {
            self.map_2m(HHDM_OFFSET + phys, phys, PTE_WRITE | PTE_NX | PTE_GLOBAL)?;
            phys += 0x20_0000;
        }
        Ok(())
    }

    pub unsafe fn map_kernel(
        &mut self,
        segments: &[crate::elf::Segment],
    ) -> Result<(), &'static str> {
        for seg in segments {
            if seg.memsz == 0 {
                continue;
            }
            let mut off = 0u64;
            while off < seg.memsz {
                let mut flags = PTE_GLOBAL;
                if seg.writable {
                    flags |= PTE_WRITE;
                }
                if !seg.executable {
                    flags |= PTE_NX;
                }
                if seg.vaddr < KERNEL_BASE {
                    return Err("kernel segment below kernel base");
                }
                self.map_page(seg.vaddr + off, seg.phys + off, flags)?;
                off += 0x1000;
            }
        }
        Ok(())
    }
}
