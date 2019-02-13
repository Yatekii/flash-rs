use std::collections::HashMap;
use crate::memory_map::{
    MemoryRegion,
    RegionType,
};
use crate::builder::FlashBuilder;
use crate::memory_map::MemoryMap;
use std::path::Path;
use std::io::{ Read, Seek, SeekFrom };
use std::fs::File;
use ihex;

pub struct Ranges<I: Iterator<Item=usize> + Sized> {
    list: I,
    start_item: Option<usize>,
    last_item: Option<usize>
}

impl<I: Iterator<Item=usize> + Sized> Ranges<I> {
    pub fn new(list: I) -> Self {
        Self {
            list,
            start_item: None,
            last_item: Some(usize::max_value() - 1)
        }
    }
}

impl<I: Iterator<Item=usize> + Sized> Iterator for Ranges<I> {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<(usize, usize)> {
        let r;
        if self.start_item.is_none() {
            self.start_item = self.list.next();
            self.last_item = self.start_item;
        }
        loop {
            if let Some(item) = self.list.next() {
                if item == self.last_item.unwrap() + 1 {
                    self.last_item = Some(item);
                } else {
                    r = (self.start_item.unwrap(), self.last_item.unwrap());
                    self.last_item = Some(item);
                    self.start_item = self.last_item;
                    break;
                }
            } else {
                if let Some(last_item) = self.last_item {
                    self.last_item = None;
                    return Some((self.start_item.unwrap(), last_item));
                } else {
                    return None;
                }
            }
        }

        Some(r)
    }
}

/// Accepts a sorted list of byte addresses. Breaks the addresses into contiguous ranges.
/// Yields 2-tuples of the start and end address for each contiguous range.

/// For instance, the input [0, 1, 2, 3, 32, 33, 34, 35] will yield the following 2-tuples:
/// (0, 3) and (32, 35).
pub fn ranges<I: Iterator<Item = usize>>(list: I)-> Ranges<I> {
    Ranges::new(list)
}

pub struct BinOptions {
    /// Memory address at which to program the binary data. If not set, the base
    /// of the boot memory will be used.
    base_address: Option<u32>,
    /// Number of bytes to skip at the start of the binary file. Does not affect the
    /// base address.
    skip: u32,
}

pub enum Format {
    Bin(BinOptions),
    Hex,
    Elf,
}

/// This struct and impl bundle functionality to start the `Downloader` which then will flash
/// the given data to the flash of the target.
/// 
/// Supported file formats are:
/// - Binary (.bin)
/// - Intel Hex (.hex)
/// - ELF (.elf or .axf)
pub struct FileDownloader;

impl FileDownloader {
    pub fn new() -> Self {
        Self
    }

    /// Downloads a file at `path` into flash.
    pub fn download_file(self, path: &Path, format: Format, memory_map: MemoryMap) -> Result<(), ()> {
        let file = File::open(path).unwrap();

        // IMPORTANT: Change this to an actual memory map of a real
        let mut loader = FlashLoader::new(memory_map);

        match format {
            Format::Bin(options) => self.download_bin(&mut file, &mut loader, options),
            Format::Elf => self.download_elf(&mut file, &mut loader),
            Format::Hex => self.download_hex(&mut file, &mut loader),
        };

        loader.commit();

        Ok(())
    }

    /// Starts the download of a binary file.
    fn download_bin<T: Read + Seek>(self, file: &mut T, loader: &mut FlashLoader, options: BinOptions) -> Result<(), ()> {
        // Skip the specified bytes.
        file.seek(SeekFrom::Start(options.skip as u64));
        
        let mut data = vec![];
        file.read_to_end(&mut data);

        loader.add_data(
            if let Some(address) = options.base_address {
                address
            } else {
                // If no base address is specified use the start of the boot memory.
                // TODO: Implement this as soon as we know targets.
                // self._session.target.memory_map.get_boot_memory().start
                0
            },
            data.as_slice()
        );

        Ok(())
    }

    /// Starts the download of a hex file.
    fn download_hex<T: Read + Seek>(self, file: &mut T, loader: &mut FlashLoader) -> Result<(), ()> {
        let mut data: String;
        file.read_to_string(&mut data);

        for item in ihex::reader::Reader::new(&data) {
            if let Ok(record) = item {
                println!("{:?}", record);
            } else {
                return Err(());
            }
        }
        Ok(())

        // hexfile = IntelHex(file_obj)
        // addresses = hexfile.addresses()
        // addresses.sort()

        // data_list = list(ranges(addresses))
        // for start, end in data_list:
        //     size = end - start + 1
        //     data = list(hexfile.tobinarray(start=start, size=size))
        //     self._loader.add_data(start, data)
    }
        
    /// Starts the download of a elf file.
    fn download_elf<T: Read + Seek>(self, file: &mut T, loader: &mut FlashLoader) -> Result<(), ()> {
    // TODO:
    //     elf = ELFBinaryFile(file_obj, self._session.target.memory_map)
    //     for section in elf.sections:
    //         if ((section.type == 'SHT_PROGBITS')
    //                 and ((section.flags & (SH_FLAGS.SHF_ALLOC | SH_FLAGS.SHF_WRITE)) == SH_FLAGS.SHF_ALLOC)
    //                 and (section.length > 0)
    //                 and (section.region.is_flash)):
    //             LOG.debug("Writing section %s", repr(section))
    //             self._loader.add_data(section.start, section.data)
    //         else:
    //             LOG.debug("Skipping section %s", repr(section))
        Ok(())
    }
}

// class FlashEraser(object):
//     """! @brief Class that manages high level flash erasing.
    
//     Can erase a target in one of three modes:
//     - chip erase: Erase all flash on the target.
//     - mass erase: Also erase all flash on the target. However, on some targets, a mass erase has
//         special properties such as unlocking security or erasing additional configuration regions
//         that are not erased by a chip erase. If a target does not have a special mass erase, then
//         it simply reverts to a chip erase.
//     - sector erase: One or more sectors are erased.
//     """
//     class Mode(Enum):
//         MASS = 1
//         CHIP = 2
//         SECTOR = 3
    
//     def __init__(self, session, mode):
//         """! @brief Constructor.
        
//         @param self
//         @param session The session instance.
//         @param mode One of the FlashEraser.Mode enums to select mass, chip, or sector erase.
//         """
//         self._session = session
//         self._mode = mode
    
//     def erase(self, addresses=None):
//         """! @brief Perform the type of erase operation selected when the object was created.
        
//         For sector erase mode, an iterable of sector addresses specifications must be provided via
//         the _addresses_ parameter. The address iterable elements can be either strings, tuples,
//         or integers. Tuples must have two elements, the start and end addresses of a range to erase.
//         Integers are simply an address within the single page to erase.
        
//         String address specifications may be in one of three formats: "<address>", "<start>-<end>",
//         or "<start>+<length>". Each field denoted by angled brackets is an integer literal in
//         either decimal or hex notation.
        
//         Examples:
//         - "0x1000" - erase the one sector at 0x1000
//         - "0x1000-0x4fff" - erase sectors from 0x1000 up to but not including 0x5000
//         - "0x8000+0x800" - erase sectors starting at 0x8000 through 0x87ff
        
//         @param self
//         @param addresses List of addresses or address ranges of the sectors to erase.
//         """
//         if self._mode == self.Mode.MASS:
//             self._mass_erase()
//         elif self._mode == self.Mode.CHIP:
//             self._chip_erase()
//         elif self._mode == self.Mode.SECTOR and addresses:
//             self._sector_erase(addresses)
//         else:
//             LOG.warning("No operation performed")
    
//     def _mass_erase(self):
//         LOG.info("Mass erasing device...")
//         if self._session.target.mass_erase():
//             LOG.info("Successfully erased.")
//         else:
//             LOG.error("Mass erase failed.")
    
//     def _chip_erase(self):
//         LOG.info("Erasing chip...")
//         # Erase all flash regions. This may be overkill if either each region's algo erases
//         # all regions on the chip. But there's no current way to know whether this will happen,
//         # so prefer to be certain.
//         for region in self._session.target.memory_map.get_regions_of_type(MemoryType.FLASH):
//             if region.flash is not None:
//                 if region.flash.is_erase_all_supported:
//                     region.flash.init(region.flash.Operation.ERASE)
//                     region.flash.erase_all()
//                     region.flash.cleanup()
//                 else:
//                     self._sector_erase((region.start, region.end))
//         LOG.info("Done")
    
//     def _sector_erase(self, addresses):
//         flash = None
//         currentRegion = None

//         for spec in addresses:
//             # Convert the spec into a start and end address.
//             page_addr, end_addr = self._convert_spec(spec)
            
//             while page_addr < end_addr:
//                 # Look up the flash memory region for the current address.
//                 region = self._session.target.memory_map.get_region_for_address(page_addr)
//                 if region is None:
//                     LOG.warning("address 0x%08x is not within a memory region", page_addr)
//                     break
//                 if not region.is_flash:
//                     LOG.warning("address 0x%08x is not in flash", page_addr)
//                     break
            
//                 # Handle switching regions.
//                 if region is not currentRegion:
//                     # Clean up previous flash.
//                     if flash is not None:
//                         flash.cleanup()
                
//                     currentRegion = region
//                     flash = region.flash
//                     flash.init(flash.Operation.ERASE)
        
//                 # Get page info for the current address.
//                 page_info = flash.get_page_info(page_addr)
//                 if not page_info:
//                     # Should not fail to get page info within a flash region.
//                     raise RuntimeError("sector address 0x%08x within flash region '%s' is invalid", page_addr, region.name)
                
//                 # Align first page address.
//                 delta = page_addr % page_info.size
//                 if delta:
//                     LOG.warning("sector address 0x%08x is unaligned", page_addr)
//                     page_addr -= delta
        
//                 # Erase this page.
//                 LOG.info("Erasing sector 0x%08x (%d bytes)", page_addr, page_info.size)
//                 flash.erase_page(page_addr)
                
//                 page_addr += page_info.size

//         if flash is not None:
//             flash.cleanup()

//     def _convert_spec(self, spec):
//         if isinstance(spec, six.string_types):
//             # Convert spec from string to range.
//             if '-' in spec:
//                 a, b = spec.split('-')
//                 page_addr = int(a, base=0)
//                 end_addr = int(b, base=0)
//             elif '+' in spec:
//                 a, b = spec.split('+')
//                 page_addr = int(a, base=0)
//                 length = int(b, base=0)
//                 end_addr = page_addr + length
//             else:
//                 page_addr = int(spec, base=0)
//                 end_addr = page_addr + 1
//         elif isinstance(spec, tuple):
//             page_addr = spec[0]
//             end_addr = spec[1]
//         else:
//             page_addr = spec
//             end_addr = page_addr + 1
//         return page_addr, end_addr

// ## Sentinel object used to identify an unset chip_erase parameter.
// CHIP_ERASE_SENTINEL = object()

/// Handles high level programming of raw binary data to flash.
/// 
/// If you need file programming, either binary files or other formats, please see the
/// FileProgrammer class.
/// 
/// This manager provides a simple interface to programming flash that may cross flash
/// region boundaries. To use it, create an instance and pass in the session object. Then call
/// add_data() for each chunk of binary data you need to write. When all data is added, call the
/// commit() method to write everything to flash. You may reuse a single FlashLoader instance for
/// multiple add-commit sequences.
/// 
/// When programming across multiple regions, progress reports are combined so that only a
/// one progress output is reported. Similarly, the programming performance report for each region
/// is suppresed and a combined report is logged.
/// 
/// Internally, FlashBuilder is used to optimize programming within each memory region.
pub struct FlashLoader<'a> {
    memory_map: MemoryMap,
    builders: HashMap<MemoryRegion, FlashBuilder<'a>>,
    total_data_size: usize,
    chip_erase: bool,
}

pub enum FlashLoaderError {
    MemoryRegionNotDefined(u32), // Contains the faulty address.
    MemoryRegionNotFlash(u32) // Contains the faulty address.
}

impl<'a> FlashLoader<'a> {
    pub fn new(memory_map: MemoryMap) -> Self {
        Self {
            memory_map: memory_map,
            builders: HashMap::new(),
            total_data_size: 0,
            chip_erase: false,
        }
    }
    
    /// Clear all state variables.
    fn reset_state(&mut self) {
        self.builders = HashMap::new();
        self.total_data_size = 0;
    }
    
    /// Add a chunk of data to be programmed.
    ///
    /// The data may cross flash memory region boundaries, as long as the regions are contiguous.
    /// `address` is the address where the first byte of `data` is located.
    /// `data` is an iterator of u8 bytes to be written at given `address` and onwards.
    pub fn add_data(&mut self, mut address: u32, data: &[u8]) -> Result<(), FlashLoaderError> {
        let size = data.len();
        let mut remaining = size;
        while remaining > 0 {
            // Look up flash region.
            let possible_region = self.memory_map.get_region_for_address(address);
            if let Some(region) = possible_region {
                if let RegionType::Flash = region.typ {
                    // Get our builder instance.
                    let builder = if self.builders.contains_key(&region) {
                        self.builders[&region]
                    } else {
                        // if region.flash is None:
                        //     raise RuntimeError("flash memory region at address 0x%08x has no flash instance" % address)
                        self.builders[&region] = region.flash.get_flash_builder();
                        self.builders[&region]
                    };
                
                    // Add as much data to the builder as is contained by this region.
                    let program_length = usize::min(remaining, (region.end() - address + 1) as usize);
                    builder.add_data(address, data[size - remaining..program_length]);
                    
                    // Advance the cursors.
                    remaining -= program_length;
                    address += program_length as u32;
                } else {
                    return Err(FlashLoaderError::MemoryRegionNotFlash(address));
                }
            } else {
                return Err(FlashLoaderError::MemoryRegionNotDefined(address));
            }
        }
        Ok(())
    }
    
    /// Write all collected data to flash.
        
    /// This routine ensures that chip erase is only used once if either the auto mode or chip
    /// erase mode are used. As an example, if two regions are to be written to and True was
    /// passed to the constructor for chip_erase (or if the session option was set), then only
    /// the first region will actually use chip erase. The second region will be forced to use
    /// sector erase. This will not result in extra erasing, as sector erase always verifies whether
    /// the sectors are already erased. This will, of course, also work correctly if the flash
    /// algorithm for the first region doesn't actually erase the entire chip (all regions).
    
    /// After calling this method, the loader instance can be reused to program more data.
    pub fn commit(self) {
        let mut did_chip_erase = false;
        
        // Iterate over builders we've created and program the data.
        let builders: Vec<&FlashBuilder> = self.builders.values().collect();
        builders.sort_unstable_by_key(|v| v.flash_start);
        let sorted = builders;
        for builder in sorted {
            // Program the data.
            let chip_erase = if !did_chip_erase { self.chip_erase } else { false };
            builder.program(chip_erase, true);
            did_chip_erase = true;
        }

        // Clear state to allow reuse.
        self.reset_state();
    }
}

#[test]
fn ranges_works() {
    let r = ranges([0, 1, 3, 5, 6, 7].iter().cloned());
    assert_eq!(
        r.collect::<Vec<(usize, usize)>>(),
        vec![
            (0, 1),
            (3, 3),
            (5, 7),
        ]
    );

    let r = ranges([3, 4, 7, 9, 11, 12].iter().cloned());
    assert_eq!(
        r.collect::<Vec<(usize, usize)>>(),
        vec![
            (3, 4),
            (7, 7),
            (9, 9),
            (11, 12),
        ]
    );

    let r = ranges([1, 3, 5, 7].iter().cloned());
    assert_eq!(
        r.collect::<Vec<(usize, usize)>>(),
        vec![
            (1, 1),
            (3, 3),
            (5, 5),
            (7, 7),
        ]
    );
}