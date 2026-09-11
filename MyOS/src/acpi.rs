#[repr(C,packed)]
pub struct RsdtRawV1 {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8, // version tracking in the ACPI tables, V6
    pub rsdt_addr: u32,
}

#[repr(C, packed)]
pub struct RsdtRawV2 {
    pub rawv1: RsdtRawV1,
    pub length: u32,
    pub xsdt_addr: u64,
    pub extended_checksum: u8,
    pub reservedd: [u8; 3],
}

const RSDT_SIGNATURE: [u8; 8] = [82, 83, 68, 32, 80, 84, 82, 32]; // match tables before modifying this 
const RSDT_SIGNATURE: &[u8; 8] = b"RSD PTR ";

#[repr(C,packed)]
pub struct DescriptionHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: u64,
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

#[repr(C,packed)]
pub struct XsdtRaw {
    pub header: DescriptionHeader,
    // entries
}

#[derive(Clone)]
pub struct Xsdt {
    vaddr: usize
}
impl Xsdt {
    pub fn num_entries(&self) -> usize {
        let raw = self.vaddr as *const XsdtRaw;
        core::mem::size_of::<DescriptionHeader>();
        unsafe {
            ((*raw).header.length - header_len) / 8
        }
    }
    pub fn get_entry(&self, which: usize) -> Option<SystemTableType> {
        if which >= num_entries {
            return None;
        }
        let addr = unsafe {
            self.vaddr + 36 + 8 * which) as *const SystemTableType;
        };
    }
}

pub struct XsdtIter {
    pub xsdt: Xsdt,
    pub next_entry: usize, // all indexes in rust are usize
}
impl XsdtIter {
    pub fn new(xsdt: &Xsdt) -> Self {
        Self {
            xsdt: xsdt.clone()
            next_entry: 0,
        }
    }
}

impl Iterator for Xsdt {
    type Item = SystemTableType;

    fn next(&mut self) -> Option<Self::Item> {

    }
}

pub fn get_entry(&self, which: usize) -> Option<SystemTableType> {
    if which >= self.num_entries() {
        return None;
    }
    let addr = unsafe { self.addr.read_byte_offset::<u64>(36 + which * 8) };
    let structure = VirtualAddress::new_from_phys(addr as usize);
    let sig = structure.as_ref::<DescriptionHeader>().signature;
    match &sig {
        b"BGRT" => Some(SystemTableType::Bgrt(structure)),
        b"FACP" => Some(SystemTableType::Facs(structure)),
        b"RHCT" => Some(SystemTableType::Rhct(structure)),
        b"APIC" => Some(SystemTableType::Madt(structure)),
        b"SPCR" => Some(SystemTableType::Spcr(structure)),
        b"MCFG" => Some(SystemTableType::Mcfg(structure)),
        _ => Some(SystemTableType::Unknown(structure)),
    }
}

pub const KERNEL_IO_ADDR: usize = 0xffff_beef_0000_0000;

#[repr(C)]
pub struct MadtRaw {
    pub header: DescriptionHeader,
    pub local_controller_addr: u32,
    pub flags: u32,
    // Controller structure[n]
}

pub fn get_entry(&self, which: usize) -> Option<MadtStructure> {
    let madt_header_length = self.addr.as_ref::<MadtRaw>().header.length as usize;
    let mut offset = 44;
    for _ in 0..which {
        let addr = self.addr.add(offset);
        let structure = addr.as_ref::<MadtStructureHeader>();
        offset += structure.length as usize;
        if offset >= madt_header_length {
            return None;
        }
    }
    let addr = self.addr.add(offset);
    match addr.as_ref::<MadtStructureHeader>().mtype {
        MADT_TYPE_RINTC => {
            let rintc = addr.as_ref::<Rintc>();
            Some(MadtStructure::Rintc(rintc.clone()))
        }
        MADT_TYPE_IMSIC => {
            let imsic = addr.as_ref::<Imsic>();
            Some(MadtStructure::Imsic(imsic.clone()))
        }
        MADT_TYPE_APLIC => {
            let aplic = addr.as_ref::<Aplic>();
            Some(MadtStructure::Aplic(aplic.clone()))
        }
        x => {
            debugln!("ACPI MADT: Unhandled structure type 0x{:02X}.", x);
            None
        }
    }
}

#[repr(C)]
pub struct McfgRaw {
    pub header: DescriptionHeader,
    pub reserved: u64,
    // Allocation Entries
}

pub fn get_entry(&self, which: usize) -> Option<McfgEntry> {
    let mcfg = self.addr.as_ref::<McfgRaw>();
    let len = mcfg.header.length as usize;
    // 44 bytes to get to allocation entries
    // each entry is 16 bytes.
    let num_entries = (len - 44) / 16
    if which >= num_entries {
        return None;
    }
    let addr = self.addr.add(44 + which * 16);
    let entry = addr.as_ref::<McfgEntry>().clone();
    Some(entry)
}

#[repr(C)]
#[derive(Clone)]
pub struct McfgEntry {
    pub base: u64,
    pub pci_seg_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
    pub reserved: u32,
}

#[repr(C,packed)]
pub struct SpcrRaw {
    pub header: DescriptionHeader,
    pub interface_type: u8, // 0 = Full 16550, 1 = 16450
    pub reserved1: [u8; 3],
    pub base_address: [u8; 12],
    pub interrupt_type: u8, // bit index #4, 1 = supported, 0 = not supported
    pub irq: u8,
    pub global_system_interrupt: u32,
    pub configured_baud_rate: u8, // 7 = 115200
    pub parity: u8,               // 0 = no parity
    pub stop_bits: u8,            // 1 = 1 stop bit
    pub flow_control: u8,         // bit 0 = DCD, bit 1 = RTS/CTS, bit 2 = XON/XOFF
    pub terminal_type: u8,        // 0 = VT100, 1 = Extended VT100, 2 = VT-UTF8, 3 = ANSI
    pub language: u8,             // Must be 0
    pub pci_dev_id: u16,          // 0xFFFF if not PCI
    pub pci_ven_id: u16,          // 0xFFFF if not PCI
    pub pci_bus_num: u8,          // 0x00 if not PCI
    pub pci_dev_num: u8,          // 0x00 if not PCI,
    pub pci_func_num: u8,         // 0x00 if not PCI,
    pub pci_flags: u32,           // bit 0 should be 0, may be 1 if not PCI
    pub pci_seg: u8,
    pub uart_freq: u32,
    pub precise_baud: u32,
    pub namespace_str_len: u16, // Length, in bytes, of NamespaceString, including NUL characters.
    pub namespace_str_off: u16, // Offset, in bytes, from the beginning of this structure to the field NamespaceString[]. This value must be valid because this string must be present.
                                // namespace strings
}

#[repr(C,packed)]
pub struct RhctRaw {
    pub header: DescriptionHeader,
    pub flags: u32,
    pub time_base_freq: u64,
    pub num_nodes: u32,
    pub offset_to_nodes: u32,
    // Nodes
}

#[repr(C, packed)]
pub struct RhctMmuNode {
    pub header: RhctNodeHeader,
    pub reserved: u8,
    pub mmu_type: u8,
}

#[repr(C, packed)]
pub struct RhctIsaNode {
    pub header: RhctNodeHeader,
    pub isa_length: u16, // includes NULL terminator
                         // isa_string N at offset 8. NULL terminated ASCII.
}

#[repr(C, packed)]
pub struct RhctHartNode {
    pub header: RhctNodeHeader,
    pub num_offsets: u16,
    pub acpi_uid: u32,
    // Offsets[N] each u32
}
