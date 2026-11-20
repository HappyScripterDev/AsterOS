//! 8259 PIC remapping and interrupt masking.

use core::arch::asm;

const PIC1_COMMAND: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_COMMAND: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const ICW1_INIT: u8 = 0x11;
const ICW4_8086: u8 = 0x01;

#[inline]
unsafe fn outb(port: u16, value: u8) {
    asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

#[inline]
unsafe fn io_wait() {
    outb(0x80, 0);
}

pub unsafe fn remap(offset: u8) {
    let saved1 = inb(PIC1_DATA);
    let saved2 = inb(PIC2_DATA);

    outb(PIC1_COMMAND, ICW1_INIT);
    io_wait();
    outb(PIC2_COMMAND, ICW1_INIT);
    io_wait();
    outb(PIC1_DATA, offset);
    io_wait();
    outb(PIC2_DATA, offset + 8);
    io_wait();
    outb(PIC1_DATA, 4);
    io_wait();
    outb(PIC2_DATA, 2);
    io_wait();
    outb(PIC1_DATA, ICW4_8086);
    io_wait();
    outb(PIC2_DATA, ICW4_8086);
    io_wait();

    outb(PIC1_DATA, saved1);
    outb(PIC2_DATA, saved2);
}

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

pub unsafe fn set_mask(irq: u8) {
    if irq < 8 {
        let mask = inb(PIC1_DATA) | (1 << irq);
        outb(PIC1_DATA, mask);
    } else {
        let mask = inb(PIC2_DATA) | (1 << (irq - 8));
        outb(PIC2_DATA, mask);
    }
}

pub unsafe fn clear_mask(irq: u8) {
    if irq < 8 {
        let mask = inb(PIC1_DATA) & !(1 << irq);
        outb(PIC1_DATA, mask);
    } else {
        let mask = inb(PIC2_DATA) & !(1 << (irq - 8));
        outb(PIC2_DATA, mask);
    }
}

pub unsafe fn eoi(irq: u8) {
    if irq >= 8 {
        outb(PIC2_COMMAND, 0x20);
    }
    outb(PIC1_COMMAND, 0x20);
}

/// Mask everything, then enable only the interrupts the kernel owns.
pub unsafe fn init() {
    remap(32);
    for irq in 0..16 {
        set_mask(irq);
    }
    // Timer, keyboard, cascade, mouse.
    clear_mask(0);
    clear_mask(1);
    clear_mask(2);
    clear_mask(12);
}
