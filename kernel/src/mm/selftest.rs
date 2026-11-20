//! Memory subsystem self-tests used at boot.

use crate::mm::{pmm, vmm};

const TEST_VIRT: u64 = 0xFFFF_A000_0000_0000;

pub fn run() -> bool {
    if !frame_test() {
        crate::error!("mm self-test: frame mapping failed");
        return false;
    }
    if !user_space_test() {
        crate::error!("mm self-test: user address space failed");
        return false;
    }
    true
}

fn frame_test() -> bool {
    let frame = match pmm::alloc_zeroed_frame() {
        Some(frame) => frame,
        None => return false,
    };
    unsafe {
        let pointer = vmm::hhdm(frame) as *mut u8;
        if pointer.read() != 0 {
            return false;
        }
        pointer.write(0x5A);
        pointer.add(4095).write(0xA5);
    }
    if vmm::map_page(
        vmm::kernel_pml4(),
        TEST_VIRT,
        frame,
        vmm::PTE_WRITE | vmm::PTE_NX | vmm::PTE_GLOBAL,
    )
    .is_err()
    {
        return false;
    }
    let translated = vmm::translate(vmm::kernel_pml4(), TEST_VIRT);
    if translated != Some(frame) {
        return false;
    }
    unsafe {
        let mapped = TEST_VIRT as *mut u8;
        if mapped.read() != 0x5A || mapped.add(4095).read() != 0xA5 {
            return false;
        }
    }
    if vmm::unmap_page(vmm::kernel_pml4(), TEST_VIRT).is_err() {
        return false;
    }
    pmm::free_frame(frame);
    true
}

fn user_space_test() -> bool {
    let before = pmm::free_kib();
    let pml4 = match vmm::create_user_pml4() {
        Some(pml4) => pml4,
        None => return false,
    };
    let mut cursor = vmm::USER_BASE;
    let address = match vmm::user_alloc(
        pml4,
        &mut cursor,
        8192,
        vmm::PTE_WRITE | vmm::PTE_NX,
    ) {
        Ok(address) => address,
        Err(_) => return false,
    };
    let payload = b"aster user space";
    if vmm::copy_to_user(pml4, address, payload).is_err() {
        return false;
    }
    let mut buffer = [0u8; 16];
    if vmm::copy_from_user(pml4, address, &mut buffer).is_err() {
        return false;
    }
    if &buffer != payload {
        return false;
    }
    vmm::destroy_user_pml4(pml4);
    let after = pmm::free_kib();
    if before > after + 64 {
        crate::error!(
            "user space test leaked {} KiB (before {} after {})",
            before - after,
            before,
            after
        );
        return false;
    }
    true
}
