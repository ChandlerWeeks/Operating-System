HHDM;
MMIO; 

#[unsafe(linker_section = ".limine_requests_start")]
static LIMINE_MAGIC_START: [u64; 4] = [0xf6b8...,];

#[unsafe(linker_section = ".limine_requests_end")]
static LIMINE_MAGIC_END: [u64; 4] = [0xffff...,];
