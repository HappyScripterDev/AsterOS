//! CMOS real-time clock (ports 0x70/0x71).

use core::arch::asm;

use aster_abi::RtcTime;

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

unsafe fn cmos(index: u8) -> u8 {
    outb(0x70, index);
    inb(0x71)
}

fn bcd(value: u8) -> u8 {
    (value & 0x0F) + ((value >> 4) * 10)
}

/// Read the wall clock. Waits for the update-in-progress flag to clear and
/// retries once if the date changed mid-read (standard CMOS procedure).
pub fn read() -> RtcTime {
    unsafe {
        let mut attempts = 0;
        loop {
            while cmos(0x0A) & 0x80 != 0 {
                core::hint::spin_loop();
            }
            let second = cmos(0x00);
            let minute = cmos(0x02);
            let hour = cmos(0x04);
            let day = cmos(0x07);
            let month = cmos(0x08);
            let year = cmos(0x09);
            let register_b = cmos(0x0B);
            let second_after = cmos(0x00);
            if second == second_after || attempts > 4 {
                let binary = register_b & 0x04 != 0;
                let hour24 = register_b & 0x02 != 0;
                let mut hour = if binary { hour } else { bcd(hour) };
                let minute = if binary { minute } else { bcd(minute) };
                let second = if binary { second } else { bcd(second) };
                let day = if binary { day } else { bcd(day) };
                let month = if binary { month } else { bcd(month) };
                let year = if binary { year } else { bcd(year) };
                if !hour24 {
                    let pm = hour & 0x80 != 0;
                    hour &= 0x7F;
                    if pm && hour < 12 {
                        hour += 12;
                    }
                }
                return RtcTime {
                    year: 2000 + year as u16,
                    month,
                    day,
                    hour,
                    minute,
                    second,
                    weekday: if binary { cmos(0x06) } else { bcd(cmos(0x06)) },
                    reserved: 0,
                };
            }
            attempts += 1;
        }
    }
}
