//! Native IPC: bidirectional message channels with per-process handles.
//!
//! A channel has two endpoints. Each endpoint has a bounded FIFO of inline
//! messages. `CHANNEL_CREATE` installs both endpoints in the caller's handle
//! table and returns their indices; a process can lend one endpoint to a child
//! through `SPAWN`. Handles are process-local indices, never global pointers.
//!
//! Blocking rules:
//!   - `send` blocks while the peer's queue is full,
//!   - `recv` blocks while the local queue is empty and the peer is open,
//!   - close/exit wakes all waiters, which re-check and return `EPIPE`.
//!
//! All argument buffers are validated by the syscall layer before they reach
//! this module. Message payloads are copied inline; no kernel pointer is ever
//! exposed to user space.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use crate::sched;
use crate::sync::IrqLock;

pub const MAX_CHANNELS: usize = 32;
pub const MAX_MESSAGES: usize = 8;
pub const MAX_PAYLOAD: usize = 256;
pub const HANDLE_BASE: usize = 3;

const HANDLE_SLOTS: usize = 16;

struct Message {
    sequence: u64,
    len: usize,
    data: [u8; MAX_PAYLOAD],
}

struct Endpoint {
    queue: VecDeque<Message>,
    closed: bool,
    refs: u32,
    recv_waiters: Vec<u64>,
    send_waiters: Vec<u64>,
}

impl Endpoint {
    const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            closed: false,
            refs: 1,
            recv_waiters: Vec::new(),
            send_waiters: Vec::new(),
        }
    }
}

struct Channel {
    id: u64,
    active: bool,
    next_sequence: u64,
    endpoints: [Endpoint; 2],
}

struct Table {
    channels: Vec<Channel>,
    next_id: u64,
}

static CHANNELS: IrqLock<Table> = IrqLock::new(Table {
    channels: Vec::new(),
    next_id: 1,
});

fn decode(endpoint: u64) -> (u64, usize) {
    (endpoint >> 1, (endpoint & 1) as usize)
}

fn encode(channel: u64, endpoint: usize) -> u64 {
    (channel << 1) | endpoint as u64
}

/// Endpoint referenced by a process-local handle, if any.
pub fn endpoint_for_handle(process: *mut sched::Process, handle: u32) -> Option<u64> {
    if process.is_null() {
        return None;
    }
    let process = unsafe { &*process };
    process.channels.get(handle as usize).and_then(|slot| *slot)
}

fn free_slot(process: &sched::Process) -> Option<usize> {
    for index in HANDLE_BASE..HANDLE_SLOTS {
        if process
            .channels
            .get(index)
            .map(|slot| slot.is_none())
            .unwrap_or(false)
        {
            return Some(index);
        }
    }
    None
}

fn wake_all(tids: &mut Vec<u64>) {
    for tid in tids.drain(..) {
        sched::wake_tid(tid);
    }
}

/// Create a channel and install both endpoint handles in `process`.
/// Writes the two handle indices as `u32` pairs to `out`.
pub fn create(process: *mut sched::Process, out: *mut u8) -> Result<(), i64> {
    use aster_abi::errno;
    if process.is_null() || out.is_null() {
        return Err(errno::EINVAL);
    }
    let process = unsafe { &mut *process };
    let first = free_slot(process).ok_or(errno::EBADF)?;
    // Temporarily reserve two slots; roll back if the second is unavailable.
    process.channels[first] = Some(u64::MAX);
    let second = match free_slot(process) {
        Some(slot) => slot,
        None => {
            process.channels[first] = None;
            return Err(errno::EBADF);
        }
    };

    let mut table = CHANNELS.lock();
    let channel_id = table.next_id;
    table.next_id += 1;
    let channel = Channel {
        id: channel_id,
        active: true,
        next_sequence: 0,
        endpoints: [Endpoint::new(), Endpoint::new()],
    };
    let slot = table.channels.iter().position(|c| !c.active);
    match slot {
        Some(index) => table.channels[index] = channel,
        None if table.channels.len() < MAX_CHANNELS => table.channels.push(channel),
        None => {
            process.channels[first] = None;
            return Err(errno::ENOSPC);
        }
    }
    drop(table);

    process.channels[first] = Some(encode(channel_id, 0));
    process.channels[second] = Some(encode(channel_id, 1));
    let handles = [first as u32, second as u32];
    let bytes = unsafe {
        core::slice::from_raw_parts(
            handles.as_ptr() as *const u8,
            core::mem::size_of_val(&handles),
        )
    };
    crate::mm::vmm::copy_to_user(process.pml4, out as u64, bytes).map_err(|_| errno::EINVAL)?;
    Ok(())
}

/// Send one inline message. Blocks while the peer queue is full.
pub fn send(process: *mut sched::Process, handle: u32, data: &[u8]) -> Result<usize, i64> {
    use aster_abi::errno;
    if data.len() > MAX_PAYLOAD {
        return Err(errno::EINVAL);
    }
    let len = data.len();
    loop {
        let endpoint = endpoint_for_handle(process, handle).ok_or(errno::EBADF)?;
        let (channel_id, source) = decode(endpoint);
        let destination = 1 - source;
        let mut table = CHANNELS.lock();
        let channel = table
            .channels
            .iter_mut()
            .find(|channel| channel.id == channel_id && channel.active)
            .ok_or(errno::EBADF)?;
        if channel.endpoints[destination].closed {
            return Err(errno::EPIPE);
        }
        if channel.endpoints[destination].queue.len() >= MAX_MESSAGES {
            let tid = sched::current_tid();
            if !channel.endpoints[destination].send_waiters.contains(&tid) {
                channel.endpoints[destination].send_waiters.push(tid);
            }
            drop(table);
            sched::block_current();
            continue;
        }

        let mut message = Message {
            sequence: channel.next_sequence,
            len,
            data: [0; MAX_PAYLOAD],
        };
        channel.next_sequence += 1;
        message.data[..len].copy_from_slice(data);
        channel.endpoints[destination].queue.push_back(message);
        let waiter = channel.endpoints[destination].recv_waiters.pop();
        drop(table);
        if let Some(tid) = waiter {
            sched::wake_tid(tid);
        }
        return Ok(len);
    }
}

/// Receive one inline message. Blocks while the queue is empty.
pub fn recv(
    process: *mut sched::Process,
    handle: u32,
    buffer: &mut [u8],
    timeout_ms: u64,
) -> Result<usize, i64> {
    use aster_abi::errno;
    let deadline = if timeout_ms == 0 {
        u64::MAX
    } else {
        crate::time::uptime_ms().saturating_add(timeout_ms)
    };
    loop {
        let endpoint = endpoint_for_handle(process, handle).ok_or(errno::EBADF)?;
        let (channel_id, local) = decode(endpoint);
        let peer = 1 - local;
        let mut table = CHANNELS.lock();
        let channel = table
            .channels
            .iter_mut()
            .find(|channel| channel.id == channel_id && channel.active)
            .ok_or(errno::EBADF)?;

        if let Some(front) = channel.endpoints[local].queue.front() {
            if front.len > buffer.len() {
                return Err(errno::EINVAL);
            }
        }
        if let Some(message) = channel.endpoints[local].queue.pop_front() {
            let waiter = channel.endpoints[local].send_waiters.pop();
            let len = message.len;
            buffer[..len].copy_from_slice(&message.data[..len]);
            drop(table);
            if let Some(tid) = waiter {
                sched::wake_tid(tid);
            }
            return Ok(len);
        }

        if channel.endpoints[peer].closed {
            return Err(errno::EPIPE);
        }

        let now = crate::time::uptime_ms();
        if deadline != u64::MAX && now >= deadline {
            return Err(errno::ETIMEDOUT);
        }

        let tid = sched::current_tid();
        if !channel.endpoints[local].recv_waiters.contains(&tid) {
            channel.endpoints[local].recv_waiters.push(tid);
        }
        drop(table);
        sched::block_current_until(deadline);

        // A timeout wake leaves the tid in the queue; remove it before the next
        // check so a later send does not wake a thread that is no longer here.
        if deadline != u64::MAX && crate::time::uptime_ms() >= deadline {
            let mut table = CHANNELS.lock();
            if let Some(channel) = table
                .channels
                .iter_mut()
                .find(|channel| channel.id == channel_id && channel.active)
            {
                channel.endpoints[local]
                    .recv_waiters
                    .retain(|&waiter| waiter != tid);
            }
        }
    }
}

/// Close a process handle and release its reference to the endpoint.
pub fn close(process: *mut sched::Process, handle: u32) -> Result<(), i64> {
    use aster_abi::errno;
    if process.is_null() {
        return Err(errno::EINVAL);
    }
    let process = unsafe { &mut *process };
    let endpoint = process
        .channels
        .get_mut(handle as usize)
        .and_then(|slot| slot.take());
    match endpoint {
        Some(endpoint) => {
            release_endpoint(endpoint);
            Ok(())
        }
        None => Err(errno::EBADF),
    }
}

/// Drop one reference to an endpoint. When the last reference disappears the
/// endpoint is marked closed and every waiter is woken to re-check.
pub fn release_endpoint(endpoint: u64) {
    let (channel_id, index) = decode(endpoint);
    let mut wake: Vec<u64> = Vec::new();
    let mut table = CHANNELS.lock();
    if let Some(channel) = table
        .channels
        .iter_mut()
        .find(|channel| channel.id == channel_id && channel.active)
    {
        let target = &mut channel.endpoints[index];
        if target.refs > 0 {
            target.refs -= 1;
        }
        if target.refs == 0 && !target.closed {
            target.closed = true;
            wake.append(&mut channel.endpoints[0].recv_waiters);
            wake.append(&mut channel.endpoints[0].send_waiters);
            wake.append(&mut channel.endpoints[1].recv_waiters);
            wake.append(&mut channel.endpoints[1].send_waiters);
        }
        if channel.endpoints[0].closed && channel.endpoints[1].closed {
            channel.endpoints[0].queue.clear();
            channel.endpoints[1].queue.clear();
            channel.active = false;
        }
    }
    drop(table);
    wake_all(&mut wake);
}

/// Duplicate an endpoint into a freshly spawned child at its first free slot.
pub fn install_in_child(child_pid: u64, endpoint: u64) -> Result<u32, i64> {
    use aster_abi::errno;
    let child = sched::process_by_pid(child_pid);
    if child.is_null() {
        return Err(errno::ESRCH);
    }
    let child = unsafe { &mut *child };
    let slot = free_slot(child).ok_or(errno::EBADF)?;
    let (channel_id, index) = decode(endpoint);
    {
        let mut table = CHANNELS.lock();
        let channel = table
            .channels
            .iter_mut()
            .find(|channel| channel.id == channel_id && channel.active)
            .ok_or(errno::EBADF)?;
        if channel.endpoints[index].closed {
            return Err(errno::EPIPE);
        }
        channel.endpoints[index].refs += 1;
    }
    child.channels[slot] = Some(endpoint);
    Ok(slot as u32)
}
