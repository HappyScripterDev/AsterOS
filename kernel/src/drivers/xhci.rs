//! Minimal xHCI host controller with USB HID boot keyboard/mouse and hotplug.
//!
//! The controller is polled from a kernel thread: the command/event rings are
//! real xHCI rings, but no MSI is programmed yet. Transfers never run in IRQ
//! context and all DMA buffers are physical frames.

use core::sync::atomic::Ordering;

use crate::drivers::pci;
use crate::mm::{pmm, vmm};
use crate::sync::IrqLock;
use aster_abi::{keys, InputEvent, InputKind};

const MAX_PORTS: usize = 16;
const MAX_DEVICES: usize = 8;
const RING_TRBS: usize = 16;
const EVT_TRBS: usize = 64;
const USABLE_TRBS: usize = RING_TRBS - 1;

const REG_USBCMD: u64 = 0x00;
const REG_USBSTS: u64 = 0x04;
const REG_CRCR: u64 = 0x18;
const REG_DCBAAP: u64 = 0x30;
const REG_CONFIG: u64 = 0x38;
const CMD_RUN: u32 = 1;
const CMD_RESET: u32 = 1 << 1;
const STS_HALT: u32 = 1;
const STS_CNR: u32 = 1 << 11;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Keyboard,
    Mouse,
}

#[derive(Clone, Copy)]
struct Device {
    slot: u8,
    port: u8,
    kind: Kind,
    speed: u8,
    dci: u8,
    report_phys: u64,
    ep0_ring_phys: u64,
    ep0_enqueue: usize,
    ep0_cycle: u8,
    ep_ring_phys: u64,
    ep_enqueue: usize,
    ep_cycle: u8,
    last_keys: [u8; 6],
    last_buttons: u8,
    x: i32,
    y: i32,
}

struct Controller {
    mmio: u64,
    op: u64,
    runtime: u64,
    doorbell: u64,
    max_slots: u8,
    max_ports: u8,
    cmd_phys: u64,
    cmd_va: u64,
    cmd_index: usize,
    cmd_cycle: u8,
    evt_phys: u64,
    evt_va: u64,
    evt_index: usize,
    evt_cycle: u8,
    erst_phys: u64,
    erst_va: u64,
    dcbaa_phys: u64,
    dcbaa_va: u64,
    input_phys: u64,
    input_va: u64,
    control_phys: u64,
    control_va: u64,
    devices: [Option<Device>; MAX_DEVICES],
    connected: [bool; MAX_PORTS],
}

static CONTROLLER: IrqLock<Option<Controller>> = IrqLock::new(None);

fn read32(address: u64) -> u32 {
    unsafe { ((address) as *const u32).read_volatile() }
}

fn write32(address: u64, value: u32) {
    unsafe { ((address) as *mut u32).write_volatile(value) }
}

fn write64(address: u64, value: u64) {
    unsafe { ((address) as *mut u64).write_volatile(value) }
}

impl Controller {
    fn delay(&self, spins: usize) {
        for _ in 0..spins {
            core::hint::spin_loop();
        }
    }

    fn command(&mut self, param: u64, status: u32, trb_type: u32, slot: u8) -> Option<u8> {
        let index = self.cmd_index;
        let control = (trb_type << 10) | self.cmd_cycle as u32 | ((slot as u32) << 24);
        let trb = self.cmd_va + (index * 16) as u64;
        write64(trb, param);
        write32(trb + 8, status);
        write32(trb + 12, control);
        let trb_phys = self.cmd_phys + (index * 16) as u64;
        self.cmd_index += 1;
        if self.cmd_index >= USABLE_TRBS {
            let link = self.cmd_va + (USABLE_TRBS * 16) as u64;
            write64(link, self.cmd_phys);
            write32(link + 8, 0);
            write32(link + 12, (6 << 10) | (1 << 1) | self.cmd_cycle as u32);
            self.cmd_index = 0;
            self.cmd_cycle ^= 1;
        }
        write32(self.doorbell, 0);

        for _ in 0..2_000_000usize {
            if let Some((param, status, _, control)) = self.next_event() {
                let event_type = (control >> 10) & 0x3F;
                if event_type == 33 && param == trb_phys {
                    let code = (status >> 24) & 0xFF;
                    let event_slot = ((control >> 24) & 0xFF) as u8;
                    if code != 1 {
                        crate::warn!("xhci: command {} failed with code {}", trb_type, code);
                        return None;
                    }
                    return Some(event_slot);
                }
            }
            core::hint::spin_loop();
        }
        crate::warn!("xhci: command {} timed out", trb_type);
        None
    }

    fn next_event(&mut self) -> Option<(u64, u32, u32, u32)> {
        let index = self.evt_index;
        let trb = self.evt_va + (index * 16) as u64;
        let control = read32(trb + 12);
        if control & 1 != self.evt_cycle as u32 {
            return None;
        }
        let event = (
            {
                let low = read32(trb) as u64;
                let high = read32(trb + 4) as u64;
                low | (high << 32)
            },
            read32(trb + 8),
            read32(trb + 8),
            control,
        );
        self.evt_index += 1;
        if self.evt_index >= EVT_TRBS {
            self.evt_index = 0;
            self.evt_cycle ^= 1;
        }
        let erdp = self.runtime + 0x20 + 0x18;
        write64(
            erdp,
            self.evt_phys + (self.evt_index * 16) as u64 | (1 << 3),
        );
        Some(event)
    }

    fn wait_transfer(&mut self, slot: u8, dci: u8) -> bool {
        // QEMU services xHCI transfers from its main loop; a tight guest spin
        // with interrupts disabled can starve that loop. Let the timer preempt
        // while the controller lock is held (no IRQ handler touches it).
        let was_enabled = crate::arch::cpu::interrupts_enabled();
        crate::arch::cpu::enable_interrupts();
        for _ in 0..2_000_000usize {
            if let Some((_, status, _, control)) = self.next_event() {
                let event_type = (control >> 10) & 0x3F;
                if event_type == 32 {
                    let event_slot = ((control >> 24) & 0xFF) as u8;
                    let event_ep = ((control >> 16) & 0x1F) as u8;
                    if event_slot == slot && event_ep == dci {
                        let code = (status >> 24) & 0xFF;
                        if code != 1 && code != 13 {
                            crate::warn!(
                                "xhci: transfer event slot {} ep {} code {}",
                                event_slot,
                                event_ep,
                                code
                            );
                        }
                        return code == 1 || code == 13;
                    }
                }
            }
            core::hint::spin_loop();
        }
        if !was_enabled {
            crate::arch::cpu::disable_interrupts();
        }
        crate::warn!(
            "xhci: transfer timeout slot {} ep {} evt idx {} cycle {}",
            slot,
            dci,
            self.evt_index,
            self.evt_cycle
        );
        false
    }

    fn push_ep_trb_impl(doorbell: u64, dev: &mut Device, param: u64, length: u32) {
        let index = dev.ep_enqueue;
        let trb = vmm::hhdm(dev.ep_ring_phys) + (index * 16) as u64;
        write64(trb, param);
        write32(trb + 8, length);
        write32(trb + 12, (1 << 10) | (1 << 5) | dev.ep_cycle as u32);
        dev.ep_enqueue += 1;
        if dev.ep_enqueue >= USABLE_TRBS {
            let link = vmm::hhdm(dev.ep_ring_phys) + (USABLE_TRBS * 16) as u64;
            write64(link, dev.ep_ring_phys);
            write32(link + 8, 0);
            write32(link + 12, (6 << 10) | (1 << 1) | dev.ep_cycle as u32);
            dev.ep_enqueue = 0;
            dev.ep_cycle ^= 1;
        }
        write32(
            doorbell + dev.slot as u64 * 4,
            dev.slot as u32 | ((dev.dci as u32) << 8),
        );
    }

    fn control(
        &mut self,
        dev: &mut Device,
        setup: [u8; 8],
        data: Option<(&mut [u8], bool)>,
    ) -> bool {
        let setup_word = u64::from_le_bytes(setup);
        let mut data = data;
        let ring_va = vmm::hhdm(dev.ep0_ring_phys);
        let mut push = |dev: &mut Device, param: u64, status: u32, control: u32| {
            let index = dev.ep0_enqueue;
            let trb = ring_va + (index * 16) as u64;
            write64(trb, param);
            write32(trb + 8, status);
            write32(trb + 12, control | dev.ep0_cycle as u32);
            dev.ep0_enqueue += 1;
            if dev.ep0_enqueue >= USABLE_TRBS {
                let link = ring_va + (USABLE_TRBS * 16) as u64;
                write64(link, dev.ep0_ring_phys);
                write32(link + 8, 0);
                write32(link + 12, (6 << 10) | (1 << 1) | dev.ep0_cycle as u32);
                dev.ep0_enqueue = 0;
                dev.ep0_cycle ^= 1;
            }
        };
        push(dev, setup_word, 8, (2 << 10) | (1 << 6));
        if let Some((buffer, input)) = data.as_mut() {
            let length = buffer.len() as u32;
            let direction = if *input { 1 << 16 } else { 0 };
            let physical = self.control_phys;
            let destination = unsafe {
                core::slice::from_raw_parts_mut(self.control_va as *mut u8, buffer.len())
            };
            if *input {
                destination.fill(0);
            } else {
                destination.copy_from_slice(buffer);
            }
            push(dev, physical, length, (3 << 10) | direction);
            push(dev, 0, 0, (4 << 10) | (1 << 5));
        } else {
            push(dev, 0, 0, (4 << 10) | (1 << 16) | (1 << 5));
        }
        write32(
            self.doorbell + dev.slot as u64 * 4,
            dev.slot as u32 | (1 << 8),
        );
        let ok = self.wait_transfer(dev.slot, 1);
        if !ok {
            crate::warn!("xhci: control {:02x?} failed on slot {}", setup, dev.slot);
        }
        if let Some((buffer, input)) = data.as_mut() {
            if *input && ok {
                buffer.copy_from_slice(unsafe {
                    core::slice::from_raw_parts(self.control_va as *const u8, buffer.len())
                });
            }
        }
        ok
    }
}

fn dma_page() -> Option<(u64, u64)> {
    let physical = pmm::alloc_zeroed_frame()?;
    Some((physical, vmm::hhdm(physical)))
}

fn descriptor_request(kind: u16, index: u16, length: u16) -> [u8; 8] {
    [
        0x80,
        6,
        (kind & 0xFF) as u8,
        (kind >> 8) as u8,
        index as u8,
        (index >> 8) as u8,
        (length & 0xFF) as u8,
        (length >> 8) as u8,
    ]
}

fn standard_out(request: u8, value: u16, index: u16) -> [u8; 8] {
    [
        request,
        0,
        (value & 0xFF) as u8,
        (value >> 8) as u8,
        index as u8,
        (index >> 8) as u8,
        0,
        0,
    ]
}

fn parse_interface(config: &[u8]) -> Option<(u8, u8, Kind, u8, u8)> {
    let mut offset = 0usize;
    let mut interface = 0u8;
    let mut kind = None;
    let mut ep_address = 0u8;
    let mut ep_packet = 0u8;
    while offset + 2 <= config.len() {
        let length = config[offset] as usize;
        let descriptor_type = config[offset + 1];
        if length < 2 || offset + length > config.len() {
            break;
        }
        match descriptor_type {
            4 if length >= 9 => {
                interface = config[offset + 2];
                if config[offset + 5] == 3 && config[offset + 6] == 1 {
                    kind = match config[offset + 7] {
                        1 => Some(Kind::Keyboard),
                        2 => Some(Kind::Mouse),
                        _ => None,
                    };
                }
            }
            5 if length >= 7 && kind.is_some() => {
                if config[offset + 2] & 0x80 != 0 {
                    ep_address = config[offset + 2];
                    ep_packet = config[offset + 4];
                }
            }
            _ => {}
        }
        offset += length;
    }
    let kind = kind?;
    if ep_address == 0 {
        return None;
    }
    Some((interface, ep_address, kind, ep_packet, 0))
}

fn hid_usage_to_key(usage: u8) -> Option<u32> {
    match usage {
        0x04..=0x1D => Some(b'a' as u32 + (usage - 0x04) as u32),
        0x1E..=0x27 => Some(b'1' as u32 + (usage - 0x1E) as u32),
        0x28 => Some(keys::ENTER),
        0x29 => Some(keys::ESC),
        0x2A => Some(keys::BACKSPACE),
        0x2B => Some(keys::TAB),
        0x2C => Some(b' ' as u32),
        0x2D => Some(b'-' as u32),
        0x2E => Some(b'=' as u32),
        0x2F => Some(b'[' as u32),
        0x30 => Some(b']' as u32),
        0x31 => Some(b'\\' as u32),
        0x33 => Some(b';' as u32),
        0x34 => Some(b'\'' as u32),
        0x35 => Some(b'`' as u32),
        0x36 => Some(b',' as u32),
        0x37 => Some(b'.' as u32),
        0x38 => Some(b'/' as u32),
        0x4F => Some(keys::RIGHT),
        0x50 => Some(keys::LEFT),
        0x51 => Some(keys::DOWN),
        0x52 => Some(keys::UP),
        _ => None,
    }
}

fn key_event(code: u32, pressed: bool, dev: &Device) -> InputEvent {
    InputEvent {
        kind: InputKind::Key as u32,
        code,
        value: if pressed { 1 } else { 0 },
        x: dev.x,
        y: dev.y,
        buttons: 0,
        mods: 0,
    }
}

fn handle_keyboard(dev: &mut Device) -> bool {
    let report = unsafe { core::slice::from_raw_parts(vmm::hhdm(dev.report_phys) as *const u8, 8) };
    let mut changed = false;
    for slot in 0..6 {
        let usage = report[2 + slot];
        if usage == 0 {
            continue;
        }
        if !dev.last_keys.contains(&usage) {
            if let Some(code) = hid_usage_to_key(usage) {
                crate::drivers::ps2::push_external(key_event(code, true, dev));
                changed = true;
            }
        }
    }
    for slot in 0..6 {
        let usage = dev.last_keys[slot];
        if usage != 0 && !report[2..8].contains(&usage) {
            if let Some(code) = hid_usage_to_key(usage) {
                crate::drivers::ps2::push_external(key_event(code, false, dev));
                changed = true;
            }
        }
    }
    dev.last_keys.copy_from_slice(&report[2..8]);
    changed
}

fn handle_mouse(dev: &mut Device) -> bool {
    let report = unsafe { core::slice::from_raw_parts(vmm::hhdm(dev.report_phys) as *const u8, 4) };
    let buttons = report[0];
    dev.x = (dev.x + report[1] as i8 as i32).clamp(0, 1279);
    dev.y = (dev.y + report[2] as i8 as i32).clamp(0, 799);
    let position = InputEvent {
        kind: InputKind::MouseMove as u32,
        code: 0,
        value: 0,
        x: dev.x,
        y: dev.y,
        buttons: (buttons & 7) as u32,
        mods: 0,
    };
    crate::drivers::ps2::push_external(position);
    if buttons & 7 != dev.last_buttons & 7 {
        let changed = (buttons ^ dev.last_buttons) & 7;
        for button in 0..3 {
            if changed & (1 << button) != 0 {
                crate::drivers::ps2::push_external(InputEvent {
                    kind: InputKind::MouseButton as u32,
                    code: button,
                    value: if buttons & (1 << button) != 0 { 1 } else { 0 },
                    x: dev.x,
                    y: dev.y,
                    buttons: (buttons & 7) as u32,
                    mods: 0,
                });
            }
        }
    }
    let wheel = report[3] as i8;
    if wheel != 0 {
        crate::drivers::ps2::push_external(InputEvent {
            kind: InputKind::MouseWheel as u32,
            code: 0,
            value: wheel as i32,
            x: dev.x,
            y: dev.y,
            buttons: (buttons & 7) as u32,
            mods: 0,
        });
    }
    dev.last_buttons = buttons;
    true
}

fn port_reset(controller: &mut Controller, port: u8) -> Option<u8> {
    let address = controller.op + 0x400 + port as u64 * 0x10;
    let mut status = read32(address);
    if status & 1 == 0 {
        return None;
    }
    write32(address, (status & !0xFE00_0000) | (1 << 4) | (0x7F << 17));
    for _ in 0..2_000_000usize {
        status = read32(address);
        if status & (1 << 21) != 0 {
            break;
        }
        core::hint::spin_loop();
    }
    write32(address, status | (0x7F << 17));
    let speed = ((status >> 10) & 0xF) as u8;
    crate::info!(
        "xhci: port {} reset status {:#x} speed {}",
        port,
        status,
        speed
    );
    if status & 1 == 0 || speed == 0 {
        return None;
    }
    Some(speed)
}

fn address_device(controller: &mut Controller, port: u8, speed: u8) -> Option<Device> {
    let slot = controller.command(0, 0, 9, 0)?;
    if slot == 0 {
        return None;
    }
    let Some((device_ctx_phys, device_ctx_va)) = dma_page() else {
        return None;
    };
    write64(controller.dcbaa_va + slot as u64 * 8, device_ctx_phys);
    let input = controller.input_va;
    unsafe { core::ptr::write_bytes(input as *mut u8, 0, 4096) };
    // Input control context: add slot and EP0.
    write32(input + 4, 3);
    let slot_context = input + 32;
    write32(slot_context, ((speed as u32) << 20) | (1 << 27));
    write32(slot_context + 4, (port as u32) << 16);
    let ep0 = input + 64;
    let max_packet = match speed {
        4 => 512u32,
        3 => 64,
        _ => 8,
    };
    write32(ep0 + 4, (3 << 1) | (4 << 3) | (max_packet << 16));
    let ep0_ring = dma_page()?;
    write64(ep0 + 8, ep0_ring.0 | 1);
    if controller
        .command(controller.input_phys, 0, 11, slot)
        .is_none()
    {
        return None;
    }
    let Some(report) = dma_page() else {
        return None;
    };
    Some(Device {
        slot,
        port,
        kind: Kind::Keyboard,
        speed,
        dci: 3,
        report_phys: report.0,
        ep0_ring_phys: ep0_ring.0,
        ep0_enqueue: 0,
        ep0_cycle: 1,
        ep_ring_phys: ep0_ring.0,
        ep_enqueue: 0,
        ep_cycle: 1,
        last_keys: [0; 6],
        last_buttons: 0,
        x: 640,
        y: 400,
    })
}

fn configure_hid(controller: &mut Controller, dev: &mut Device) -> bool {
    let mut device_descriptor = [0u8; 18];
    if !controller.control(
        dev,
        descriptor_request(0x0100, 0, 18),
        Some((&mut device_descriptor, true)),
    ) {
        crate::warn!("xhci: device descriptor read failed on slot {}", dev.slot);
        return false;
    }
    let mut header = [0u8; 9];
    if !controller.control(
        dev,
        descriptor_request(0x0200, 0, 9),
        Some((&mut header, true)),
    ) {
        return false;
    }
    let total = u16::from_le_bytes([header[2], header[3]]) as usize;
    let mut config = [0u8; 256];
    let length = total.min(config.len());
    if !controller.control(
        dev,
        descriptor_request(0x0200, 0, length as u16),
        Some((&mut config[..length], true)),
    ) {
        return false;
    }
    let Some((interface, ep_address, kind, packet, _)) = parse_interface(&config[..length]) else {
        crate::warn!("xhci: no HID boot interface on slot {}", dev.slot);
        return false;
    };
    dev.kind = kind;
    let set_config = [0x00, 0x09, 1, 0, 0, 0, 0, 0];
    if !controller.control(dev, set_config, None::<(&mut [u8], bool)>) {
        return false;
    }
    // SET_PROTOCOL(boot) to the interface.
    let boot = [0x21, 0x0B, 0, 0, interface, 0, 0, 0];
    let _ = controller.control(dev, boot, None::<(&mut [u8], bool)>);
    let dci = if ep_address & 0x0F == 0 {
        3
    } else {
        (ep_address & 0x0F) * 2 + 1
    };
    dev.dci = dci;

    // Input context for Configure Endpoint: slot + EP.
    let input = controller.input_va;
    unsafe { core::ptr::write_bytes(input as *mut u8, 0, 4096) };
    write32(input + 4, 1 | (1 << dci));
    let slot_context = input + 32;
    write32(slot_context, ((dev.speed as u32) << 20) | (2 << 27));
    write32(slot_context + 4, (dev.port as u32) << 16);
    let ep = input + 32u64 * (1 + dci as u64);
    write32(ep, 6 << 16);
    write32(ep + 4, (3 << 1) | (7 << 3) | ((packet.max(8) as u32) << 16));
    let ep_ring = dma_page();
    let Some(ep_ring) = ep_ring else {
        return false;
    };
    write64(ep + 8, ep_ring.0 | 1);
    if controller
        .command(controller.input_phys, 0, 12, dev.slot)
        .is_none()
    {
        return false;
    }
    dev.ep_ring_phys = ep_ring.0;
    dev.ep_enqueue = 0;
    dev.ep_cycle = 1;
    Controller::push_ep_trb_impl(controller.doorbell, dev, dev.report_phys, 8);
    crate::info!(
        "xhci: {:?} on slot {} port {} (if {}, ep {:#x}, packet {})",
        dev.kind,
        dev.slot,
        dev.port,
        interface,
        ep_address,
        packet
    );
    true
}

fn enumerate_port(controller: &mut Controller, port: u8) {
    if controller
        .devices
        .iter()
        .flatten()
        .any(|device| device.port == port)
    {
        controller.connected[port as usize] = true;
        return;
    }
    let Some(speed) = port_reset(controller, port) else {
        return;
    };
    let Some(mut device) = address_device(controller, port, speed) else {
        crate::warn!("xhci: address device failed on port {}", port);
        controller.connected[port as usize] = true;
        return;
    };
    if !configure_hid(controller, &mut device) {
        controller.connected[port as usize] = true;
        return;
    }
    if let Some(slot) = controller.devices.iter_mut().find(|slot| slot.is_none()) {
        *slot = Some(device);
        controller.connected[port as usize] = true;
        crate::info!("xhci: HID device ready on port {}", port);
    }
}

fn poll(controller: &mut Controller) {
    for port in 1..=controller.max_ports.min(MAX_PORTS as u8) {
        let address = controller.op + 0x400 + port as u64 * 0x10;
        let status = read32(address);
        let connected = status & 1 != 0;
        let change = status & (1 << 17) != 0;
        if change {
            write32(address, status | (0x7F << 17));
        }
        if connected && !controller.connected[port as usize] {
            enumerate_port(controller, port);
        }
        if !connected && controller.connected[port as usize] {
            controller.connected[port as usize] = false;
            for slot in controller.devices.iter_mut() {
                if slot.map(|device| device.port == port).unwrap_or(false) {
                    *slot = None;
                }
            }
        }
    }

    let mut events = 0;
    while let Some((_, status, _, control)) = controller.next_event() {
        let event_type = (control >> 10) & 0x3F;
        if event_type != 32 {
            continue;
        }
        let event_slot = ((control >> 24) & 0xFF) as u8;
        let event_ep = ((control >> 16) & 0x1F) as u8;
        let code = (status >> 24) & 0xFF;
        let mut requeue = None;
        for slot in controller.devices.iter_mut() {
            if let Some(device) = slot {
                if device.slot == event_slot && device.dci == event_ep {
                    if code == 1 || code == 13 {
                        match device.kind {
                            Kind::Keyboard => {
                                handle_keyboard(device);
                            }
                            Kind::Mouse => {
                                handle_mouse(device);
                            }
                        }
                    }
                    requeue = Some(*device);
                }
            }
        }
        if let Some(device) = requeue {
            if let Some(slot) = controller
                .devices
                .iter_mut()
                .find(|slot| slot.map(|d| d.slot == device.slot).unwrap_or(false))
            {
                if let Some(device) = slot {
                    let report = device.report_phys;
                    Controller::push_ep_trb_impl(controller.doorbell, device, report, 8);
                }
            }
        }
        events += 1;
        if events > 16 {
            break;
        }
    }
}

pub fn init() -> bool {
    let Some(device) = pci::find_xhci() else {
        crate::info!("xhci: no USB 3 controller present");
        return false;
    };
    let Some(bar) = pci::memory_bar(&device, 0) else {
        crate::warn!("xhci: BAR0 is not a memory BAR");
        return false;
    };
    pci::enable_device(&device);
    let mmio = match vmm::map_kernel_mmio(bar, 0x10000) {
        Ok(address) => address,
        Err(error) => {
            crate::warn!("xhci: cannot map BAR0: {}", error);
            return false;
        }
    };
    let caplength = unsafe { ((mmio) as *const u8).read_volatile() } as u64;
    let op = mmio + caplength;
    let dboff = read32(mmio + 0x14) as u64;
    let rtsoff = read32(mmio + 0x18) as u64;
    let hcsparams1 = read32(mmio + 0x04);
    let max_slots = (hcsparams1 & 0xFF) as u8;
    let max_ports = ((hcsparams1 >> 24) & 0xFF) as u8;
    let hccparams1 = read32(mmio + 0x10);
    let csz = if hccparams1 & (1 << 2) != 0 { 64 } else { 32 };
    let doorbell = mmio + dboff;
    let runtime = mmio + rtsoff;

    // Reset the controller.
    write32(op + REG_USBCMD, read32(op + REG_USBCMD) & !CMD_RUN);
    write32(op + REG_USBCMD, read32(op + REG_USBCMD) | CMD_RESET);
    for _ in 0..2_000_000usize {
        if read32(op + REG_USBCMD) & CMD_RESET == 0 && read32(op + REG_USBSTS) & STS_CNR == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    let Some((cmd_phys, cmd_va)) = dma_page() else {
        return false;
    };
    let Some((evt_phys, evt_va)) = dma_page() else {
        return false;
    };
    let Some((erst_phys, erst_va)) = dma_page() else {
        return false;
    };
    let Some((dcbaa_phys, dcbaa_va)) = dma_page() else {
        return false;
    };
    let Some((input_phys, input_va)) = dma_page() else {
        return false;
    };
    let Some((control_phys, control_va)) = dma_page() else {
        return false;
    };
    write64(erst_va, evt_phys);
    write32(erst_va + 8, EVT_TRBS as u32);
    write32(erst_va + 12, 0);
    write64(dcbaa_va, 0);

    write64(op + REG_CRCR, cmd_phys | 1);
    write64(op + REG_DCBAAP, dcbaa_phys);
    write32(op + REG_CONFIG, max_slots as u32);
    write64(runtime + 0x20 + 0x08, 1);
    write64(runtime + 0x20 + 0x10, erst_phys);
    write64(runtime + 0x20 + 0x18, evt_phys | (1 << 3));
    write32(runtime + 0x20 + 0x00, 1);

    write32(op + REG_USBCMD, CMD_RUN);
    for _ in 0..2_000_000usize {
        if read32(op + REG_USBSTS) & STS_HALT == 0 {
            break;
        }
        core::hint::spin_loop();
    }

    let controller = Controller {
        mmio,
        op,
        runtime,
        doorbell,
        max_slots,
        max_ports,
        cmd_phys,
        cmd_va,
        cmd_index: 0,
        cmd_cycle: 1,
        evt_phys,
        evt_va,
        evt_index: 0,
        evt_cycle: 1,
        erst_phys,
        erst_va,
        dcbaa_phys,
        dcbaa_va,
        input_phys,
        input_va,
        control_phys,
        control_va,
        devices: [None; MAX_DEVICES],
        connected: [false; MAX_PORTS],
    };
    *CONTROLLER.lock() = Some(controller);
    crate::info!(
        "xhci: controller {:04x}:{:04x} at {:#x}, {} slots, {} ports, CSZ {}",
        device.vendor,
        device.id,
        bar,
        max_slots,
        max_ports,
        csz
    );
    let mut guard = CONTROLLER.lock();
    if let Some(controller) = guard.as_mut() {
        poll(controller);
    }
    drop(guard);
    crate::sched::create_kernel_thread("usb", usb_thread);
    true
}

extern "C" fn usb_thread() {
    loop {
        {
            let mut guard = CONTROLLER.lock();
            if let Some(controller) = guard.as_mut() {
                poll(controller);
            }
        }
        crate::sched::sleep_ms(10);
    }
}
