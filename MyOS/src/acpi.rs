use crate::debugln;
use crate::limine;

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

const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR "; // Identifies the RSDP.

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
    vaddr: VirtualAddress,
}
impl Xsdt {
    pub fn num_entries(&self) -> usize {
        let header = unsafe { self.vaddr.read_byte_offset::<DescriptionHeader>(0) };
        let table_length = header.length as usize;

        table_length.saturating_sub(36) / 8
    }

    pub fn get_entry(&self, which: usize) -> Option<SystemTableType> {
        if which >= self.num_entries() {
            return None;
        }
        let addr = unsafe { self.vaddr.read_byte_offset::<u64>(36 + which * 8) };
        let structure = VirtualAddress::new_from_phys(addr as usize);
        let sig = unsafe { structure.read_byte_offset::<[u8; 4]>(0) };
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
}


pub struct XsdtIter {
    pub xsdt: Xsdt,
    pub next_entry: usize, // all indexes in rust are usize
}
impl XsdtIter {
    pub fn new(xsdt: &Xsdt) -> Self {
        Self {
            xsdt: xsdt.clone(),
            next_entry: 0,
        }
    }
}

impl Iterator for XsdtIter {
    type Item = SystemTableType;

    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.xsdt.get_entry(self.next_entry)?;
        self.next_entry += 1;
        Some(entry)
    }
}

#[derive(Clone)]
pub struct VirtualAddress {
    virtual_address: usize,
}

impl VirtualAddress {
    /// Converts an ACPI physical address to an HHDM virtual address.
    fn new_from_phys(physical_address: usize) -> Self {
        Self {
            virtual_address: limine::pa_to_va(physical_address),
        }
    }

    /// Returns an address at a byte offset from this address.
    fn add(&self, offset: usize) -> Self {
        Self {
            virtual_address: self
                .virtual_address
                .checked_add(offset)
                .expect("virtual-address offset overflow"),
        }
    }

    /// Reads a value of type T at a byte offset from this virtual address.
    unsafe fn read_byte_offset<T>(&self, offset: usize) -> T {
        let address = self
            .virtual_address
            .checked_add(offset)
            .expect("virtual-address offset overflow");

        unsafe { (address as *const T).read_unaligned() }
    }
}

/// Classifies an ACPI table by its signature.
pub enum SystemTableType {
    Bgrt(VirtualAddress),
    Facs(VirtualAddress),
    Rhct(VirtualAddress),
    Madt(VirtualAddress),
    Spcr(VirtualAddress),
    Mcfg(VirtualAddress),
    Unknown(VirtualAddress),
}

impl SystemTableType {
    /// Returns an SPCR reader when this entry is an SPCR table.
    pub fn spcr(&self) -> Option<Spcr> {
        match self {
            Self::Spcr(vaddr) => Some(Spcr::new(vaddr.clone())),
            _ => None,
        }
    }

    /// Returns an RHCT reader when this entry is an RHCT table.
    pub fn rhct(&self) -> Option<Rhct> {
        match self {
            Self::Rhct(vaddr) => Some(Rhct::new(vaddr.clone())),
            _ => None,
        }
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

/// Represents one RISC-V interrupt-controller entry in the MADT.
enum MadtStructure {
    Rintc(Rintc),
    Imsic(Imsic),
    Aplic(Aplic),
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct MadtStructureHeader {
    mtype: u8,
    length: u8,
}

const MADT_TYPE_RINTC: u8 = 24;
const MADT_TYPE_IMSIC: u8 = 25;
const MADT_TYPE_APLIC: u8 = 26;

/// Describes one per-hart RISC-V interrupt-controller entry in the MADT.
#[repr(C, packed)]
#[derive(Clone)]
pub struct Rintc {
    pub header: MadtStructureHeader,
    pub version: u8,
    pub reserved: u8,
    pub flags: u32,
    pub hart_id: u64,
    pub acpi_processor_uid: u32,
    pub external_interrupt_controller_id: u32,
    pub imsic_base_address: u64,
    pub imsic_size: u32,
}

/// Describes the system-wide RISC-V incoming MSI-controller configuration.
#[repr(C, packed)]
#[derive(Clone)]
pub struct Imsic {
    pub header: MadtStructureHeader,
    pub version: u8,
    pub reserved: u8,
    pub flags: u32,
    pub num_sup_interrupt_ids: u16,
    pub num_guest_interrupt_ids: u16,
    pub guest_index_bits: u8,
    pub hart_index_bits: u8,
    pub group_index_bits: u8,
    pub group_index_shift: u8,
}

/// Describes one RISC-V advanced platform-level interrupt controller.
#[repr(C, packed)]
#[derive(Clone)]
pub struct Aplic {
    pub header: MadtStructureHeader,
    pub version: u8,
    pub aplic_id: u8,
    pub flags: u32,
    pub hardware_id: [u8; 8],
    pub num_idcs: u16,
    pub total_external_sources: u16,
    pub global_system_irq_base: u32,
    pub aplic_address: u64,
    pub aplic_size: u32,
}

struct Madt {
    vaddr: VirtualAddress,
}
impl Madt {
    pub fn get_entry(&self, which: usize) -> Option<MadtStructure> {
        let madt_header_length = unsafe { self.vaddr.read_byte_offset::<u32>(4) } as usize;
        let mut offset = 44;
        for _ in 0..which {
            let addr = self.vaddr.add(offset);
            let length = unsafe { addr.read_byte_offset::<u8>(1) } as usize;
            if length == 0 {
                return None;
            }
            offset += length;
            if offset >= madt_header_length {
                return None;
            }
        }
        let addr = self.vaddr.add(offset);
        match unsafe { addr.read_byte_offset::<u8>(0) } {
            MADT_TYPE_RINTC => {
                let rintc = unsafe { addr.read_byte_offset::<Rintc>(0) };
                Some(MadtStructure::Rintc(rintc))
            }
            MADT_TYPE_IMSIC => {
                let imsic = unsafe { addr.read_byte_offset::<Imsic>(0) };
                Some(MadtStructure::Imsic(imsic))
            }
            MADT_TYPE_APLIC => {
                let aplic = unsafe { addr.read_byte_offset::<Aplic>(0) };
                Some(MadtStructure::Aplic(aplic))
            }
            x => {
                debugln!("ACPI MADT: Unhandled structure type 0x{:02X}.", x);
                None
            }
        }
    }
}

#[repr(C)]
pub struct McfgRaw {
    pub header: DescriptionHeader,
    pub reserved: u64,
    // Allocation Entries
}

struct Mcfg {
    vaddr: VirtualAddress,
}
impl Mcfg {
    pub fn get_entry(&self, which: usize) -> Option<McfgEntry> {
        let len = unsafe { self.vaddr.read_byte_offset::<u32>(4) } as usize;
        // 44 bytes to get to allocation entries
        // each entry is 16 bytes.
        let num_entries = (len - 44) / 16;
        if which >= num_entries {
            return None;
        }
        let addr = self.vaddr.add(44 + which * 16);
        let entry = unsafe { addr.read_byte_offset::<McfgEntry>(0) };
        Some(entry)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct McfgEntry {
    pub base: u64,
    pub pci_seg_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
    pub reserved: u32,
}

/// Describes a register address in an ACPI table.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GenericAddress {
    pub address_space_id: u8,
    pub register_bit_width: u8,
    pub register_bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

/// Contains serial-console settings from the SPCR table.
#[repr(C, packed)]
pub struct SpcrRaw {
    pub header: DescriptionHeader,
    pub interface_type: u8, // 0 = Full 16550, 1 = 16450
    pub reserved1: [u8; 3],
    pub base_address: GenericAddress,
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

/// Reads the serial-console address from an SPCR table.
pub struct Spcr {
    vaddr: VirtualAddress,
}

impl Spcr {
    /// Creates an SPCR reader for an XSDT SPCR table address.
    fn new(vaddr: VirtualAddress) -> Self {
        Self { vaddr }
    }

    /// Returns the UART base address from the SPCR Generic Address Structure.
    pub fn base_address(&self) -> u64 {
        unsafe { self.vaddr.read_byte_offset::<u64>(44) }
    }

    /// Returns the SPCR address-space identifier.
    pub fn address_space_id(&self) -> u8 {
        unsafe { self.vaddr.read_byte_offset::<u8>(40) }
    }
}

/// Contains RISC-V hart capabilities and the timer frequency.
#[repr(C, packed)]
pub struct RhctRaw {
    pub header: DescriptionHeader,
    pub flags: u32,
    pub time_base_freq: u64,
    pub num_nodes: u32,
    pub offset_to_nodes: u32,
    // Nodes
}

/// Describes one variable-length RHCT node.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RhctNodeHeader {
    pub node_type: u16,
    pub length: u16,
    pub revision: u16,
}

/// Reads timer data from an RHCT table.
pub struct Rhct {
    vaddr: VirtualAddress,
}

impl Rhct {
    /// Creates an RHCT reader for an XSDT RHCT table address.
    fn new(vaddr: VirtualAddress) -> Self {
        Self { vaddr }
    }

    /// Returns the RISC-V timer frequency in ticks per second.
    pub fn time_base_freq(&self) -> u64 {
        unsafe { self.vaddr.read_byte_offset::<u64>(40) }
    }

    /// Returns the number of RHCT nodes.
    pub fn num_nodes(&self) -> u32 {
        unsafe { self.vaddr.read_byte_offset::<u32>(48) }
    }
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
