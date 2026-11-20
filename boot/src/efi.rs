//! Raw UEFI bindings used by Aster Boot.
//!
//! Only the protocols the loader needs are typed. All layouts follow the
//! UEFI Specification 2.10 (system table, boot services, file system, GOP).
//! Function pointers use the `efiapi` calling convention (Microsoft x64).

#![allow(non_snake_case)]

use core::ffi::c_void;

pub type Status = usize;
pub type Handle = *mut c_void;
pub type PhysicalAddress = u64;

pub const EFI_SUCCESS: Status = 0;
pub const EFI_BUFFER_TOO_SMALL: Status = 5;
pub const EFI_ERROR_BIT: Status = 1 << 63;

pub const ALLOCATE_ANY_PAGES: u32 = 0;
pub const EFI_LOADER_DATA: u32 = 2;
pub const EFI_FILE_MODE_READ: u64 = 1;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

impl Guid {
    pub const fn new(data1: u32, data2: u16, data3: u16, data4: [u8; 8]) -> Self {
        Self { data1, data2, data3, data4 }
    }
}

pub const LOADED_IMAGE_GUID: Guid =
    Guid::new(0x5B1B31A1, 0x9562, 0x11d2, [0x8E, 0x3F, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B]);
pub const SIMPLE_FILE_SYSTEM_GUID: Guid =
    Guid::new(0x964E5B22, 0x6459, 0x11d2, [0x8E, 0x39, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B]);
pub const FILE_INFO_GUID: Guid =
    Guid::new(0x09576E92, 0x6D3F, 0x11d2, [0x8E, 0x39, 0x00, 0xA0, 0xC9, 0x69, 0x72, 0x3B]);
pub const GRAPHICS_OUTPUT_GUID: Guid =
    Guid::new(0x9042A9DE, 0x23DC, 0x4A38, [0x96, 0xFB, 0x7A, 0xDE, 0xD0, 0x80, 0x51, 0x6A]);
pub const ACPI2_TABLE_GUID: Guid =
    Guid::new(0x8868E871, 0xE4F1, 0x11D3, [0xBC, 0x22, 0x00, 0x80, 0xC7, 0x3C, 0x88, 0x81]);

#[repr(C)]
pub struct TableHeader {
    pub signature: u64,
    pub revision: u32,
    pub header_size: u32,
    pub crc32: u32,
    pub reserved: u32,
}

pub type TextOutputString =
    unsafe extern "efiapi" fn(this: *mut SimpleTextOutput, string: *const u16) -> Status;

#[repr(C)]
pub struct SimpleTextOutput {
    pub reset: usize,
    pub output_string: TextOutputString,
    pub test_string: usize,
    pub query_mode: usize,
    pub set_mode: usize,
    pub set_attribute: usize,
    pub clear_screen: usize,
    pub set_cursor_position: usize,
    pub enable_cursor: usize,
    pub mode: *mut c_void,
}

#[repr(C)]
pub struct ConfigurationTable {
    pub vendor_guid: Guid,
    pub vendor_table: *mut c_void,
}

#[repr(C)]
pub struct SystemTable {
    pub hdr: TableHeader,
    pub firmware_vendor: *const u16,
    pub firmware_revision: u32,
    pub console_in_handle: Handle,
    pub con_in: *mut c_void,
    pub console_out_handle: Handle,
    pub con_out: *mut SimpleTextOutput,
    pub std_err_handle: Handle,
    pub std_err: *mut SimpleTextOutput,
    pub runtime_services: *mut c_void,
    pub boot_services: *mut BootServices,
    pub number_of_table_entries: usize,
    pub configuration_table: *mut ConfigurationTable,
}

#[repr(C)]
pub struct BootServices {
    pub hdr: TableHeader,
    pub raise_tpl: usize,
    pub restore_tpl: usize,
    pub allocate_pages: unsafe extern "efiapi" fn(
        alloc_type: u32,
        memory_type: u32,
        pages: usize,
        memory: *mut PhysicalAddress,
    ) -> Status,
    pub free_pages:
        unsafe extern "efiapi" fn(memory: PhysicalAddress, pages: usize) -> Status,
    pub get_memory_map: unsafe extern "efiapi" fn(
        size: *mut usize,
        map: *mut u8,
        key: *mut usize,
        descriptor_size: *mut usize,
        descriptor_version: *mut u32,
    ) -> Status,
    pub allocate_pool:
        unsafe extern "efiapi" fn(pool_type: u32, size: usize, buffer: *mut *mut u8) -> Status,
    pub free_pool: unsafe extern "efiapi" fn(buffer: *mut u8) -> Status,
    pub create_event: usize,
    pub set_timer: usize,
    pub wait_for_event: usize,
    pub signal_event: usize,
    pub close_event: usize,
    pub check_event: usize,
    pub install_protocol_interface: usize,
    pub reinstall_protocol_interface: usize,
    pub uninstall_protocol_interface: usize,
    pub handle_protocol: usize,
    pub reserved: usize,
    pub register_protocol_notify: usize,
    pub locate_handle: usize,
    pub locate_device_path: usize,
    pub install_configuration_table: usize,
    pub load_image: usize,
    pub start_image: usize,
    pub exit: usize,
    pub unload_image: usize,
    pub exit_boot_services:
        unsafe extern "efiapi" fn(image: Handle, map_key: usize) -> Status,
    pub get_next_monotonic_count: usize,
    pub stall: unsafe extern "efiapi" fn(microseconds: usize) -> Status,
    pub set_watchdog_timer: unsafe extern "efiapi" fn(
        timeout: usize,
        watchdog_code: u64,
        data_size: usize,
        data: *mut u16,
    ) -> Status,
    pub connect_controller: usize,
    pub disconnect_controller: usize,
    pub open_protocol: unsafe extern "efiapi" fn(
        handle: Handle,
        protocol: *const Guid,
        interface: *mut *mut c_void,
        agent_handle: Handle,
        controller_handle: Handle,
        attributes: u32,
    ) -> Status,
    pub close_protocol: usize,
    pub open_protocol_information: usize,
    pub protocols_per_handle: usize,
    pub locate_handle_buffer: usize,
    pub locate_protocol: unsafe extern "efiapi" fn(
        protocol: *const Guid,
        registration: *mut c_void,
        interface: *mut *mut c_void,
    ) -> Status,
    pub install_multiple_protocol_interfaces: usize,
    pub uninstall_multiple_protocol_interfaces: usize,
    pub calculate_crc32: usize,
    pub copy_mem: usize,
    pub set_mem: usize,
    pub create_event_ex: usize,
}

#[repr(C)]
pub struct LoadedImage {
    pub revision: u32,
    pub parent_handle: Handle,
    pub system_table: *mut SystemTable,
    pub device_handle: Handle,
    pub file_path: *mut c_void,
    pub reserved: *mut c_void,
    pub load_options_size: u32,
    pub load_options: *mut c_void,
    pub image_base: *mut c_void,
    pub image_size: u64,
    pub image_code_type: u32,
    pub image_data_type: u32,
    pub unload: usize,
}

#[repr(C)]
pub struct FileProtocol {
    pub revision: u64,
    pub open: unsafe extern "efiapi" fn(
        this: *mut FileProtocol,
        out_file: *mut *mut FileProtocol,
        file_name: *const u16,
        open_mode: u64,
        attributes: u64,
    ) -> Status,
    pub close: unsafe extern "efiapi" fn(this: *mut FileProtocol) -> Status,
    pub delete: usize,
    pub read: unsafe extern "efiapi" fn(
        this: *mut FileProtocol,
        buffer_size: *mut usize,
        buffer: *mut u8,
    ) -> Status,
    pub write: usize,
    pub get_position: usize,
    pub set_position: unsafe extern "efiapi" fn(this: *mut FileProtocol, position: u64) -> Status,
    pub get_info: unsafe extern "efiapi" fn(
        this: *mut FileProtocol,
        information_type: *const Guid,
        buffer_size: *mut usize,
        buffer: *mut u8,
    ) -> Status,
    pub set_info: usize,
    pub flush: usize,
    pub open_ex: usize,
    pub read_ex: usize,
    pub write_ex: usize,
    pub flush_ex: usize,
}

impl FileProtocol {
    pub unsafe fn read_exact(&mut self, buffer: &mut [u8]) -> Result<(), Status> {
        let mut offset = 0usize;
        while offset < buffer.len() {
            let mut chunk = buffer.len() - offset;
            let status = (self.read)(self, &mut chunk, buffer.as_mut_ptr().add(offset));
            if status != EFI_SUCCESS {
                return Err(status);
            }
            if chunk == 0 {
                break;
            }
            offset += chunk;
        }
        Ok(())
    }
}

#[repr(C)]
pub struct SimpleFileSystem {
    pub revision: u64,
    pub open_volume:
        unsafe extern "efiapi" fn(this: *mut SimpleFileSystem, root: *mut *mut FileProtocol) -> Status,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct PixelBitmask {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
    pub reserved: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct GopModeInfo {
    pub version: u32,
    pub horizontal_resolution: u32,
    pub vertical_resolution: u32,
    pub pixel_format: u32,
    pub pixel_information: PixelBitmask,
    pub pixels_per_scan_line: u32,
}

#[repr(C)]
pub struct GopMode {
    pub max_mode: u32,
    pub mode: u32,
    pub info: *mut GopModeInfo,
    pub size_of_info: usize,
    pub frame_buffer_base: PhysicalAddress,
    pub frame_buffer_size: usize,
}

pub type GopSetMode =
    unsafe extern "efiapi" fn(this: *mut GraphicsOutput, mode: u32) -> Status;

pub type GopQueryMode = unsafe extern "efiapi" fn(
    this: *mut GraphicsOutput,
    mode: u32,
    size_of_info: *mut usize,
    info: *mut *mut GopModeInfo,
) -> Status;

#[repr(C)]
pub struct GraphicsOutput {
    pub query_mode: GopQueryMode,
    pub set_mode: GopSetMode,
    pub blt: usize,
    pub mode: *mut GopMode,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct MemoryDescriptor {
    pub ty: u32,
    pub pad: u32,
    pub physical_start: u64,
    pub virtual_start: u64,
    pub number_of_pages: u64,
    pub attribute: u64,
}

pub unsafe fn open_protocol<T>(
    bs: *mut BootServices,
    handle: Handle,
    guid: &Guid,
) -> Result<*mut T, Status> {
    let mut interface: *mut c_void = core::ptr::null_mut();
    // Attributes: BY_HANDLE_PROTOCOL (0x02) avoids a CloseProtocol requirement.
    let status =
        ((*bs).open_protocol)(handle, guid, &mut interface, core::ptr::null_mut(), core::ptr::null_mut(), 0x02);
    if status != EFI_SUCCESS {
        return Err(status);
    }
    if interface.is_null() {
        return Err(EFI_SUCCESS);
    }
    Ok(interface as *mut T)
}

pub unsafe fn locate_protocol<T>(bs: *mut BootServices, guid: &Guid) -> Result<*mut T, Status> {
    let mut interface: *mut c_void = core::ptr::null_mut();
    let status = ((*bs).locate_protocol)(guid, core::ptr::null_mut(), &mut interface);
    if status != EFI_SUCCESS || interface.is_null() {
        return Err(status);
    }
    Ok(interface as *mut T)
}

pub unsafe fn allocate_pages(bs: *mut BootServices, pages: usize) -> Result<PhysicalAddress, Status> {
    let mut address: PhysicalAddress = 0;
    let status =
        ((*bs).allocate_pages)(ALLOCATE_ANY_PAGES, EFI_LOADER_DATA, pages, &mut address);
    if status != EFI_SUCCESS {
        return Err(status);
    }
    // UEFI does not guarantee zeroed pages.
    core::ptr::write_bytes(address as *mut u8, 0, pages * 4096);
    Ok(address)
}

pub unsafe fn allocate_pool(bs: *mut BootServices, size: usize) -> Result<*mut u8, Status> {
    let mut buffer: *mut u8 = core::ptr::null_mut();
    let status = ((*bs).allocate_pool)(EFI_LOADER_DATA, size, &mut buffer);
    if status != EFI_SUCCESS {
        return Err(status);
    }
    Ok(buffer)
}
