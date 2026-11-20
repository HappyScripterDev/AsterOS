//! COM1 serial console (16550 compatible).

use core::arch::asm;

const COM1: u16 = 0x3F8;

#[inline]
unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

pub fn init() {
    unsafe {
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x80);
        outb(COM1 + 0, 0x01);
        outb(COM1 + 1, 0x00);
        outb(COM1 + 3, 0x03);
        outb(COM1 + 2, 0xC7);
        outb(COM1 + 4, 0x03);
    }
}

pub fn put(byte: u8) {
    unsafe {
        let mut spins = 0u32;
        while inb(COM1 + 5) & 0x20 == 0 {
            spins += 1;
            if spins > 1_000_000 {
                break;
            }
        }
        outb(COM1, byte);
    }
}

/// True when a byte is waiting in the receive FIFO.
pub fn data_ready() -> bool {
    unsafe { inb(COM1 + 5) & 1 != 0 }
}

pub fn read() -> u8 {
    unsafe { inb(COM1) }
}
