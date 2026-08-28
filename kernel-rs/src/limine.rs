//! Limine boot protocol requests (v8/9 revision layout, kept in lockstep with
//! the projections that used to live inline in the C sources).

use core::ptr::addr_of;

pub const LIMINE_COMMON_MAGIC: [u64; 2] = [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b];

#[used]
#[link_section = ".limine_requests_start"]
static REQUESTS_START_MARKER: [u64; 4] = [LIMINE_COMMON_MAGIC[0], LIMINE_COMMON_MAGIC[1], 0, 0];

#[used]
#[link_section = ".limine_requests_end"]
static REQUESTS_END_MARKER: [u64; 2] = [0xadc0e0531bb10d03, 0x9572709f3174c460];

#[used]
#[link_section = ".limine_requests"]
static BASE_REVISION: [u64; 3] = [0xf9562b2d5c95a6c8, 0x6a7b384944536bdc, 4];

// --- memory map ------------------------------------------------------------

pub const LIMINE_MEMMAP_USABLE: u64 = 0;

#[repr(C)]
pub struct MemmapEntry {
    pub base: u64,
    pub length: u64,
    pub typ: u64,
}

#[repr(C)]
pub struct MemmapResponse {
    pub revision: u64,
    pub entry_count: u64,
    pub entries: *const *const MemmapEntry,
}

#[repr(C)]
struct MemmapRequest {
    id: [u64; 4],
    revision: u64,
    response: *const MemmapResponse,
}
unsafe impl Sync for MemmapRequest {}

#[used]
#[link_section = ".limine_requests"]
static MEMMAP_REQUEST: MemmapRequest = MemmapRequest {
    id: [
        LIMINE_COMMON_MAGIC[0],
        LIMINE_COMMON_MAGIC[1],
        0x67cf3d9d378a806f,
        0xe304acdfc50c3c62,
    ],
    revision: 0,
    response: core::ptr::null(),
};

pub fn memmap_response() -> Option<&'static MemmapResponse> {
    let p = unsafe { addr_of!(MEMMAP_REQUEST.response).read_volatile() };
    unsafe { p.as_ref() }
}

// --- HHDM ------------------------------------------------------------------

#[repr(C)]
pub struct HhdmResponse {
    pub revision: u64,
    pub offset: u64,
}

#[repr(C)]
struct HhdmRequest {
    id: [u64; 4],
    revision: u64,
    response: *const HhdmResponse,
}
unsafe impl Sync for HhdmRequest {}

#[used]
#[link_section = ".limine_requests"]
static HHDM_REQUEST: HhdmRequest = HhdmRequest {
    id: [
        LIMINE_COMMON_MAGIC[0],
        LIMINE_COMMON_MAGIC[1],
        0x48dcf1cb8ad2b852,
        0x63984e959a98244b,
    ],
    revision: 0,
    response: core::ptr::null(),
};

pub fn hhdm_response() -> Option<&'static HhdmResponse> {
    let p = unsafe { addr_of!(HHDM_REQUEST.response).read_volatile() };
    unsafe { p.as_ref() }
}

// --- framebuffer -----------------------------------------------------------

#[repr(C)]
pub struct Framebuffer {
    pub address: *mut u8,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
    pub bpp: u16,
    pub memory_model: u8,
    pub red_mask_size: u8,
    pub red_mask_shift: u8,
    pub green_mask_size: u8,
    pub green_mask_shift: u8,
    pub blue_mask_size: u8,
    pub blue_mask_shift: u8,
    pub unused: [u8; 7],
    pub edid_size: u64,
    pub edid: *const u8,
    pub mode_count: u64,
    pub modes: *const *const u8,
}

#[repr(C)]
pub struct FramebufferResponse {
    pub revision: u64,
    pub framebuffer_count: u64,
    pub framebuffers: *const *const Framebuffer,
}

#[repr(C)]
struct FramebufferRequest {
    id: [u64; 4],
    revision: u64,
    response: *const FramebufferResponse,
}
unsafe impl Sync for FramebufferRequest {}

#[used]
#[link_section = ".limine_requests"]
static FB_REQUEST: FramebufferRequest = FramebufferRequest {
    id: [
        LIMINE_COMMON_MAGIC[0],
        LIMINE_COMMON_MAGIC[1],
        0x9d5827dcd881dd75,
        0xa3148604f6fab11b,
    ],
    revision: 0,
    response: core::ptr::null(),
};

pub fn framebuffer_response() -> Option<&'static FramebufferResponse> {
    let p = unsafe { addr_of!(FB_REQUEST.response).read_volatile() };
    unsafe { p.as_ref() }
}

// --- modules (initrd) ------------------------------------------------------

#[repr(C)]
pub struct File {
    pub revision: u64,
    pub address: *const u8,
    pub size: u64,
    pub path: *const core::ffi::c_char,
    pub cmdline: *const core::ffi::c_char,
    pub media_type: u32,
    pub unused: u32,
    pub tftp_ip: u32,
    pub tftp_port: u32,
    pub partition_index: u32,
    pub mbr_disk_id: u32,
    pub gpt_disk_uuid: [u64; 2],
    pub gpt_part_uuid: [u64; 2],
    pub part_uuid: [u64; 2],
}

#[repr(C)]
pub struct ModuleResponse {
    pub revision: u64,
    pub module_count: u64,
    pub modules: *const *const File,
}

#[repr(C)]
struct ModuleRequest {
    id: [u64; 4],
    revision: u64,
    response: *const ModuleResponse,
}
unsafe impl Sync for ModuleRequest {}

#[used]
#[link_section = ".limine_requests"]
static MODULE_REQUEST: ModuleRequest = ModuleRequest {
    id: [
        LIMINE_COMMON_MAGIC[0],
        LIMINE_COMMON_MAGIC[1],
        0x3e7e279702be32af,
        0xca1c4f3bd1280cee,
    ],
    revision: 0,
    response: core::ptr::null(),
};

pub fn module_response() -> Option<&'static ModuleResponse> {
    let p = unsafe { addr_of!(MODULE_REQUEST.response).read_volatile() };
    unsafe { p.as_ref() }
}
