//! Thin system-call wrappers for Aster Desktop.

use aster_abi::{sys, FramebufferInfo};

#[inline(always)]
pub unsafe fn syscall6(number: u64, a: u64, b: u64, c: u64, d: u64, e: u64, f: u64) -> i64 {
    let result: i64;
    core::arch::asm!(
        "int 0x80",
        inlateout("rax") number as i64 => result,
        in("rdi") a,
        in("rsi") b,
        in("rdx") c,
        in("r10") d,
        in("r8") e,
        in("r9") f,
        options(nostack)
    );
    result
}

pub fn write(bytes: &[u8]) -> i64 {
    unsafe {
        syscall6(
            sys::WRITE,
            1,
            bytes.as_ptr() as u64,
            bytes.len() as u64,
            0,
            0,
            0,
        )
    }
}

pub fn raw(number: u64, a: u64, b: u64, c: u64) -> i64 {
    unsafe { syscall6(number, a, b, c, 0, 0, 0) }
}

pub fn log(message: &[u8]) -> i64 {
    unsafe {
        syscall6(
            sys::LOG,
            message.as_ptr() as u64,
            message.len() as u64,
            0,
            0,
            0,
            0,
        )
    }
}

pub fn getpid() -> i64 {
    unsafe { syscall6(sys::GETPID, 0, 0, 0, 0, 0, 0) }
}

pub fn exit(code: i64) -> ! {
    unsafe {
        syscall6(sys::EXIT, code as u64, 0, 0, 0, 0, 0);
    }
    loop {
        core::hint::spin_loop();
    }
}

pub fn sleep_ms(ms: u64) {
    unsafe {
        syscall6(sys::SLEEP_MS, ms, 0, 0, 0, 0, 0);
    }
}

pub fn yield_now() {
    unsafe {
        syscall6(sys::YIELD, 0, 0, 0, 0, 0, 0);
    }
}

pub fn time_ms() -> u64 {
    unsafe { syscall6(sys::TIME_MS, 0, 0, 0, 0, 0, 0) as u64 }
}

pub fn framebuffer_info() -> Option<FramebufferInfo> {
    let mut info: FramebufferInfo = unsafe { core::mem::zeroed() };
    let result = unsafe {
        syscall6(
            sys::FB_INFO,
            &mut info as *mut FramebufferInfo as u64,
            0,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        None
    } else {
        Some(info)
    }
}

pub fn framebuffer_map() -> Option<u64> {
    let result = unsafe { syscall6(sys::FB_MAP, 0, 0, 0, 0, 0, 0) };
    if result < 0 {
        None
    } else {
        Some(result as u64)
    }
}

pub fn vm_alloc(size: usize) -> Option<u64> {
    let result = unsafe { syscall6(sys::VM_ALLOC, size as u64, 0, 0, 0, 0, 0) };
    if result < 0 {
        None
    } else {
        Some(result as u64)
    }
}

fn path_buffer(path: &str) -> ([u8; 256], usize) {
    let mut buffer = [0u8; 256];
    let length = path.len().min(255);
    buffer[..length].copy_from_slice(&path.as_bytes()[..length]);
    (buffer, length)
}

pub fn open(path: &str, flags: u32) -> i64 {
    let (buffer, _) = path_buffer(path);
    unsafe { syscall6(sys::OPEN, buffer.as_ptr() as u64, flags as u64, 0, 0, 0, 0) }
}

pub fn close(fd: i64) -> i64 {
    unsafe { syscall6(sys::CLOSE, fd as u64, 0, 0, 0, 0, 0) }
}

pub fn read_fd(fd: i64, buffer: &mut [u8]) -> i64 {
    unsafe {
        syscall6(
            sys::READ,
            fd as u64,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
            0,
            0,
            0,
        )
    }
}

pub fn write_fd(fd: i64, buffer: &[u8]) -> i64 {
    unsafe {
        syscall6(
            sys::WRITE_FD,
            fd as u64,
            buffer.as_ptr() as u64,
            buffer.len() as u64,
            0,
            0,
            0,
        )
    }
}

pub fn seek(fd: i64, offset: i64, whence: u32) -> i64 {
    unsafe { syscall6(sys::SEEK, fd as u64, offset as u64, whence as u64, 0, 0, 0) }
}

pub fn read_dir(path: &str) -> Option<alloc::vec::Vec<(alloc::string::String, bool, u64)>> {
    use alloc::string::String;
    use aster_abi::DirEntry;
    let flags = aster_abi::open_flags::READ | aster_abi::open_flags::DIRECTORY;
    let fd = open(path, flags);
    if fd < 0 {
        return None;
    }
    let mut entries = alloc::vec::Vec::new();
    loop {
        let mut buffer = [DirEntry {
            inode: 0,
            ty: 0,
            name_len: 0,
            name: [0; aster_abi::DIRENT_NAME_LEN],
        }; 32];
        let count = unsafe {
            syscall6(
                sys::READDIR,
                fd as u64,
                buffer.as_mut_ptr() as u64,
                32,
                0,
                0,
                0,
            )
        };
        if count <= 0 {
            break;
        }
        for entry in buffer.iter().take(count as usize) {
            let name_len = entry.name_len as usize;
            let name =
                String::from_utf8_lossy(&entry.name[..name_len.min(entry.name.len())]).into_owned();
            entries.push((name, entry.ty == 1, entry.inode));
        }
    }
    close(fd);
    Some(entries)
}

pub fn spawn(path: &str) -> i64 {
    spawn_with_handle(path, 0)
}

pub fn spawn_with_handle(path: &str, bootstrap: u32) -> i64 {
    let (buffer, _) = path_buffer(path);
    unsafe {
        syscall6(
            sys::SPAWN,
            buffer.as_ptr() as u64,
            bootstrap as u64,
            0,
            0,
            0,
            0,
        )
    }
}

pub fn channel_create() -> Option<(u32, u32)> {
    let mut handles = [0u32; 2];
    let result = unsafe {
        syscall6(
            sys::CHANNEL_CREATE,
            handles.as_mut_ptr() as u64,
            0,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        None
    } else {
        Some((handles[0], handles[1]))
    }
}

pub fn channel_send(handle: u32, data: &[u8]) -> i64 {
    unsafe {
        syscall6(
            sys::CHANNEL_SEND,
            handle as u64,
            data.as_ptr() as u64,
            data.len() as u64,
            0,
            0,
            0,
        )
    }
}

pub fn channel_recv(handle: u32, buffer: &mut [u8]) -> i64 {
    unsafe {
        syscall6(
            sys::CHANNEL_RECV,
            handle as u64,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
            0,
            0,
            0,
        )
    }
}

pub fn channel_close(handle: u32) -> i64 {
    unsafe { syscall6(sys::CHANNEL_CLOSE, handle as u64, 0, 0, 0, 0, 0) }
}

pub fn wait(pid: u64, status: &mut i64) -> i64 {
    unsafe { syscall6(sys::WAIT, pid, status as *mut i64 as u64, 0, 0, 0, 0) }
}

pub fn storage_info() -> Option<aster_abi::StorageInfo> {
    let mut info: aster_abi::StorageInfo = unsafe { core::mem::zeroed() };
    let result = unsafe {
        syscall6(
            sys::STORAGE_INFO,
            &mut info as *mut aster_abi::StorageInfo as u64,
            0,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        None
    } else {
        Some(info)
    }
}

pub fn stat(path: &str) -> Option<aster_abi::FileStat> {
    let (buffer, _) = path_buffer(path);
    let mut stat: aster_abi::FileStat = unsafe { core::mem::zeroed() };
    let result = unsafe {
        syscall6(
            sys::STAT,
            buffer.as_ptr() as u64,
            &mut stat as *mut aster_abi::FileStat as u64,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        None
    } else {
        Some(stat)
    }
}

pub fn read_file(path: &str) -> Option<alloc::vec::Vec<u8>> {
    let fd = open(path, aster_abi::open_flags::READ);
    if fd < 0 {
        return None;
    }
    let mut data = alloc::vec::Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let count = read_fd(fd, &mut buffer);
        if count <= 0 {
            break;
        }
        data.extend_from_slice(&buffer[..count as usize]);
        if data.len() > 4 * 1024 * 1024 {
            break;
        }
    }
    close(fd);
    Some(data)
}

pub fn write_file(path: &str, data: &[u8]) -> bool {
    let flags =
        aster_abi::open_flags::WRITE | aster_abi::open_flags::CREATE | aster_abi::open_flags::TRUNC;
    let fd = open(path, flags);
    if fd < 0 {
        return false;
    }
    let mut offset = 0;
    let mut ok = true;
    while offset < data.len() {
        let end = (offset + 8192).min(data.len());
        let written = write_fd(fd, &data[offset..end]);
        if written <= 0 {
            ok = false;
            break;
        }
        offset += written as usize;
    }
    close(fd);
    ok
}

pub fn unlink(path: &str) -> bool {
    let (buffer, _) = path_buffer(path);
    unsafe { syscall6(sys::UNLINK, buffer.as_ptr() as u64, 0, 0, 0, 0, 0) >= 0 }
}

pub fn mkdir(path: &str) -> bool {
    let (buffer, _) = path_buffer(path);
    unsafe { syscall6(sys::MKDIR, buffer.as_ptr() as u64, 0, 0, 0, 0, 0) >= 0 }
}

pub fn rtc() -> Option<aster_abi::RtcTime> {
    let mut time: aster_abi::RtcTime = unsafe { core::mem::zeroed() };
    let result = unsafe {
        syscall6(
            sys::RTC,
            &mut time as *mut aster_abi::RtcTime as u64,
            0,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        None
    } else {
        Some(time)
    }
}

pub fn process_list(buffer: &mut [aster_abi::ProcessInfo]) -> usize {
    let result = unsafe {
        syscall6(
            sys::PROC_LIST,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        0
    } else {
        result as usize
    }
}

pub fn system_info() -> Option<aster_abi::SystemInfo> {
    let mut info: aster_abi::SystemInfo = unsafe { core::mem::zeroed() };
    let result = unsafe {
        syscall6(
            sys::SYSTEM_INFO,
            &mut info as *mut aster_abi::SystemInfo as u64,
            0,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        None
    } else {
        Some(info)
    }
}

pub fn poll_input(buffer: &mut [aster_abi::InputEvent]) -> usize {
    let result = unsafe {
        syscall6(
            sys::INPUT_POLL,
            buffer.as_mut_ptr() as u64,
            buffer.len() as u64,
            0,
            0,
            0,
            0,
        )
    };
    if result < 0 {
        0
    } else {
        result as usize
    }
}

pub fn shutdown() -> ! {
    unsafe {
        syscall6(sys::SHUTDOWN, 0, 0, 0, 0, 0, 0);
    }
    loop {
        core::hint::spin_loop();
    }
}
