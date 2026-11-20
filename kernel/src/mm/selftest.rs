//! Memory subsystem self-tests used at boot.

use crate::mm::{pmm, vmm};

const TEST_VIRT: u64 = 0xFFFF_B000_0000_0000;

pub fn run() -> bool {
    if !frame_test() {
        crate::error!("mm self-test: frame mapping failed");
        return false;
    }
    if !user_space_test() {
        crate::error!("mm self-test: user address space failed");
        return false;
    }
    if !isolation_test() {
        crate::error!("mm self-test: address-space isolation failed");
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

fn isolation_test() -> bool {
    let before = pmm::free_kib();
    let first = match vmm::create_user_pml4() {
        Some(pml4) => pml4,
        None => return false,
    };
    let second = match vmm::create_user_pml4() {
        Some(pml4) => pml4,
        None => {
            vmm::destroy_user_pml4(first);
            return false;
        }
    };

    let mut first_cursor = vmm::USER_BASE;
    let mut second_cursor = vmm::USER_BASE;
    let first_address =
        vmm::user_alloc(first, &mut first_cursor, 4096, vmm::PTE_WRITE | vmm::PTE_NX);
    let second_address = vmm::user_alloc(
        second,
        &mut second_cursor,
        4096,
        vmm::PTE_WRITE | vmm::PTE_NX,
    );
    let (first_address, second_address) = match (first_address, second_address) {
        (Ok(a), Ok(b)) => (a, b),
        _ => {
            vmm::destroy_user_pml4(first);
            vmm::destroy_user_pml4(second);
            return false;
        }
    };
    if first_address != second_address {
        return false;
    }

    if vmm::copy_to_user(first, first_address, b"first").is_err()
        || vmm::copy_to_user(second, second_address, b"second").is_err()
    {
        return false;
    }

    let mut first_buffer = [0u8; 5];
    let mut second_buffer = [0u8; 6];
    if vmm::copy_from_user(first, first_address, &mut first_buffer).is_err()
        || vmm::copy_from_user(second, second_address, &mut second_buffer).is_err()
    {
        return false;
    }
    if &first_buffer != b"first" || &second_buffer != b"second" {
        return false;
    }

    let first_physical = vmm::translate(first, first_address);
    let second_physical = vmm::translate(second, second_address);
    if first_physical.is_none() || first_physical == second_physical {
        return false;
    }

    // Kernel addresses must never validate as user buffers.
    if vmm::user_leaf(first, vmm::KERNEL_BASE, false).is_some() {
        return false;
    }

    vmm::destroy_user_pml4(first);
    vmm::destroy_user_pml4(second);
    let after = pmm::free_kib();
    if before > after + 64 {
        crate::error!(
            "isolation test leaked {} KiB (before {} after {})",
            before - after,
            before,
            after
        );
        return false;
    }
    true
}

fn user_space_test() -> bool {
    let before = pmm::free_kib();
    let pml4 = match vmm::create_user_pml4() {
        Some(pml4) => pml4,
        None => return false,
    };
    let mut cursor = vmm::USER_BASE;
    let address = match vmm::user_alloc(pml4, &mut cursor, 8192, vmm::PTE_WRITE | vmm::PTE_NX) {
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
