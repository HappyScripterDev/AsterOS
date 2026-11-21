//! System call dispatch (`int 0x80`).
//!
//! Argument registers follow the platform ABI:
//!   rax = number, rdi, rsi, rdx, r10, r8, r9 = arguments.
//! Return value: rax, negative `errno` values on failure.
//!
//! Every user pointer is validated through the process page tables before the
//! kernel reads or writes it. Syscalls run with interrupts disabled (interrupt
//! gate), so blocking calls must yield explicitly.

use aster_abi::{sys, FramebufferInfo, NetConfig, NetStatus, SystemInfo};

use crate::arch::idt::InterruptFrame;
use crate::mm::vmm;
use crate::sched;
use crate::{boot, logging, time};

fn error(code: i64) -> u64 {
    code as u64
}

pub fn handle(frame: &mut InterruptFrame) {
    let number = frame.rax;
    let args = [
        frame.rdi, frame.rsi, frame.rdx, frame.r10, frame.r8, frame.r9,
    ];
    let process = sched::current_process();
    if process.is_null() {
        frame.rax = error(errno::EPERM);
        return;
    }
    let pml4 = unsafe { (*process).pml4 };

    let result = match number {
        sys::EXIT => {
            sched::exit_current(args[0] as i64);
        }
        sys::WRITE => write_console(pml4, args[0] as u32, args[1], args[2] as usize),
        sys::GETPID => sched::current_pid() as u64,
        sys::YIELD => {
            sched::yield_now();
            0
        }
        sys::SLEEP_MS => {
            sched::sleep_ms(args[0]);
            0
        }
        sys::TIME_MS => time::uptime_ms(),
        sys::FB_INFO => framebuffer_info(pml4, args[0]),
        sys::FB_MAP => framebuffer_map(pml4, process),
        sys::INPUT_POLL => crate::drivers::ps2::poll(args[0], args[1] as usize, pml4),
        sys::VM_ALLOC => vm_alloc(pml4, process, args[0] as usize),
        sys::LOG => write_log(pml4, args[0], args[1] as usize),
        sys::SYSTEM_INFO => system_info(pml4, args[0]),
        sys::OPEN => open_file(pml4, process, args[0], args[1] as u32),
        sys::READ => read_file(pml4, process, args[0] as u32, args[1], args[2] as usize),
        sys::WRITE_FD => write_file(pml4, process, args[0] as u32, args[1], args[2] as usize),
        sys::CLOSE => close_file(process, args[0] as u32),
        sys::SEEK => seek_file(process, args[0] as u32, args[1] as i64, args[2] as u32),
        sys::READDIR => read_directory(pml4, process, args[0] as u32, args[1], args[2] as usize),
        sys::STAT => stat_path(pml4, args[0], args[1]),
        sys::UNLINK => unlink_path(pml4, args[0]),
        sys::MKDIR => mkdir_path(pml4, args[0]),
        sys::SPAWN => spawn_process(pml4, process, args[0], args[1]),
        sys::WAIT => wait_process(pml4, args[0], args[1]),
        sys::CHANNEL_CREATE => channel_create(process, args[0]),
        sys::CHANNEL_SEND => channel_send(pml4, process, args[0] as u32, args[1], args[2] as usize),
        sys::CHANNEL_RECV => channel_recv(
            pml4,
            process,
            args[0] as u32,
            args[1],
            args[2] as usize,
            args[3],
        ),
        sys::CHANNEL_CLOSE => match crate::ipc::close(process, args[0] as u32) {
            Ok(()) => 0,
            Err(code) => error(code),
        },
        sys::RTC => rtc_time(pml4, args[0]),
        sys::PROC_LIST => process_list(pml4, args[0], args[1] as usize),
        sys::STORAGE_INFO => storage_info(pml4, args[0]),
        sys::STORAGE_DETAILS => storage_details(pml4, args[0]),
        sys::RENAME => rename_path(pml4, args[0], args[1]),
        sys::LOG_READ => log_read(pml4, args[0], args[1] as usize),
        sys::SYNC => sync_storage(),
        sys::SHUTDOWN => shutdown(),
        sys::NET_STATUS => net_status(pml4, args[0]),
        sys::NET_CONFIG => net_config(pml4, args[0]),
        sys::NET_PING => net_ping(pml4, args[0], args[1], args[2]),
        sys::NET_DNS => net_dns(pml4, args[0], args[1] as usize, args[2], args[3]),
        sys::SOCKET => socket_open(process, args[0]),
        sys::BIND => socket_bind(process, args[0], args[1] as u16),
        sys::CONNECT => socket_connect(pml4, process, args[0], args[1], args[2] as u16, args[3]),
        sys::SEND => socket_send(pml4, process, args[0], args[1], args[2] as usize),
        sys::RECV => socket_recv(pml4, process, args[0], args[1], args[2] as usize, args[3]),
        sys::SOCKET_CLOSE => match crate::net::socket_close(unsafe { (*process).pid }, args[0]) {
            0 => 0,
            code => error(code),
        },
        sys::POWER => power(args[0]),
        sys::DEVICES => devices(pml4, args[0], args[1] as usize),
        _ => error(errno::ENOSYS),
    };
    frame.rax = result;
}

fn write_console(pml4: u64, fd: u32, pointer: u64, length: usize) -> u64 {
    if fd != 1 && fd != 2 {
        return error(errno::EBADF);
    }
    let length = length.min(8192);
    if length == 0 {
        return 0;
    }
    let mut buffer = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, pointer, &mut buffer).is_err() {
        return error(errno::EINVAL);
    }
    logging::write_raw(&buffer);
    length as u64
}

fn write_log(pml4: u64, pointer: u64, length: usize) -> u64 {
    let length = length.min(4096);
    if length == 0 {
        return 0;
    }
    let mut buffer = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, pointer, &mut buffer).is_err() {
        return error(errno::EINVAL);
    }
    logging::write_raw(b"[user] ");
    logging::write_raw(&buffer);
    logging::write_raw(b"\n");
    length as u64
}

fn framebuffer_info(pml4: u64, pointer: u64) -> u64 {
    let info: FramebufferInfo = boot::info().framebuffer;
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const FramebufferInfo as *const u8,
            core::mem::size_of::<FramebufferInfo>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    core::mem::size_of::<FramebufferInfo>() as u64
}

fn framebuffer_map(pml4: u64, process: *mut sched::Process) -> u64 {
    unsafe {
        if (*process).fb_va != 0 {
            return (*process).fb_va;
        }
        let info = boot::info().framebuffer;
        if info.address == 0 || info.size == 0 {
            return error(errno::EIO);
        }
        match vmm::map_framebuffer(pml4, info.address, info.size) {
            Ok(address) => {
                (*process).fb_va = address;
                address
            }
            Err(e) => {
                crate::error!("fb_map failed: {}", e);
                error(errno::ENOMEM)
            }
        }
    }
}

fn vm_alloc(pml4: u64, process: *mut sched::Process, length: usize) -> u64 {
    unsafe {
        let cursor = &mut (*process).heap_cursor;
        match vmm::user_alloc(pml4, cursor, length, vmm::PTE_WRITE | vmm::PTE_NX) {
            Ok(address) => address,
            Err(e) => {
                crate::warn!("vm_alloc({}) failed: {}", length, e);
                error(errno::ENOMEM)
            }
        }
    }
}

fn system_info(pml4: u64, pointer: u64) -> u64 {
    let (threads, processes, switches) = sched::counts();
    let (total_kib, free_kib) = crate::mm::pmm::stats();
    let (_, heap_used) = crate::mm::heap::stats_kib();
    let info = SystemInfo {
        uptime_ms: time::uptime_ms(),
        total_memory_kib: total_kib,
        free_memory_kib: free_kib,
        kernel_heap_used_kib: heap_used,
        thread_count: threads,
        process_count: processes,
        cpu_count: 1,
        timer_hz: time::TIMER_HZ,
        context_switches: switches,
        ticks: time::uptime_ticks(),
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const SystemInfo as *const u8,
            core::mem::size_of::<SystemInfo>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    core::mem::size_of::<SystemInfo>() as u64
}

pub use aster_abi::errno;

fn read_path(pml4: u64, pointer: u64) -> Result<alloc::string::String, i64> {
    if pointer == 0 {
        return Err(errno::EINVAL);
    }
    // Read one byte at a time so a path that ends at the last mapped byte of a
    // page is still valid. A fixed-size copy would reject it.
    let mut buffer = [0u8; 256];
    let mut length = 0usize;
    while length < buffer.len() {
        if vmm::copy_from_user(
            pml4,
            pointer + length as u64,
            &mut buffer[length..length + 1],
        )
        .is_err()
        {
            return Err(errno::EINVAL);
        }
        if buffer[length] == 0 {
            return alloc::string::String::from_utf8(buffer[..length].to_vec())
                .map_err(|_| errno::EINVAL);
        }
        length += 1;
    }
    Err(errno::ENAMETOOLONG)
}

unsafe fn fd_slot(
    process: *mut sched::Process,
    fd: u32,
) -> Option<&'static mut crate::fs::OpenFile> {
    let fds = &mut (*process).fds;
    fds.get_mut(fd as usize).and_then(|slot| slot.as_mut())
}

fn open_file(pml4: u64, process: *mut sched::Process, path_pointer: u64, flags: u32) -> u64 {
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    let inode = match crate::fs::open(&path, flags) {
        Ok(inode) => inode,
        Err(code) => return error(code),
    };
    unsafe {
        let fds = &mut (*process).fds;
        for index in 3..crate::fs::MAX_FDS {
            if fds[index].is_none() {
                fds[index] = Some(crate::fs::OpenFile {
                    inode,
                    offset: 0,
                    flags,
                    dir_index: 0,
                });
                return index as u64;
            }
        }
    }
    error(errno::EBADF)
}

fn read_file(pml4: u64, process: *mut sched::Process, fd: u32, pointer: u64, length: usize) -> u64 {
    let file = unsafe {
        match fd_slot(process, fd) {
            Some(file) => file,
            None => return error(errno::EBADF),
        }
    };
    let length = length.min(1024 * 1024);
    let data = match crate::fs::read(file.inode, file.offset, length) {
        Ok(data) => data,
        Err(code) => return error(code),
    };
    if vmm::copy_to_user(pml4, pointer, &data).is_err() {
        return error(errno::EINVAL);
    }
    file.offset += data.len() as u64;
    data.len() as u64
}

fn write_file(
    pml4: u64,
    process: *mut sched::Process,
    fd: u32,
    pointer: u64,
    length: usize,
) -> u64 {
    let file = unsafe {
        match fd_slot(process, fd) {
            Some(file) => file,
            None => return error(errno::EBADF),
        }
    };
    if file.flags & aster_abi::open_flags::WRITE == 0 {
        return error(errno::EPERM);
    }
    let length = length.min(1024 * 1024);
    let mut buffer = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, pointer, &mut buffer).is_err() {
        return error(errno::EINVAL);
    }
    match crate::fs::write(file.inode, file.offset, &buffer) {
        Ok(written) => {
            file.offset += written as u64;
            written as u64
        }
        Err(code) => error(code),
    }
}

fn close_file(process: *mut sched::Process, fd: u32) -> u64 {
    if fd < 3 {
        return 0;
    }
    unsafe {
        let fds: &mut alloc::vec::Vec<Option<crate::fs::OpenFile>> = &mut (*process).fds;
        if let Some(slot) = fds.get_mut(fd as usize) {
            *slot = None;
            return 0;
        }
    }
    error(errno::EBADF)
}

fn seek_file(process: *mut sched::Process, fd: u32, offset: i64, whence: u32) -> u64 {
    let file = unsafe {
        match fd_slot(process, fd) {
            Some(file) => file,
            None => return error(errno::EBADF),
        }
    };
    let base = match whence {
        0 => 0i64,
        1 => file.offset as i64,
        2 => crate::fs::size(file.inode).unwrap_or(0) as i64,
        _ => return error(errno::EINVAL),
    };
    let new_offset = base + offset;
    if new_offset < 0 {
        return error(errno::EINVAL);
    }
    file.offset = new_offset as u64;
    file.offset
}

fn read_directory(
    pml4: u64,
    process: *mut sched::Process,
    fd: u32,
    pointer: u64,
    maximum: usize,
) -> u64 {
    let file = unsafe {
        match fd_slot(process, fd) {
            Some(file) => file,
            None => return error(errno::EBADF),
        }
    };
    let maximum = maximum.min(64);
    let items = match crate::fs::list(file.inode, file.dir_index, maximum) {
        Ok(items) => items,
        Err(code) => return error(code),
    };
    let entry_size = core::mem::size_of::<aster_abi::DirEntry>();
    let mut buffer = alloc::vec![0u8; items.len() * entry_size];
    for (index, item) in items.iter().enumerate() {
        let mut entry = aster_abi::DirEntry {
            inode: item.inode,
            ty: if item.kind == crate::fs::NodeKind::Dir {
                1
            } else {
                0
            },
            name_len: item.name.len() as u32,
            name: [0; aster_abi::DIRENT_NAME_LEN],
        };
        for (i, byte) in item
            .name
            .as_bytes()
            .iter()
            .take(aster_abi::DIRENT_NAME_LEN)
            .enumerate()
        {
            entry.name[i] = *byte;
        }
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &entry as *const aster_abi::DirEntry as *const u8,
                entry_size,
            )
        };
        buffer[index * entry_size..(index + 1) * entry_size].copy_from_slice(bytes);
    }
    if vmm::copy_to_user(pml4, pointer, &buffer).is_err() {
        return error(errno::EINVAL);
    }
    file.dir_index += items.len();
    items.len() as u64
}

fn stat_path(pml4: u64, path_pointer: u64, stat_pointer: u64) -> u64 {
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    let stat = match crate::fs::stat_path(&path) {
        Ok(stat) => stat,
        Err(code) => return error(code),
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &stat as *const aster_abi::FileStat as *const u8,
            core::mem::size_of::<aster_abi::FileStat>(),
        )
    };
    if vmm::copy_to_user(pml4, stat_pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    0
}

fn unlink_path(pml4: u64, path_pointer: u64) -> u64 {
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    match crate::fs::unlink(&path) {
        Ok(()) => 0,
        Err(code) => error(code),
    }
}

fn mkdir_path(pml4: u64, path_pointer: u64) -> u64 {
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    match crate::fs::mkdir(&path) {
        Ok(_) => 0,
        Err(code) => error(code),
    }
}

fn rtc_time(pml4: u64, pointer: u64) -> u64 {
    let time = crate::arch::rtc::read();
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &time as *const aster_abi::RtcTime as *const u8,
            core::mem::size_of::<aster_abi::RtcTime>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    0
}

fn process_list(pml4: u64, pointer: u64, maximum: usize) -> u64 {
    let maximum = maximum.min(64);
    let mut entries = alloc::vec![aster_abi::ProcessInfo {
        pid: 0,
        threads: 0,
        state: 0,
        name: [0; 24],
    }; maximum];
    let count = sched::list_processes(&mut entries);
    let bytes = unsafe {
        core::slice::from_raw_parts(
            entries.as_ptr() as *const u8,
            count * core::mem::size_of::<aster_abi::ProcessInfo>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    count as u64
}

fn spawn_process(
    pml4: u64,
    process: *mut sched::Process,
    path_pointer: u64,
    bootstrap: u64,
) -> u64 {
    if sched::process_count() >= 24 {
        return error(errno::EPERM);
    }
    let path = match read_path(pml4, path_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    let endpoint = if bootstrap != 0 {
        match crate::ipc::endpoint_for_handle(process, bootstrap as u32) {
            Some(endpoint) => Some(endpoint),
            None => return error(errno::EBADF),
        }
    } else {
        None
    };
    match crate::user::spawn_path(&path, sched::current_pid()) {
        Ok(pid) => {
            if let Some(endpoint) = endpoint {
                if let Err(code) = crate::ipc::install_in_child(pid, endpoint) {
                    return error(code);
                }
            }
            pid
        }
        Err(code) => error(code),
    }
}

fn channel_create(process: *mut sched::Process, out: u64) -> u64 {
    match crate::ipc::create(process, out as *mut u8) {
        Ok(()) => 0,
        Err(code) => error(code),
    }
}

fn channel_send(
    pml4: u64,
    process: *mut sched::Process,
    handle: u32,
    pointer: u64,
    length: usize,
) -> u64 {
    if length > crate::ipc::MAX_PAYLOAD {
        return error(errno::EINVAL);
    }
    let mut buffer = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, pointer, &mut buffer).is_err() {
        return error(errno::EINVAL);
    }
    match crate::ipc::send(process, handle, &buffer) {
        Ok(written) => written as u64,
        Err(code) => error(code),
    }
}

fn channel_recv(
    pml4: u64,
    process: *mut sched::Process,
    handle: u32,
    pointer: u64,
    capacity: usize,
    timeout_ms: u64,
) -> u64 {
    let capacity = capacity.min(crate::ipc::MAX_PAYLOAD);
    if capacity == 0 {
        return error(errno::EINVAL);
    }
    let mut buffer = alloc::vec![0u8; capacity];
    match crate::ipc::recv(process, handle, &mut buffer, timeout_ms) {
        Ok(length) => {
            if vmm::copy_to_user(pml4, pointer, &buffer[..length]).is_err() {
                return error(errno::EINVAL);
            }
            length as u64
        }
        Err(code) => error(code),
    }
}

fn wait_process(pml4: u64, pid: u64, status_pointer: u64) -> u64 {
    if status_pointer == 0 {
        return error(errno::EINVAL);
    }
    match sched::wait_child(pid) {
        Ok((child, status)) => {
            if vmm::copy_to_user(pml4, status_pointer, &status.to_le_bytes()).is_err() {
                return error(errno::EINVAL);
            }
            child
        }
        Err(code) => error(code),
    }
}

fn storage_info(pml4: u64, pointer: u64) -> u64 {
    let info = match crate::storage::fat32::usage() {
        Some(usage) => aster_abi::StorageInfo {
            mounted: 1,
            sector_bytes: 512,
            cluster_bytes: usage.cluster_bytes,
            reserved: 0,
            total_bytes: usage.total_bytes,
            free_bytes: usage.free_bytes,
        },
        None => aster_abi::StorageInfo {
            mounted: 0,
            sector_bytes: 512,
            cluster_bytes: 0,
            reserved: 0,
            total_bytes: 0,
            free_bytes: 0,
        },
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const aster_abi::StorageInfo as *const u8,
            core::mem::size_of::<aster_abi::StorageInfo>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    0
}

fn storage_details(pml4: u64, pointer: u64) -> u64 {
    let details = crate::storage::details();
    let info = aster_abi::StorageDetails {
        mounted: details.mounted as u32,
        healthy: details.healthy as u32,
        filesystem: details.filesystem,
        read_only: details.read_only as u32,
        sector_bytes: details.sector_size,
        cluster_bytes: details.cluster_bytes,
        device: details.device,
        reserved: 0,
        total_bytes: details.total_bytes,
        free_bytes: details.free_bytes,
        device_sectors: details.device_sectors,
        partition_start: details.partition_start,
        partition_sectors: details.partition_sectors,
        sectors_read: details.stats.sectors_read,
        sectors_written: details.stats.sectors_written,
        reads: details.stats.reads,
        writes: details.stats.writes,
        flushes: details.stats.flushes,
        errors: details.stats.errors,
        label: details.label,
        reserved2: 0,
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const aster_abi::StorageDetails as *const u8,
            core::mem::size_of::<aster_abi::StorageDetails>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    0
}

fn rename_path(pml4: u64, old_pointer: u64, new_pointer: u64) -> u64 {
    let old_path = match read_path(pml4, old_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    let new_path = match read_path(pml4, new_pointer) {
        Ok(path) => path,
        Err(code) => return error(code),
    };
    match crate::fs::rename(&old_path, &new_path) {
        Ok(()) => 0,
        Err(code) => error(code),
    }
}

fn log_read(pml4: u64, pointer: u64, capacity: usize) -> u64 {
    let capacity = capacity.min(16 * 1024);
    if capacity == 0 {
        return 0;
    }
    let mut buffer = alloc::vec![0u8; capacity];
    let count = logging::read_log(&mut buffer);
    if vmm::copy_to_user(pml4, pointer, &buffer[..count]).is_err() {
        return error(errno::EINVAL);
    }
    count as u64
}

fn sync_storage() -> u64 {
    match crate::fs::sync() {
        code if code < 0 => error(code),
        _ => 0,
    }
}

fn shutdown() -> u64 {
    crate::info!("shutdown requested by user space (legacy syscall)");
    crate::arch::acpi::power(0);
}

fn devices(pml4: u64, out: u64, max: usize) -> u64 {
    let max = max.min(32);
    if max == 0 || out == 0 {
        return error(errno::EINVAL);
    }
    let mut list = [aster_abi::DeviceInfo::EMPTY; 32];
    let count = crate::drivers::pci::enumerate(&mut list).min(max);
    let bytes = unsafe {
        core::slice::from_raw_parts(
            list.as_ptr() as *const u8,
            count * core::mem::size_of::<aster_abi::DeviceInfo>(),
        )
    };
    if vmm::copy_to_user(pml4, out, bytes).is_err() {
        return error(errno::EINVAL);
    }
    count as u64
}

fn power(mode: u64) -> u64 {
    crate::info!("power requested by user space: mode {}", mode);
    crate::arch::acpi::power(mode)
}

// ---------------------------------------------------------------------------
// Networking
// ---------------------------------------------------------------------------

fn read_ipv4(pml4: u64, pointer: u64) -> Result<[u8; 4], i64> {
    let mut address = [0u8; 4];
    vmm::copy_from_user(pml4, pointer, &mut address).map_err(|_| aster_abi::errno::EINVAL)?;
    Ok(address)
}

fn net_status(pml4: u64, pointer: u64) -> u64 {
    let status = crate::net::status();
    let info = NetStatus {
        rx_packets: status.rx_packets,
        tx_packets: status.tx_packets,
        rx_bytes: status.rx_bytes,
        tx_bytes: status.tx_bytes,
        rx_errors: status.rx_errors,
        tx_errors: status.tx_errors,
        dropped: status.dropped,
        mac: status.mac,
        present: status.present as u8,
        link_up: status.link_up as u8,
        configured: status.configured as u8,
        dhcp_state: status.dhcp_state,
        ip: status.ip,
        mask: status.mask,
        gateway: status.gateway,
        dns: status.dns,
        mtu: status.mtu as u32,
        lease_remaining: status.lease_remaining,
    };
    let bytes = unsafe {
        core::slice::from_raw_parts(
            &info as *const NetStatus as *const u8,
            core::mem::size_of::<NetStatus>(),
        )
    };
    if vmm::copy_to_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    core::mem::size_of::<NetStatus>() as u64
}

fn net_config(pml4: u64, pointer: u64) -> u64 {
    let mut config = NetConfig::dhcp();
    let bytes = unsafe {
        core::slice::from_raw_parts_mut(
            &mut config as *mut NetConfig as *mut u8,
            core::mem::size_of::<NetConfig>(),
        )
    };
    if vmm::copy_from_user(pml4, pointer, bytes).is_err() {
        return error(errno::EINVAL);
    }
    let result = if config.mode == 0 {
        crate::net::renew()
    } else if config.mode == 1 {
        crate::net::set_static(config.ip, config.mask, config.gateway, config.dns)
    } else {
        errno::EINVAL
    };
    if result < 0 {
        error(result)
    } else {
        0
    }
}

fn net_ping(pml4: u64, destination: u64, timeout_ms: u64, rtt_out: u64) -> u64 {
    let destination = match read_ipv4(pml4, destination) {
        Ok(address) => address,
        Err(code) => return error(code),
    };
    let timeout = timeout_ms.clamp(100, 30_000);
    match crate::net::ping(destination, timeout) {
        Ok(rtt) => {
            if vmm::copy_to_user(pml4, rtt_out, &rtt.to_le_bytes()).is_err() {
                return error(errno::EINVAL);
            }
            0
        }
        Err(code) => error(code),
    }
}

fn net_dns(pml4: u64, name_pointer: u64, name_length: usize, out: u64, timeout_ms: u64) -> u64 {
    let length = name_length.min(63);
    if length == 0 {
        return error(errno::EINVAL);
    }
    let mut buffer = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, name_pointer, &mut buffer).is_err() {
        return error(errno::EINVAL);
    }
    let Ok(name) = core::str::from_utf8(&buffer) else {
        return error(errno::EINVAL);
    };
    let timeout = timeout_ms.clamp(100, 30_000);
    match crate::net::resolve(name, timeout) {
        Ok(address) => {
            if vmm::copy_to_user(pml4, out, &address).is_err() {
                return error(errno::EINVAL);
            }
            0
        }
        Err(code) => error(code),
    }
}

fn socket_open(process: *mut sched::Process, kind: u64) -> u64 {
    let owner = unsafe { (*process).pid };
    match crate::net::socket_open(owner, kind as u8) {
        Ok(id) => id,
        Err(code) => error(code),
    }
}

fn socket_bind(process: *mut sched::Process, id: u64, port: u16) -> u64 {
    let owner = unsafe { (*process).pid };
    match crate::net::socket_bind(owner, id, port) {
        Ok(()) => 0,
        Err(code) => error(code),
    }
}

fn socket_connect(
    pml4: u64,
    process: *mut sched::Process,
    id: u64,
    address: u64,
    port: u16,
    timeout_ms: u64,
) -> u64 {
    let remote = match read_ipv4(pml4, address) {
        Ok(value) => value,
        Err(code) => return error(code),
    };
    let owner = unsafe { (*process).pid };
    let timeout = timeout_ms.clamp(100, 60_000);
    match crate::net::socket_connect(owner, id, remote, port, timeout) {
        Ok(()) => 0,
        Err(code) => error(code),
    }
}

fn socket_send(
    pml4: u64,
    process: *mut sched::Process,
    id: u64,
    buffer: u64,
    length: usize,
) -> u64 {
    let length = length.min(65536);
    if length == 0 {
        return 0;
    }
    let mut data = alloc::vec![0u8; length];
    if vmm::copy_from_user(pml4, buffer, &mut data).is_err() {
        return error(errno::EINVAL);
    }
    let owner = unsafe { (*process).pid };
    match crate::net::socket_send(owner, id, &data) {
        Ok(count) => count as u64,
        Err(code) => error(code),
    }
}

fn socket_recv(
    pml4: u64,
    process: *mut sched::Process,
    id: u64,
    buffer: u64,
    length: usize,
    timeout_ms: u64,
) -> u64 {
    let length = length.min(65536);
    if length == 0 {
        return 0;
    }
    let owner = unsafe { (*process).pid };
    let timeout = timeout_ms.clamp(0, 60_000);
    match crate::net::socket_recv(owner, id, length, timeout) {
        Ok(data) => {
            if data.is_empty() {
                return 0;
            }
            if vmm::copy_to_user(pml4, buffer, &data).is_err() {
                return error(errno::EINVAL);
            }
            data.len() as u64
        }
        Err(code) => error(code),
    }
}
