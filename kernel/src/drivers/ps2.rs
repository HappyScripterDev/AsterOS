//! PS/2 controller: 8042 keyboard and auxiliary (mouse) port.
//!
//! Scancode set 1 is used throughout. Key events carry ASCII for printable
//! keys, a stable `keys::*` code for special keys, and the live modifier
//! state. Mouse events are delivered as absolute positions clamped to the
//! framebuffer.

use aster_abi::{keys, mods, InputEvent, InputKind};

use crate::arch::idt::InterruptFrame;
use crate::mm::vmm;
use crate::sync::IrqLock;
use core::sync::atomic::{AtomicBool, Ordering};

const DATA: u16 = 0x60;
const STATUS: u16 = 0x64;
const COMMAND: u16 = 0x64;

const EVENT_CAPACITY: usize = 256;

struct State {
    shift: bool,
    ctrl: bool,
    alt: bool,
    meta: bool,
    caps: bool,
    extended: bool,
    mouse_cycle: u8,
    mouse_packet: [u8; 4],
    mouse_packet_size: u8,
    mouse_x: i32,
    mouse_y: i32,
    mouse_buttons: u32,
}

static STATE: IrqLock<State> = IrqLock::new(State {
    shift: false,
    ctrl: false,
    alt: false,
    meta: false,
    caps: false,
    extended: false,
    mouse_cycle: 0,
    mouse_packet: [0; 4],
    mouse_packet_size: 3,
    mouse_x: 640,
    mouse_y: 400,
    mouse_buttons: 0,
});

struct Queue {
    events: [InputEvent; EVENT_CAPACITY],
    head: usize,
    tail: usize,
    count: usize,
}

impl Queue {
    const fn new() -> Self {
        Self {
            events: [InputEvent {
                kind: 0,
                code: 0,
                value: 0,
                x: 0,
                y: 0,
                buttons: 0,
                mods: 0,
            }; EVENT_CAPACITY],
            head: 0,
            tail: 0,
            count: 0,
        }
    }

    fn push(&mut self, event: InputEvent) {
        if self.count == EVENT_CAPACITY {
            // Drop the oldest event: input must never block the IRQ handler.
            self.head = (self.head + 1) % EVENT_CAPACITY;
            self.count -= 1;
        }
        self.events[self.tail] = event;
        self.tail = (self.tail + 1) % EVENT_CAPACITY;
        self.count += 1;
    }
}

static QUEUE: IrqLock<Queue> = IrqLock::new(Queue::new());
static KEYBOARD_SEEN: AtomicBool = AtomicBool::new(false);
static MOUSE_SEEN: AtomicBool = AtomicBool::new(false);
static WHEEL_SEEN: AtomicBool = AtomicBool::new(false);

#[inline]
unsafe fn inb(port: u16) -> u8 {
    let value: u8;
    core::arch::asm!("in al, dx", in("dx") port, out("al") value, options(nomem, nostack, preserves_flags));
    value
}

#[inline]
unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!("out dx, al", in("dx") port, in("al") value, options(nomem, nostack, preserves_flags));
}

unsafe fn wait_input_clear() -> bool {
    for _ in 0..200_000 {
        if inb(STATUS) & 0x02 == 0 {
            return true;
        }
    }
    false
}

unsafe fn wait_output_full() -> bool {
    for _ in 0..200_000 {
        if inb(STATUS) & 0x01 != 0 {
            return true;
        }
    }
    false
}

unsafe fn controller_command(command: u8) {
    wait_input_clear();
    outb(COMMAND, command);
}

unsafe fn keyboard_write(value: u8) {
    wait_input_clear();
    outb(DATA, value);
}

unsafe fn read_data() -> u8 {
    wait_output_full();
    inb(DATA)
}

unsafe fn flush_output() {
    for _ in 0..32 {
        if inb(STATUS) & 0x01 == 0 {
            break;
        }
        let _ = inb(DATA);
    }
}

unsafe fn mouse_command(command: u8) {
    controller_command(0xD4);
    keyboard_write(command);
    let _ = read_data(); // ACK
}

/// Initialise both ports before interrupts are enabled.
pub fn init() {
    unsafe {
        controller_command(0xAD); // disable keyboard
        controller_command(0xA7); // disable auxiliary
        flush_output();

        controller_command(0xA8); // enable auxiliary port
        controller_command(0x20); // read configuration byte
        let mut config = read_data();
        config |= 0x01; // keyboard interrupt
        config |= 0x02; // mouse interrupt
        config &= !0x10; // enable keyboard clock
        config &= !0x20; // enable mouse clock
        controller_command(0x60);
        keyboard_write(config);

        // Mouse defaults, then the IntelliMouse magic sample-rate sequence.
        mouse_command(0xF6);
        mouse_command(0xF3);
        mouse_command(0xC8);
        mouse_command(0xF3);
        mouse_command(0x64);
        mouse_command(0xF3);
        mouse_command(0x50);
        mouse_command(0xF2);
        let id = read_data();
        let wheel = id == 3 || id == 4;
        if wheel {
            mouse_command(0xF3);
            mouse_command(0x64);
            crate::info!("ps2: IntelliMouse wheel enabled (id {})", id);
        } else {
            crate::info!("ps2: standard 3-byte mouse (id {})", id);
        }
        STATE.lock().mouse_packet_size = if wheel { 4 } else { 3 };
        mouse_command(0xF4);

        controller_command(0xAE); // enable keyboard
        flush_output();

        if let Some(info) = Some(crate::boot::info().framebuffer) {
            let mut state = STATE.lock();
            if info.width > 0 && info.height > 0 {
                state.mouse_x = info.width as i32 / 2;
                state.mouse_y = info.height as i32 / 2;
            }
        }
    }
    crate::info!("ps2: keyboard and mouse initialised");
}

fn push_key(code: u32, pressed: bool, state: &State) {
    let mut event = InputEvent {
        kind: InputKind::Key as u32,
        code,
        value: if pressed { 1 } else { 0 },
        x: state.mouse_x,
        y: state.mouse_y,
        buttons: state.mouse_buttons,
        mods: 0,
    };
    if state.shift {
        event.mods |= mods::SHIFT;
    }
    if state.ctrl {
        event.mods |= mods::CTRL;
    }
    if state.alt {
        event.mods |= mods::ALT;
    }
    if state.meta {
        event.mods |= mods::META;
    }
    if state.caps {
        event.mods |= mods::CAPS;
    }
    QUEUE.lock().push(event);
}

fn push_special(code: u32, state: &State) {
    push_key(code, true, state);
    push_key(code, false, state);
}

fn shifted_ascii(byte: u8) -> u8 {
    match byte {
        b'a'..=b'z' => byte - 32,
        b'1' => b'!',
        b'2' => b'@',
        b'3' => b'#',
        b'4' => b'$',
        b'5' => b'%',
        b'6' => b'^',
        b'7' => b'&',
        b'8' => b'*',
        b'9' => b'(',
        b'0' => b')',
        b'-' => b'_',
        b'=' => b'+',
        b'[' => b'{',
        b']' => b'}',
        b'\\' => b'|',
        b';' => b':',
        b'\'' => b'"',
        b'`' => b'~',
        b',' => b'<',
        b'.' => b'>',
        b'/' => b'?',
        other => other,
    }
}

fn scancode_ascii(code: u8, state: &State) -> Option<u8> {
    let base = match code {
        0x02 => b'1',
        0x03 => b'2',
        0x04 => b'3',
        0x05 => b'4',
        0x06 => b'5',
        0x07 => b'6',
        0x08 => b'7',
        0x09 => b'8',
        0x0A => b'9',
        0x0B => b'0',
        0x0C => b'-',
        0x0D => b'=',
        0x10 => b'q',
        0x11 => b'w',
        0x12 => b'e',
        0x13 => b'r',
        0x14 => b't',
        0x15 => b'y',
        0x16 => b'u',
        0x17 => b'i',
        0x18 => b'o',
        0x19 => b'p',
        0x1A => b'[',
        0x1B => b']',
        0x1E => b'a',
        0x1F => b's',
        0x20 => b'd',
        0x21 => b'f',
        0x22 => b'g',
        0x23 => b'h',
        0x24 => b'j',
        0x25 => b'k',
        0x26 => b'l',
        0x27 => b';',
        0x28 => b'\'',
        0x29 => b'`',
        0x2B => b'\\',
        0x2C => b'z',
        0x2D => b'x',
        0x2E => b'c',
        0x2F => b'v',
        0x30 => b'b',
        0x31 => b'n',
        0x32 => b'm',
        0x33 => b',',
        0x34 => b'.',
        0x35 => b'/',
        0x39 => b' ',
        0x4A => b'-',
        0x4E => b'+',
        0x53 => b'.',
        0x37 => b'*',
        0x47 => b'7',
        0x48 => b'8',
        0x49 => b'9',
        0x50 => b'2',
        0x51 => b'3',
        0x4B => b'4',
        0x4C => b'5',
        0x4D => b'6',
        0x52 => b'0',
        0x4F => b'1',
        _ => return None,
    };
    let mut byte = base;
    let letter = base.is_ascii_lowercase();
    if (state.shift ^ (state.caps && letter)) {
        byte = shifted_ascii(byte);
    }
    Some(byte)
}

fn handle_key(scancode: u8) {
    let mut state = STATE.lock();
    if scancode == 0xE0 {
        state.extended = true;
        return;
    }
    let released = scancode & 0x80 != 0;
    let code = scancode & 0x7F;
    let extended = state.extended;
    state.extended = false;

    if extended {
        let special = match code {
            0x48 => Some(keys::UP),
            0x50 => Some(keys::DOWN),
            0x4B => Some(keys::LEFT),
            0x4D => Some(keys::RIGHT),
            0x47 => Some(keys::HOME),
            0x4F => Some(keys::END),
            0x53 => Some(keys::DELETE),
            0x49 => Some(keys::PAGE_UP),
            0x51 => Some(keys::PAGE_DOWN),
            0x1C => Some(keys::ENTER),
            _ => None,
        };
        if code == 0x1D {
            state.ctrl = !released;
            push_key(keys::RIGHT_CTRL, !released, &state);
        } else if code == 0x38 {
            state.alt = !released;
            push_key(keys::RIGHT_ALT, !released, &state);
        } else if code == 0x5B {
            state.meta = !released;
            push_key(keys::LEFT_META, !released, &state);
        } else if code == 0x5C {
            state.meta = !released;
            push_key(keys::RIGHT_META, !released, &state);
        } else if let Some(key) = special {
            push_key(key, !released, &state);
        }
        return;
    }

    match code {
        0x2A => {
            state.shift = !released;
            push_key(keys::LEFT_SHIFT, !released, &state);
        }
        0x36 => {
            state.shift = !released;
            push_key(keys::RIGHT_SHIFT, !released, &state);
        }
        0x1D => {
            state.ctrl = !released;
            push_key(keys::LEFT_CTRL, !released, &state);
        }
        0x38 => {
            state.alt = !released;
            push_key(keys::LEFT_ALT, !released, &state);
        }
        0x3A => {
            if !released {
                state.caps = !state.caps;
            }
            push_key(keys::CAPS_LOCK, !released, &state);
        }
        0x01 => push_key(keys::ESC, !released, &state),
        0x0E => push_key(keys::BACKSPACE, !released, &state),
        0x0F => push_key(keys::TAB, !released, &state),
        0x1C => push_key(keys::ENTER, !released, &state),
        0x3B => push_key(keys::F1, !released, &state),
        0x3C => push_key(keys::F2, !released, &state),
        0x3D => push_key(keys::F3, !released, &state),
        0x3E => push_key(keys::F4, !released, &state),
        0x3F => push_key(keys::F5, !released, &state),
        0x40 => push_key(keys::F6, !released, &state),
        0x41 => push_key(keys::F7, !released, &state),
        0x42 => push_key(keys::F8, !released, &state),
        0x43 => push_key(keys::F9, !released, &state),
        0x44 => push_key(keys::F10, !released, &state),
        0x57 => push_key(keys::F11, !released, &state),
        0x58 => push_key(keys::F12, !released, &state),
        _ => {
            if let Some(ascii) = scancode_ascii(code, &state) {
                push_key(ascii as u32, !released, &state);
            }
        }
    }
}

pub fn handle_keyboard() {
    let status = unsafe { inb(STATUS) };
    if status & 0x01 == 0 {
        return;
    }
    let data = unsafe { inb(DATA) };
    if !KEYBOARD_SEEN.swap(true, Ordering::Relaxed) {
        crate::info!("ps2: first keyboard scancode {:#x}", data);
    }
    handle_key(data);
}

pub fn handle_mouse() {
    let status = unsafe { inb(STATUS) };
    if status & 0x01 == 0 {
        return;
    }
    let byte = unsafe { inb(DATA) };
    if !MOUSE_SEEN.swap(true, Ordering::Relaxed) {
        crate::info!("ps2: first mouse byte {:#x}", byte);
    }
    let mut state = STATE.lock();
    let size = state.mouse_packet_size.clamp(3, 4) as u8;
    let cycle = state.mouse_cycle as usize;
    state.mouse_packet[cycle] = byte;
    state.mouse_cycle += 1;
    if state.mouse_cycle < size {
        return;
    }
    state.mouse_cycle = 0;
    let packet = state.mouse_packet;
    if packet[0] & 0x08 == 0 {
        return; // out of sync
    }
    let wheel = if size == 4 {
        let raw = (packet[3] & 0x0F) as i8;
        let value = if raw & 0x08 != 0 { raw - 16 } else { raw };
        if value != 0 {
            Some(value)
        } else {
            None
        }
    } else {
        None
    };
    let dx = packet[1] as i8 as i32;
    let dy = packet[2] as i8 as i32;
    let buttons = (packet[0] & 0x07) as u32;

    if let Some(info) = Some(crate::boot::info().framebuffer) {
        state.mouse_x = (state.mouse_x + dx).clamp(0, info.width.saturating_sub(1) as i32);
        state.mouse_y = (state.mouse_y - dy).clamp(0, info.height.saturating_sub(1) as i32);
    } else {
        state.mouse_x = (state.mouse_x + dx).clamp(0, 4095);
        state.mouse_y = (state.mouse_y - dy).clamp(0, 4095);
    }

    let position = InputEvent {
        kind: InputKind::MouseMove as u32,
        code: 0,
        value: 0,
        x: state.mouse_x,
        y: state.mouse_y,
        buttons,
        mods: 0,
    };
    QUEUE.lock().push(position);

    if let Some(delta) = wheel {
        let event = InputEvent {
            kind: InputKind::MouseWheel as u32,
            code: 0,
            value: delta as i32,
            x: state.mouse_x,
            y: state.mouse_y,
            buttons,
            mods: 0,
        };
        QUEUE.lock().push(event);
        if !WHEEL_SEEN.swap(true, Ordering::Relaxed) {
            crate::info!("ps2: first wheel event {}", delta);
        }
    }

    if buttons != state.mouse_buttons {
        let changed = buttons ^ state.mouse_buttons;
        for button in 0..3 {
            if changed & (1 << button) != 0 {
                let event = InputEvent {
                    kind: InputKind::MouseButton as u32,
                    code: button,
                    value: if buttons & (1 << button) != 0 { 1 } else { 0 },
                    x: state.mouse_x,
                    y: state.mouse_y,
                    buttons,
                    mods: 0,
                };
                QUEUE.lock().push(event);
            }
        }
        state.mouse_buttons = buttons;
    }
}

/// Copy pending events into a user buffer. Returns the number of events.
pub fn poll(pointer: u64, maximum: usize, pml4: u64) -> u64 {
    if maximum == 0 {
        return 0;
    }
    let mut queue = QUEUE.lock();
    let count = queue.count.min(maximum);
    if count == 0 {
        return 0;
    }
    let event_size = core::mem::size_of::<InputEvent>();
    let mut buffer = alloc::vec![0u8; count * event_size];
    for index in 0..count {
        let event = queue.events[(queue.head + index) % EVENT_CAPACITY];
        let bytes = unsafe {
            core::slice::from_raw_parts(&event as *const InputEvent as *const u8, event_size)
        };
        buffer[index * event_size..(index + 1) * event_size].copy_from_slice(bytes);
    }
    queue.head = (queue.head + count) % EVENT_CAPACITY;
    queue.count -= count;
    drop(queue);
    if vmm::copy_to_user(pml4, pointer, &buffer).is_err() {
        return 0;
    }
    count as u64
}

/// Test hook: inject a synthetic key event (used by boot self-tests).
pub fn inject_test_key(ascii: u8) {
    let state = STATE.lock();
    push_key(ascii as u32, true, &state);
    push_key(ascii as u32, false, &state);
}

pub fn _unused(_frame: *mut InterruptFrame) {}
