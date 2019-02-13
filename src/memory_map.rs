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
    pub fn end(&self) -> u32 {
        self.start + self.length
    }

    fn contains_address(&self, address: u32) -> bool {
        (address >= self.start) && (address <= self.end())
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