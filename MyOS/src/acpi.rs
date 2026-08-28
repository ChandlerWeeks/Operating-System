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
