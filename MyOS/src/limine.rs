/// Common prefix shared by all Limine requests.
const COMMON_MAGIC: [u64; 2] = [0xc7b1dd30df4c8b88, 0x0a82e883a194f07b];

/// Base revision magic. Identifies the base revision array.
const BASE_REVISION_MAGIC: [u64; 2] = [0xf9562b2d5c95a6c8, 0x6a7b384944536bdc];

/// Start marker magic value. Required from base revision 2 onwards.
const REQUESTS_START_MAGIC: [u64; 4] = [
    0xf6b8f4b39de7d1ae,
    0xfab91a6940fcb9cf,
    0x785c6ed015d3e316,
    0x181e920a7852b9d9,
];

/// End marker magic value. Required from base revision 2 onwards.
const REQUESTS_END_MAGIC: [u64; 2] = [0xadc0e0531bb10d03, 0x9572709f31764c62];

// Request-specific ID halves (id[2], id[3])
const HHDM_ID: [u64; 2] = [0x48dcf1cb8ad2b852, 0x63984e959a98244b];
const MEMMAP_ID: [u64; 2] = [0x67cf3d9d378a806f, 0xe304acdfc50c3c62];
const RSDP_ID: [u64; 2] = [0xc5e77b6b397e7b43, 0x27637845accdcf3c];
const DTB_ID: [u64; 2] = [0xb40ddb48fb54bac7, 0x545081493f81ffb7];
const EXE_ADDR_ID: [u64; 2] = [0x71ba76863cc55f63, 0xb2644a48c516a487];
const STACK_SIZE_ID: [u64; 2] = [0x224ef0460a8e8926, 0xe1cb0fc25f46ea3d];
const BSP_HARTID_ID: [u64; 2] = [0x1369359f025525f9, 0x2ff2a56178391bb6];
const FW_TYPE_ID: [u64; 2] = [0x8c2f75d90bef28a8, 0x7045a4688eac00c3];
const PAGING_MODE_ID: [u64; 2] = [0x95c1a0edab0944cb, 0xa4e5cb3842f7488a];
// const FRAMEBUF_ID: [u64; 2] = [0x9d5827dcd881dd75, 0xa3148604f6fab11b];
const DATE_ID: [u64; 2] = [0x502746e184c088aa, 0xfbc5ec83e6327893];
const SMBIOS_ID: [u64; 2] = [0x9e9046f11e095391, 0xaa4a520fefbde5ee];
// const MP_ID: [u64; 2] = [0x95a67b819a1b857e, 0xa0b61b723b6a73e0];
const CMD_LINE_ID: [u64; 2] = [0x4b161536e598651e, 0xb390ad4a2f1f303a];

const fn make_id(specific: [u64; 2]) -> [u64; 4] {
    [COMMON_MAGIC[0], COMMON_MAGIC[1], specific[0], specific[1]]
}

#[used]
#[unsafe(link_section = ".limine_requests_start")]
// The requests start magic must be in the limine_requests_start linker section.
static REQUESTS_START: [u64; 4] = REQUESTS_START_MAGIC;

#[used]
#[unsafe(link_section = ".limine_requests_end")]
// The requests end magic must be in the limine_requests_end linker section.
static REQUESTS_END: [u64; 2] = REQUESTS_END_MAGIC;

#[cfg(target_arch = "riscv64")]
#[allow(dead_code)]
mod paging_modes {
    pub const SV39: u64 = 0;
    pub const SV48: u64 = 1;
    pub const SV57: u64 = 2;

    pub const PREFERED: u64 = SV48;
    pub const MIN: u64 = SV48;
    pub const MAX = u64 = SV48;
}

#[repr(C)]
struct PagingModeRequest {
    id: [u64; 4],
    revision: u64,
    response: AtomicPtr<u8>, // PagingModeResponse* — opaque, we check via mode
    mode: u64,               // preferred mode
    max_mode: u64,           // maximum acceptable mode (must be >= min_mode)
    min_mode: u64,           // minimum acceptable mode
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static PAGING_MODE_REQUEST: PagingModeRequest = PagingModeRequest {
    id: make_id(PAGING_MODE_ID),
    // Revision 1 adds max_mode and min_mode. Without revision 1, Limine
    // only reads `mode` and leaves max_mode/min_mode as uninitialized memory,
    // which can cause the "max_mode lower than min_mode" panic.
    revision: 1,
    response: AtomicPtr::new(core::ptr::null_mut()),
    mode: paging_modes::PREFERRED,
    max_mode: paging_modes::MAX,
    min_mode: paging_modes::MIN,
};

#[repr(C)]
struct HhdmRequest {
    id: [u64; 4],
    revision: u64,
    response: AtomicPtr<HhdmResponse>,
}

#[repr(C)]
struct HhdmResponse {
    revision: u64,
    offset: u64,
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest {
    id: make_id(HHDM_ID),
    revision: 0,
    response: AtomicPtr::new(core::ptr::null_mut()),
};


/// The HHDM virtual offset and add it to any physical address to get its kernel VA.
fn hhdm_offset() -> u64 {
    let ptr = HHDM_REQUEST.response.load(Ordering::Relaxed);
    assert!(!ptr.is_null(), "Limine did not provide HHDM response");
    unsafe { (*ptr).offset }
}

fn hhdm_phys_to_virt(paddr: usize) -> usize {
    let off = hhdm_offset() as usize;
    off.saturating_add(paddr)
}
fn hhdm_virt_to_phys(vaddr: usize) -> usize {
    let off = hhdm_offset() as usize;
    vaddr.saturating_sub(off)
}

#[repr(C)]
struct ExecutableAddressResponse {
    revision: u64,
    physical_base: u64, // physical address Limine loaded the kernel at
    virtual_base: u64,  // virtual address (should match your linker script VMA)
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static EXE_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest {
    id: make_id(EXE_ADDR_ID),
    revision: 0,
    response: AtomicPtr::new(core::ptr::null_mut()),
};

/// Physical address where Limine loaded the kernel image.
pub fn kernel_phys_base() -> u64 {
    let ptr = EXE_ADDRESS_REQUEST.response.load(Ordering::Acquire);
    assert!(!ptr.is_null(), "Limine did not provide executable address");
    unsafe { (*ptr).physical_base }
}

/// Virtual address where the kernel is mapped.
pub fn kernel_virt_base() -> u64 {
    let ptr = EXE_ADDRESS_REQUEST.response.load(Ordering::Acquire);
    assert!(!ptr.is_null(), "Limine did not provide executable address");
    unsafe { (*ptr).virtual_base }
}

pub fn va_to_pa(addr: usize) -> usize {
    if addr & 0xFFFF_FFFF_0000_0000 == 0xFFFF_FFFF_0000_0000 {
        // Kernel executable address
        let kv = kernel_virt_base();
        let kp = kernel_phys_base();
        (kp + (addr as u64 - kv)) as usize
    } else if addr & config::KERNEL_IO_ADDR == config::KERNEL_IO_ADDR {
        // This is an IO mapping
        addr.saturating_sub(config::KERNEL_IO_ADDR)
    } else if addr & config::KERNEL_HEAP_ADDR == config::KERNEL_HEAP_ADDR {
        addr.saturating_sub(config::KERNEL_HEAP_ADDR)
    } else if addr & 0xFFFF_8000_0000_0000 == 0xFFFF_8000_0000_0000 {
        // HHDM mapped virtual address
        hhdm_virt_to_phys(addr)
    } else {
        // Lower-half, so already a physical address.
        addr
    }
}

pub fn pa_to_va(addr: usize) -> usize {
    if addr & 0xFFFF_8000_0000_0000 == 0xFFFF_8000_0000_0000 {
        // Address is already a virtual address, so just return it.
        return addr;
    }
    if addr < 0x8000_0000 {
        // This is an MMIO address.
        return KERNEL_IO_ADDR + addr;
    }
    // HHDM mapped virtual address
    hhdm_phys_to_virt(addr)
}

#[repr(C)]
struct BspHartidRequest {
    id: [u64; 4],
    revision: u64,
    response: AtomicPtr<BspHartidResponse>,
}

#[repr(C)]
struct BspHartidResponse {
    revision: u64,
    bsp_hartid: u64,
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static BSP_HARTID_REQUEST: BspHartidRequest = BspHartidRequest {
    id: make_id(BSP_HARTID_ID),
    revision: 0,
    response: AtomicPtr::new(core::ptr::null_mut()),
};

/// The hart ID of the bootstrap HART.
pub fn bsp_hartid() -> u64 {
    let ptr = BSP_HARTID_REQUEST.response.load(Ordering::Acquire);
    if ptr.is_null() {
        0
    } else {
        unsafe { (*ptr).bsp_hartid }
    }
}

#[repr(C)]
struct StackSizeRequest {
    id: [u64; 4],
    revision: u64,
    response: AtomicPtr<u8>, // response unused
    stack_size: u64,
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static STACK_SIZE_REQUEST: StackSizeRequest = StackSizeRequest {
    id: make_id(STACK_SIZE_ID),
    revision: 0,
    response: AtomicPtr::new(core::ptr::null_mut()),
    stack_size: LIMINE_STACK_SIZE, // 8192 bytes
};

#[repr(C)]
struct CmdlineRequest {
    id: [u64; 4],
    revision: u64,
    response: AtomicPtr<CmdlineResponse>,
}
#[repr(C)]
struct CmdlineResponse {
    revision: u64,
    cmdline: *const u8,
}
/// # Command Line Request
#[used]
#[unsafe(link_section = ".limine_requests")]
static CMD_LINE_REQUEST: CmdlineRequest = CmdlineRequest {
    id: make_id(CMD_LINE_ID),
    revision: 0,
    response: AtomicPtr::new(core::ptr::null_mut()),
};

pub fn cmd_line() -> Option<&'static str> {
    let ptr = CMD_LINE_REQUEST.response.load(Ordering::SeqCst);
    if ptr.is_null() {
        return None;
    }
    let response = unsafe { ptr.as_ref_unchecked() };
    match unsafe { core::ffi::CStr::from_ptr(response.cmdline) }.to_str() {
        Ok(x) => Some(x),
        Err(_) => None,
    }
}

#[repr(C)]
struct MemoryMapRequest {
    id: [u64; 4],
    revision: u64,
    response: AtomicPtr<MemoryMapResponse>,
}

#[repr(C)]
struct MemoryMapResponse {
    revision: u64,
    entry_count: u64,
    entries: *const *const MemoryMapEntry,
}

#[repr(C)]
pub struct MemoryMapEntry {
    pub base: u64,
    pub length: u64,
    pub entry_type: u64,
}

/// Memory map entry types, only USABLE regions should go to your allocator.
pub mod memmap {
    pub const USABLE: u64 = 0;
    pub const RESERVED: u64 = 1;
    pub const ACPI_RECLAIMABLE: u64 = 2;
    pub const ACPI_NVS: u64 = 3;
    pub const BAD_MEMORY: u64 = 4;
    pub const BOOTLOADER_RECLAIMABLE: u64 = 5;
    pub const KERNEL_AND_MODULES: u64 = 6;
    pub const FRAMEBUFFER: u64 = 7;
    pub const RESERVED_MAPPED: u64 = 8;
}
#[used]
#[unsafe(link_section = ".limine_requests")]
static MEMORY_MAP_REQUEST: MemoryMapRequest = MemoryMapRequest {
    id: make_id(MEMMAP_ID),
    revision: 0,
    response: AtomicPtr::new(core::ptr::null_mut()),
};

pub fn usable_memory_regions() -> impl Iterator<Item = (u64, u64)> {
    let ptr = MEMORY_MAP_REQUEST.response.load(Ordering::Acquire);
    assert!(!ptr.is_null(), "Limine did not provide memory map");

    let response = unsafe { &*ptr };
    let count = response.entry_count as usize;
    let entries = response.entries;

    (0..count).filter_map(move |i| {
        let entry = unsafe { &**entries.add(i) };
        if entry.entry_type == memmap::USABLE {
            Some((entry.base, entry.length))
        } else {
            None
        }
    })
}

pub fn all_memory_regions() -> impl Iterator<Item = &'static MemoryMapEntry> {
    let ptr = MEMORY_MAP_REQUEST.response.load(Ordering::Acquire);
    assert!(!ptr.is_null(), "Limine did not provide memory map");

    let response = unsafe { &*ptr };
    let count = response.entry_count as usize;
    let entries = response.entries;

    (0..count).map(move |i| unsafe { &**entries.add(i) })
}

#[repr(C)]
struct DateAtBootRequest {
    id: [u64; 4],
    revision: u64,
    response: AtomicPtr<DateAtBootResponse>,
}

#[repr(C)]
struct DateAtBootResponse {
    revision: u64,
    timestamp: u64,
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static DATE_AT_BOOT_REQUEST: DateAtBootRequest = DateAtBootRequest {
    id: make_id(DATE_ID),
    revision: 0,
    response: AtomicPtr::new(core::ptr::null_mut()),
};

/// Returns the first available framebuffer, if any.
pub fn date_at_boot() -> Option<u64> {
    let ptr = DATE_AT_BOOT_REQUEST.response.load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }
    Some(unsafe { (*ptr).timestamp })
}

#[repr(C)]
struct RsdpRequest {
    id: [u64; 4],
    revision: u64,
    response: AtomicPtr<RsdpResponse>,
}

#[repr(C)]
struct RsdpResponse {
    revision: u64,
    address: u64, // virtual address (HHDM) of the RSDP in base revision 3+
}

#[used]
#[unsafe(link_section = ".limine_requests")]
static RSDP_REQUEST: RsdpRequest = RsdpRequest {
    id: make_id(RSDP_ID),
    revision: 0,
    response: AtomicPtr::new(core::ptr::null_mut()),
};

/// Returns None if ACPI is not available.
pub fn rsdp_virt() -> Option<u64> {
    let ptr = RSDP_REQUEST.response.load(Ordering::Acquire);
    if ptr.is_null() {
        return None;
    }
    let addr = unsafe { (*ptr).address };
    if addr == 0 { None } else { Some(addr) }
}
