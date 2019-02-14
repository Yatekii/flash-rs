use crate::flash_algorithm::FlashAlgorithm;

pub struct MemoryMap {
    regions: Vec<MemoryRegion>,
}

impl MemoryMap {
    pub fn new(regions: Vec<MemoryRegion>) -> Self {
        Self {
            regions: regions,
        }
    }
}

impl MemoryMap {
    pub fn get_region_for_address(&self, address: u32) -> Option<MemoryRegion> {
        for r in self.regions {
            if r.contains_address(address) {
                return Some(r);
            }
        }
        None
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct MemoryRegion {
    pub(crate) typ: RegionType,
    pub(crate) start: u32,
    pub(crate) length: u32,
    pub(crate) blocksize: u32,
    pub(crate) algorithm: Option<FlashAlgorithm>,
}

impl MemoryRegion {
    const erased_byte_value: u8 = 0x00;

    pub fn end(&self) -> u32 {
        self.start + self.length
    }

    pub fn contains_address(&self, address: u32) -> bool {
        (address >= self.start) && (address <= self.end())
    }

    /// Helper method to check if a block of data is erased.
    pub fn is_erased(self, d: &[u8]) -> bool {
        for b in d {
            if *b != Self::erased_byte_value {
                return false;
            }
        }
        true
    }
}

#[derive(PartialEq, Eq, Hash)]
pub enum RegionType {
    Other,
    Ram,
    Rom,
    Flash,
    Device,
}