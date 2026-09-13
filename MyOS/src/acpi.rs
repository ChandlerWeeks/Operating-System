//! # COSC562 Fall 2026 Operating System
//!
//! Contains structures to model the ACPI and it's internal tables, such as the RSDT, XSDT, MADT,
//! MCFG, SPCR, and RHCT.
//!
//! Jason Weeks - jweeks12
//! September 13th, 2026

use crate::limine;
use core::fmt;

#[repr(C,packed)]
/// RSDP fields used by ACPI version 1.
pub struct RsdtRawV1 {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8, // ACPI RSDP revision.
    pub rsdt_addr: u32,
}

#[repr(C, packed)]
/// Extra RSDP fields used by ACPI version 2 or later.
pub struct RsdtRawV2 {
    pub rawv1: RsdtRawV1,
    pub length: u32,
    pub xsdt_addr: u64,
    pub extended_checksum: u8,
    pub reservedd: [u8; 3],
}

// The RSDP signature includes the final space.
const RSDP_SIGNATURE: &[u8; 8] = b"RSD PTR ";

#[repr(C,packed)]
/// Header at the start of each ACPI description table.
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
/// XSDT header. The table entries follow this header.
pub struct XsdtRaw {
    pub header: DescriptionHeader,
    // Entries are 64-bit physical addresses.
}

#[derive(Clone)]
/// XSDT table reader from ACPI.
pub struct Xsdt {
    vaddr: VirtualAddress,
}
impl Xsdt {
    /// Build an XSDT reader from the RSDP virtual address.
    pub fn from_rsdp(rsdp_vaddr: usize) -> Option<Self> {
        let rsdp = VirtualAddress {
            virtual_address: rsdp_vaddr,
        };

        let signature = unsafe { rsdp.read_byte_offset::<[u8; 8]>(0) };
        let revision = unsafe { rsdp.read_byte_offset::<u8>(15) };
        if signature != *RSDP_SIGNATURE || revision < 2 {
            return None;
        }

        let xsdt_paddr = unsafe { rsdp.read_byte_offset::<u64>(24) } as usize;
        if xsdt_paddr == 0 {
            return None;
        }

        Some(Self {
            vaddr: VirtualAddress::new_from_phys(xsdt_paddr),
        })
    }

    /// Iterate over the ACPI tables in this XSDT.
    pub fn iter(&self) -> XsdtIter {
        XsdtIter::new(self)
    }

    /// Get the number of XSDT entries.
    pub fn num_entries(&self) -> usize {
        let header = unsafe { self.vaddr.read_byte_offset::<DescriptionHeader>(0) };
        let table_length = header.length as usize;

        table_length.saturating_sub(36) / 8
    }

    /// Get one XSDT entry by index.
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
    pub next_entry: usize, // Current XSDT entry index.
}
impl XsdtIter {
    /// Make an iterator for this XSDT.
    pub fn new(xsdt: &Xsdt) -> Self {
        Self {
            xsdt: xsdt.clone(),
            next_entry: 0,
        }
    }
}

impl Iterator for XsdtIter {
    type Item = SystemTableType;

    /// Get the next XSDT entry.
    fn next(&mut self) -> Option<Self::Item> {
        let entry = self.xsdt.get_entry(self.next_entry)?;
        self.next_entry += 1;
        Some(entry)
    }
}

#[derive(Clone)]
/// ACPI table address after physical-to-virtual conversion.
pub struct VirtualAddress {
    virtual_address: usize,
}

impl VirtualAddress {
    /// Convert an ACPI physical address to a virtual address.
    fn new_from_phys(physical_address: usize) -> Self {
        Self {
            virtual_address: limine::pa_to_va(physical_address),
        }
    }

    /// Add a byte offset to this address.
    fn add(&self, offset: usize) -> Self {
        Self {
            virtual_address: self
                .virtual_address
                .checked_add(offset)
                .expect("virtual-address offset overflow"),
        }
    }

    /// Read a value at a byte offset from this address.
    unsafe fn read_byte_offset<T>(&self, offset: usize) -> T {
        let address = self
            .virtual_address
            .checked_add(offset)
            .expect("virtual-address offset overflow");

        unsafe { (address as *const T).read_unaligned() }
    }
}

/// ACPI table type found in the XSDT.
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
    /// Get the MADT reader from this entry.
    pub fn madt(&self) -> Option<Madt> {
        match self {
            Self::Madt(vaddr) => Some(Madt::new(vaddr.clone())),
            _ => None,
        }
    }

    /// Get the MCFG reader from this entry.
    pub fn mcfg(&self) -> Option<Mcfg> {
        match self {
            Self::Mcfg(vaddr) => Some(Mcfg::new(vaddr.clone())),
            _ => None,
        }
    }

    /// Get the SPCR reader from this entry.
    pub fn spcr(&self) -> Option<Spcr> {
        match self {
            Self::Spcr(vaddr) => Some(Spcr::new(vaddr.clone())),
            _ => None,
        }
    }

    /// Get the RHCT reader from this entry.
    pub fn rhct(&self) -> Option<Rhct> {
        match self {
            Self::Rhct(vaddr) => Some(Rhct::new(vaddr.clone())),
            _ => None,
        }
    }
}



pub const KERNEL_IO_ADDR: usize = 0xffff_beef_0000_0000;

/// MADT fixed fields. Controller entries follow this header.
#[repr(C)]
pub struct MadtRaw {
    pub header: DescriptionHeader,
    pub local_controller_addr: u32,
    pub flags: u32,
    // Controller entries follow the fixed fields.
}

/// MADT interrupt-controller entry.
pub enum MadtStructure {
    Rintc(Rintc),
    Imsic(Imsic),
    Aplic(Aplic),
    Unknown(u8),
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
/// Header for a variable-length MADT entry.
pub struct MadtStructureHeader {
    mtype: u8,
    length: u8,
}

// RISC-V MADT entry type values.
const MADT_TYPE_RINTC: u8 = 24;
const MADT_TYPE_IMSIC: u8 = 25;
const MADT_TYPE_APLIC: u8 = 26;

/// RINTC entry for one RISC-V hart.
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

impl Rintc {
    /// Get the hardware hart ID.
    pub fn hart_id(&self) -> u64 {
        unsafe { core::ptr::addr_of!(self.hart_id).read_unaligned() }
    }

    /// Get the IMSIC base address for this hart.
    pub fn imsic_base_address(&self) -> u64 {
        unsafe { core::ptr::addr_of!(self.imsic_base_address).read_unaligned() }
    }
}

#[repr(C, packed)]
#[derive(Clone)]
/// IMSIC configuration in the MADT.
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

impl Imsic {
    /// Get the IMSIC version.
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Get the supported interrupt ID count.
    pub fn num_sup_interrupt_ids(&self) -> u16 {
        unsafe { core::ptr::addr_of!(self.num_sup_interrupt_ids).read_unaligned() }
    }

    /// Get the guest interrupt ID count.
    pub fn num_guest_interrupt_ids(&self) -> u16 {
        unsafe { core::ptr::addr_of!(self.num_guest_interrupt_ids).read_unaligned() }
    }

    /// Get the hart index bit count.
    pub fn hart_index_bits(&self) -> u8 {
        self.hart_index_bits
    }
}

impl fmt::Debug for Imsic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mtype = unsafe { core::ptr::addr_of!(self.header.mtype).read_unaligned() };
        let length = unsafe { core::ptr::addr_of!(self.header.length).read_unaligned() };
        let flags = unsafe { core::ptr::addr_of!(self.flags).read_unaligned() };
        let supported = unsafe { core::ptr::addr_of!(self.num_sup_interrupt_ids).read_unaligned() };
        let guest = unsafe { core::ptr::addr_of!(self.num_guest_interrupt_ids).read_unaligned() };

        f.debug_struct("Imsic")
            .field("mtype", &mtype)
            .field("length", &length)
            .field("version", &self.version)
            .field("reserved", &self.reserved)
            .field("flags", &flags)
            .field("num_sup_interrupt_ids", &supported)
            .field("num_guest_interrupt_ids", &guest)
            .field("guest_index_bits", &self.guest_index_bits)
            .field("hart_index_bits", &self.hart_index_bits)
            .field("group_index_bits", &self.group_index_bits)
            .field("group_index_shift", &self.group_index_shift)
            .finish()
    }
}

/// APLIC configuration in the MADT.
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

impl Aplic {
    /// Get the APLIC MMIO address.
    pub fn aplic_address(&self) -> u64 {
        unsafe { core::ptr::addr_of!(self.aplic_address).read_unaligned() }
    }
}

impl fmt::Debug for Aplic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mtype = unsafe { core::ptr::addr_of!(self.header.mtype).read_unaligned() };
        let length = unsafe { core::ptr::addr_of!(self.header.length).read_unaligned() };
        let flags = unsafe { core::ptr::addr_of!(self.flags).read_unaligned() };
        let hardware_id = unsafe { core::ptr::addr_of!(self.hardware_id).read_unaligned() };
        let num_idcs = unsafe { core::ptr::addr_of!(self.num_idcs).read_unaligned() };
        let sources = unsafe { core::ptr::addr_of!(self.total_external_sources).read_unaligned() };
        let irq_base = unsafe { core::ptr::addr_of!(self.global_system_irq_base).read_unaligned() };
        let size = unsafe { core::ptr::addr_of!(self.aplic_size).read_unaligned() };

        f.debug_struct("Aplic")
            .field("mtype", &mtype)
            .field("length", &length)
            .field("version", &self.version)
            .field("aplic_id", &self.aplic_id)
            .field("flags", &flags)
            .field("hardware_id", &hardware_id)
            .field("num_idcs", &num_idcs)
            .field("total_external_sources", &sources)
            .field("global_system_irq_base", &irq_base)
            .field("aplic_size", &size)
            .finish()
    }
}

/// MADT table reader from ACPI.
pub struct Madt {
    vaddr: VirtualAddress,
}
impl Madt {
    /// Build a MADT reader from an XSDT entry.
    fn new(vaddr: VirtualAddress) -> Self {
        Self { vaddr }
    }

    /// Get one MADT entry by index.
    pub fn get_entry(&self, which: usize) -> Option<MadtStructure> {
        let madt_header_length = unsafe { self.vaddr.read_byte_offset::<u32>(4) } as usize;
        let mut offset = 44;
        for _ in 0..which {
            if offset + 2 > madt_header_length {
                return None;
            }
            let addr = self.vaddr.add(offset);
            let length = unsafe { addr.read_byte_offset::<u8>(1) } as usize;
            if length < 2 || offset + length > madt_header_length {
                return None;
            }
            offset += length;
        }
        if offset + 2 > madt_header_length {
            return None;
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
            x => Some(MadtStructure::Unknown(x)),
        }
    }
}

/// MCFG fixed fields. PCI ECAM entries follow this header.
#[repr(C)]
pub struct McfgRaw {
    pub header: DescriptionHeader,
    pub reserved: u64,
    // PCI ECAM allocation entries follow this header.
}

/// MCFG table reader from ACPI.
pub struct Mcfg {
    vaddr: VirtualAddress,
}
impl Mcfg {
    /// Build an MCFG reader from an XSDT entry.
    fn new(vaddr: VirtualAddress) -> Self {
        Self { vaddr }
    }

    /// Get the PCI ECAM allocation entry count.
    pub fn num_entries(&self) -> usize {
        let length = unsafe { self.vaddr.read_byte_offset::<u32>(4) } as usize;
        length.saturating_sub(44) / 16
    }

    /// Get one PCI ECAM entry by index.
    pub fn get_entry(&self, which: usize) -> Option<McfgEntry> {
        if which >= self.num_entries() {
            return None;
        }
        let addr = self.vaddr.add(44 + which * 16);
        let entry = unsafe { addr.read_byte_offset::<McfgEntry>(0) };
        Some(entry)
    }
}

/// One PCI ECAM allocation entry.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct McfgEntry {
    pub base: u64,
    pub pci_seg_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
    pub reserved: u32,
}

/// ACPI Generic Address Structure.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct GenericAddress {
    pub address_space_id: u8,
    pub register_bit_width: u8,
    pub register_bit_offset: u8,
    pub access_size: u8,
    pub address: u64,
}

/// SPCR table fields.
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
    pub namespace_str_len: u16, // Namespace string length, including NUL.
    pub namespace_str_off: u16, // Namespace string offset from this table.
                                // Namespace string bytes follow the fixed fields.
}

/// SPCR table reader from ACPI.
pub struct Spcr {
    vaddr: VirtualAddress,
}

impl Spcr {
    /// Build an SPCR reader from an XSDT entry.
    fn new(vaddr: VirtualAddress) -> Self {
        Self { vaddr }
    }

    /// Get the UART base address.
    pub fn base_address(&self) -> u64 {
        unsafe { self.vaddr.read_byte_offset::<u64>(44) }
    }

    /// Get the UART address-space ID.
    pub fn address_space_id(&self) -> u8 {
        unsafe { self.vaddr.read_byte_offset::<u8>(40) }
    }

    /// Get the UART interrupt type flags.
    pub fn interrupt_type(&self) -> u8 {
        unsafe { self.vaddr.read_byte_offset::<u8>(52) }
    }

    /// Get the UART IRQ number.
    pub fn irq(&self) -> u8 {
        unsafe { self.vaddr.read_byte_offset::<u8>(53) }
    }

    /// Get the UART clock frequency in hertz.
    pub fn uart_frequency(&self) -> u32 {
        unsafe { self.vaddr.read_byte_offset::<u32>(76) }
    }
}

/// RHCT table fields.
#[repr(C, packed)]
pub struct RhctRaw {
    pub header: DescriptionHeader,
    pub flags: u32,
    pub time_base_freq: u64,
    pub num_nodes: u32,
    pub offset_to_nodes: u32,
    // RHCT nodes follow the fixed fields.
}

/// Header for one variable-length RHCT node.
#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct RhctNodeHeader {
    pub node_type: u16,
    pub length: u16,
    pub revision: u16,
}

/// RHCT table reader from ACPI.
pub struct Rhct {
    vaddr: VirtualAddress,
}

impl Rhct {
    /// Build an RHCT reader from an XSDT entry.
    fn new(vaddr: VirtualAddress) -> Self {
        Self { vaddr }
    }

    /// Get the RISC-V timer frequency in hertz.
    pub fn time_base_freq(&self) -> u64 {
        unsafe { self.vaddr.read_byte_offset::<u64>(40) }
    }

    /// Get the RHCT node count.
    pub fn num_nodes(&self) -> u32 {
        unsafe { self.vaddr.read_byte_offset::<u32>(48) }
    }

    /// Get the RHCT virtual address.
    pub fn virtual_address(&self) -> usize {
        self.vaddr.virtual_address
    }

    /// Get the RHCT physical address.
    pub fn physical_address(&self) -> usize {
        limine::va_to_pa(self.virtual_address())
    }
}

impl fmt::Debug for Rhct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Rhct {{ addr: {{ VA: {:#018x}, PA: {:#010x} }} }}",
            self.virtual_address(),
            self.physical_address()
        )
    }
}

#[repr(C, packed)]
/// RHCT MMU capability node.
pub struct RhctMmuNode {
    pub header: RhctNodeHeader,
    pub reserved: u8,
    pub mmu_type: u8,
}

#[repr(C, packed)]
/// RHCT ISA string node.
pub struct RhctIsaNode {
    pub header: RhctNodeHeader,
    pub isa_length: u16, // Includes the NUL terminator.
                         // ISA string bytes start at offset 8.
}

#[repr(C, packed)]
/// RHCT hart capability node.
pub struct RhctHartNode {
    pub header: RhctNodeHeader,
    pub num_offsets: u16,
    pub acpi_uid: u32,
    // A u32 offset follows for each referenced node.
}
