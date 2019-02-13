// from ..core.target import Target
// from ..core.exceptions import FlashFailure
// from ..utility.notification import Notification
// from ..utility.mask import same
// import logging
// from struct import unpack
// from time import time
// from binascii import crc32

// Number of bytes in a page to read to quickly determine if the page has the same data
use crate::common::same;

const PAGE_ESTIMATE_SIZE: u32 = 32;
const PAGE_READ_WEIGHT: f32 = 0.3;
const DATA_TRANSFER_B_PER_S: f32 = 40.0 * 1000.0; // ~40KB/s, depends on clock speed, theoretical limit for HID is 56,000 B/s

// class ProgrammingInfo(object):
//     def __init__(self):
//         self.program_type = None                # Type of programming performed - FLASH_PAGE_ERASE or FLASH_CHIP_ERASE
//         self.program_time = None                # Total programming time
//         self.analyze_type = None                # Type of flash analysis performed - FLASH_ANALYSIS_CRC32 or FLASH_ANALYSIS_PARTIAL_PAGE_READ
//         self.analyze_time = None                # Time to analyze flash contents
//         self.program_byte_count = 0
//         self.page_count = 0
//         self.same_page_count = 0

pub struct FlashPage {
    address: u32,
    size: u32,
    data: Vec<u8>,
    erase_weight: f32,
    program_weight: f32,
    pub erased: Option<bool>,
    pub same: Option<bool>,
}

impl FlashPage {
    pub fn new(address: u32, size: u32, data: Vec<u8>, erase_weight: f32, program_weight: f32) -> Self {
        Self {
            address,
            size,
            data,
            erase_weight,
            program_weight,
            erased: None,
            same: None,
        }
    }

    pub fn extend(&mut self, data: &[u8]) {
        self.data.extend(data);
    }

    /// Get time to verify a page.
    pub fn get_verify_weight(&self) -> f32 {
        self.size as f32 / DATA_TRANSFER_B_PER_S
    }

    /// Get time to program a page including the data transfer.
    fn get_program_weight(&self) -> f32 {
        self.program_weight + self.data.len() as f32 / DATA_TRANSFER_B_PER_S
    }
}

    // fn get_erase_program_weight(self):
    //     """
    //     Get time to erase and program a page including data transfer time
    //     """
    //     return self.erase_weight + self.program_weight + \
    //         float(len(self.data)) / float(DATA_TRANSFER_B_PER_S)

    

#[derive(Clone, Copy)]
struct FlashOperation<'a> {
    pub address: u32,
    pub data: &'a [u8],
}

impl<'a> FlashOperation<'a> {
    pub fn new(address: u32, data: &'a [u8]) -> Self {
        Self {
            address,
            data,
        }
    }
}

pub struct FlashBuilder<'a> {
    pub(crate) flash_start: u32,
    flash_operations: Vec<FlashOperation<'a>>,
    buffered_data_size: u32,
    flash: Flash,
    page_list: Vec<FlashPage>,
    enable_double_buffering: bool,
}

pub enum FlashBuilderError {
    AddressBeforeFlashStart(u32), // Contains faulty address.
    DataOverlap(u32), // Contains faulty address.
    InvalidFlashAddress(u32), // Contains faulty address.
}

impl<'a> FlashBuilder<'a> {

    // TODO: Needed when we do advanced flash analysis.
    // // Type of flash analysis
    // FLASH_ANALYSIS_CRC32 = "CRC32"
    // FLASH_ANALYSIS_PARTIAL_PAGE_READ = "PAGE_READ"

    fn new(flash: Flash, base_addr: u32) -> Self {
        Self {
            flash,
            flash_start: base_addr,
            flash_operations: vec![],
            buffered_data_size: 0,
            page_list: vec![],
            enable_double_buffering: false,
        }
    }

    /// Add a block of data to be programmed
    ///
    /// Note - programming does not start until the method
    /// program is called.
    pub fn add_data(&mut self, address: u32, data: &'a [u8]) -> Result<(), FlashBuilderError> {
        // Sanity check
        if address >= self.flash_start {
            // Add operation to sorted list
            match self.flash_operations.binary_search_by_key(&address, |&v| v.address) {
                Ok(_) => { /* TODO: Return error as this should never happen (we would have double data for an address) */ },
                Err(position) => self.flash_operations.insert(position, FlashOperation::new(address, data))
            }
            self.buffered_data_size += data.len() as u32;

            let mut previous_operation: Option<FlashOperation> = None;
            for operation in self.flash_operations {
                if let Some(previous) = previous_operation {
                    if previous.address + previous.data.len() as u32 > operation.address {
                        return Err(FlashBuilderError::DataOverlap(operation.address));
                    }
                }
                previous_operation = Some(operation);
            }
            Ok(())
        } else {
            Err(FlashBuilderError::AddressBeforeFlashStart(address))
        }
    }

    /// Determine fastest method of flashing and then run flash programming.
    ///
    /// Data must have already been added with add_data
    /// TODO: Not sure if this works as intended ...
    pub fn program(self, chip_erase: bool, smart_flash: bool) -> Result<(), FlashBuilderError> {
        // Assumptions
        // 1. Page erases must be on page boundaries ( page_erase_addr % page_size == 0 )
        // 2. Page erase can have a different size depending on location
        // 3. It is safe to program a page with less than a page of data

        // Examples
        // - lpc4330     - Non 0 base address
        // - nRF51       - UICR location far from flash (address 0x10001000)
        // - LPC1768     - Different sized pages

        // Convert the list of flash operations into flash pages
        let mut program_byte_count = 0;
        let mut flash_address = self.flash_operations[0].address;
        let mut info = self.flash.get_page_info(flash_address).ok_or_else(|| Err(FlashBuilderError::InvalidFlashAddress(flash_address)))?;
        let mut page_address = flash_address - (flash_address % info.size);
        let mut current_page = FlashPage::new(page_address, info.size, vec![], info.erase_weight, info.program_weight);
        self.page_list.push(current_page);
        for flash_operation in self.flash_operations {
            let mut pos = 0;
            while pos < flash_operation.data.len() {
                // Check if operation is in next page
                flash_address = flash_operation.address + pos as u32;
                if flash_address >= current_page.address + current_page.size {
                    info = self.flash.get_page_info(flash_address).ok_or_else(|| Err(FlashBuilderError::InvalidFlashAddress(flash_address)))?;
                    page_address = flash_address - (flash_address % info.size);
                    current_page = FlashPage::new(page_address, info.size, vec![], info.erase_weight, info.program_weight);
                    self.page_list.push(current_page);
                }

                // Fill the page gap if there is one
                // TODO: WTF?
                // let page_data_end = current_page.address + current_page.data.len() as u32;
                // if flash_address != page_data_end {
                //     let old_data = self.flash.target.read_memory_block8(page_data_end, flash_address - page_data_end);
                //     current_page.data.extend(old_data);
                // }

                // Copy data to page and increment pos
                let space_left_in_page = info.size - current_page.data.len();
                let space_left_in_data = flash_operation.data.len() - pos;
                let amount = usize::min(space_left_in_page, space_left_in_data);
                current_page.extend(&flash_operation.data[pos..pos + amount]);
                program_byte_count += amount;

                // increment position
                pos += amount;
            }
        }

        // If smart flash was set to false then mark all pages
        // as requiring programming
        if !smart_flash {
            self.mark_all_pages_for_programming();
        }
        
        // If the flash algo doesn't support erase all, disable chip erase.
        if !self.flash.is_erase_all_supported {
            chip_erase = false;
        }

        let (chip_erase_count, chip_erase_program_time) = self.compute_chip_erase_pages_and_weight();
        let page_erase_min_program_time = self.compute_page_erase_pages_weight_min();

        // If chip_erase hasn't been specified determine if chip erase is faster
        // than page erase regardless of contents
        if !chip_erase && (chip_erase_program_time < page_erase_min_program_time) {
            chip_erase = true;
        }

        // TODO:
        // If chip erase isn't True then analyze the flash
        // if !chip_erase {
        //     analyze_start = time()
        //     if self.flash.get_flash_info().crc_supported {
        //         sector_erase_count, page_program_time = self._compute_page_erase_pages_and_weight_crc32(fast_verify)
        //         self.perf.analyze_type = FlashBuilder.FLASH_ANALYSIS_CRC32
        //     else {
        //         sector_erase_count, page_program_time = self._compute_page_erase_pages_and_weight_sector_read()
        //         self.perf.analyze_type = FlashBuilder.FLASH_ANALYSIS_PARTIAL_PAGE_READ
        //     analyze_finish = time()
        //     self.perf.analyze_time = analyze_finish - analyze_start
        //     LOG.debug("Analyze time { %f" % (analyze_finish - analyze_start))
        // }

        // If chip erase hasn't been set then determine fastest method to program
        // if !chip_erase {
        //     chip_erase = chip_erase_program_time < page_program_time;
        // }

        if chip_erase {
            if self.flash.is_double_buffering_supported && self.enable_double_buffering {
                // TODO: Implement double buffering (for now it's disabled so not erasing here is ok as this if never triggers)
                // self._chip_erase_program_double_buffer()
            } else {
                self.chip_erase_program();
            }
        }
        else {
            if self.flash.is_double_buffering_supported && self.enable_double_buffering {
                // TODO: Implement double buffering (for now it's disabled so not erasing here is ok as this if never triggers)
                // self._page_erase_program_double_buffer()
            } else {
                self.page_erase_program();
            }
        };

        // Cleanup flash algo and reset target after programming.
        self.flash.cleanup();
        // TODO: Reset target at a different location.
        // self.flash.target.reset_stop_on_reset();

        Ok(())
    }

    fn mark_all_pages_for_programming(&mut self) {
        for page in self.page_list {
            page.erased = None;
            page.same = None;
        }
    }

    /// Compute the number of erased pages.
    ///
    /// Determine how many pages in the new data are already erased.
    fn compute_chip_erase_pages_and_weight(&self) -> (u32, f32) {
        let mut chip_erase_count: u32 = 0;
        let mut chip_erase_weight: f32 = self.flash.get_flash_info().erase_weight;
        for page in self.page_list {
            if let Some(erased) = page.erased {
                if !erased {
                    chip_erase_count += 1;
                    chip_erase_weight += page.get_program_weight();
                    page.erased = self.flash.region.is_erased(page.data)
                }
            } else {
                page.erased = self.flash.region.is_erased(page.data)
            }
        }
        (chip_erase_count, chip_erase_weight)
    }

    fn compute_page_erase_pages_weight_min(&self) -> f32 {
        let mut page_erase_min_weight = 0.0;
        for page in self.page_list {
            page_erase_min_weight += page.get_verify_weight();
        }
        return page_erase_min_weight
    }

    /// Program by first performing a chip erase.
    fn chip_erase_program(&mut self) {
        self.flash.init(self.flash.Operation.ERASE);
        self.flash.erase_all();
        self.flash.uninit();
        
        self.flash.init(self.flash.Operation.PROGRAM);
        for page in self.page_list {
            if let Some(erased) = page.erased {
                if !erased {
                    self.flash.program_page(page.address, page.data);
                }
            }
        }
        self.flash.uninit();
    }

    /// Program by performing sector erases.
    fn page_erase_program(&self) {
        for page in self.page_list {
            // Read page data if unknown - after this page.same will be True or False
            if let Some(same) = page.same {
                // Program page if not the same
                if !same {
                    self.flash.init(self.flash.Operation.ERASE);
                    self.flash.erase_page(page.address);
                    self.flash.uninit();

                    self.flash.init(self.flash.Operation.PROGRAM);
                    self.flash.program_page(page.address, page.data);
                    self.flash.uninit();
                }
            } else {
                let data = self.flash.target.read_memory_block8(page.address, page.data.len());
                page.same = Some(same(page.data.as_slice(), data));
            }
        }
    }
}

    // def _compute_page_erase_pages_and_weight_sector_read(self):
    //     """
    //     Estimate how many pages are the same.

    //     Quickly estimate how many pages are the same.  These estimates are used
    //     by page_erase_program so it is recommended to call this before beginning programming
    //     This is done automatically by smart_program.
    //     """
    //     # Quickly estimate how many pages are the same
    //     page_erase_count = 0
    //     page_erase_weight = 0
    //     for page in self.page_list:
    //         # Analyze pages that haven't been analyzed yet
    //         if page.same is None:
    //             size = min(PAGE_ESTIMATE_SIZE, len(page.data))
    //             data = self.flash.target.read_memory_block8(page.address, size)
    //             page_same = same(data, page.data[0:size])
    //             if page_same is False:
    //                 page.same = False

    //     # Put together page and time estimate
    //     for page in self.page_list:
    //         if page.same is False:
    //             page_erase_count += 1
    //             page_erase_weight += page.get_erase_program_weight()
    //         elif page.same is None:
    //             # Page is probably the same but must be read to confirm
    //             page_erase_weight += page.get_verify_weight()
    //         elif page.same is True:
    //             # Page is confirmed to be the same so no programming weight
    //             pass

    //     self.page_erase_count = page_erase_count
    //     self.page_erase_weight = page_erase_weight
    //     return page_erase_count, page_erase_weight

    // def _compute_page_erase_pages_and_weight_crc32(self, assume_estimate_correct=False):
    //     """
    //     Estimate how many pages are the same.

    //     Quickly estimate how many pages are the same.  These estimates are used
    //     by page_erase_program so it is recommended to call this before beginning programming
    //     This is done automatically by smart_program.

    //     If assume_estimate_correct is set to True, then pages with matching CRCs
    //     will be marked as the same.  There is a small chance that the CRCs match even though the
    //     data is different, but the odds of this happing are low: ~1/(2^32) = ~2.33*10^-8%.
    //     """
    //     # Build list of all the pages that need to be analyzed
    //     sector_list = []
    //     page_list = []
    //     for page in self.page_list:
    //         if page.same is None:
    //             # Add sector to compute_crcs
    //             sector_list.append((page.address, page.size))
    //             page_list.append(page)
    //             # Compute CRC of data (Padded with 0xFF)
    //             data = list(page.data)
    //             pad_size = page.size - len(page.data)
    //             if pad_size > 0:
    //                 data.extend([0xFF] * pad_size)
    //             page.crc = crc32(bytearray(data)) & 0xFFFFFFFF

    //     # Analyze pages
    //     page_erase_count = 0
    //     page_erase_weight = 0
    //     if len(page_list) > 0:
    //         self.flash.init(self.flash.Operation.PROGRAM)
    //         crc_list = self.flash.compute_crcs(sector_list)
    //         for page, crc in zip(page_list, crc_list):
    //             page_same = page.crc == crc
    //             if assume_estimate_correct:
    //                 page.same = page_same
    //             elif page_same is False:
    //                 page.same = False
    //         self.flash.uninit()

    //     # Put together page and time estimate
    //     for page in self.page_list:
    //         if page.same is False:
    //             page_erase_count += 1
    //             page_erase_weight += page.get_erase_program_weight()
    //         elif page.same is None:
    //             # Page is probably the same but must be read to confirm
    //             page_erase_weight += page.get_verify_weight()
    //         elif page.same is True:
    //             # Page is confirmed to be the same so no programming weight
    //             pass

    //     self.page_erase_count = page_erase_count
    //     self.page_erase_weight = page_erase_weight
    //     return page_erase_count, page_erase_weight

    // def _next_unerased_page(self, i):
    //     if i >= len(self.page_list):
    //         return None, i
    //     page = self.page_list[i]
    //     while page.erased:
    //         i += 1
    //         if i >= len(self.page_list):
    //             return None, i
    //         page = self.page_list[i]
    //     return page, i + 1

    // def _chip_erase_program_double_buffer(self, progress_cb=_stub_progress):
    //     """
    //     Program by first performing a chip erase.
    //     """
    //     LOG.debug("Smart chip erase")
    //     LOG.debug("%i of %i pages already erased", len(self.page_list) - self.chip_erase_count, len(self.page_list))
    //     progress_cb(0.0)
    //     progress = 0

    //     self.flash.init(self.flash.Operation.ERASE)
    //     self.flash.erase_all()
    //     self.flash.uninit()
        
    //     progress += self.flash.get_flash_info().erase_weight

    //     # Set up page and buffer info.
    //     error_count = 0
    //     current_buf = 0
    //     next_buf = 1
    //     page, i = self._next_unerased_page(0)
    //     assert page is not None

    //     # Load first page buffer
    //     self.flash.load_page_buffer(current_buf, page.address, page.data)

    //     self.flash.init(self.flash.Operation.PROGRAM)
    //     while page is not None:
    //         # Kick off this page program.
    //         current_addr = page.address
    //         current_weight = page.get_program_weight()
    //         self.flash.start_program_page_with_buffer(current_buf, current_addr)

    //         # Get next page and load it.
    //         page, i = self._next_unerased_page(i)
    //         if page is not None:
    //             self.flash.load_page_buffer(next_buf, page.address, page.data)

    //         # Wait for the program to complete.
    //         result = self.flash.wait_for_completion()

    //         # check the return code
    //         if result != 0:
    //             LOG.error('program_page(0x%x) error: %i', current_addr, result)
    //             error_count += 1
    //             if error_count > self.max_errors:
    //                 LOG.error("Too many page programming errors, aborting program operation")
    //                 break

    //         # Swap buffers.
    //         temp = current_buf
    //         current_buf = next_buf
    //         next_buf = temp

    //         # Update progress.
    //         progress += current_weight
    //         progress_cb(float(progress) / float(self.chip_erase_weight))
        
    //     self.flash.uninit()
    //     progress_cb(1.0)
    //     return FlashBuilder.FLASH_CHIP_ERASE

    // def _scan_pages_for_same(self, progress_cb=_stub_progress):
    //     """
    //     Program by performing sector erases.
    //     """
    //     progress = 0
    //     count = 0
    //     same_count = 0

    //     for page in self.page_list:
    //         # Read page data if unknown - after this page.same will be True or False
    //         if page.same is None:
    //             data = self.flash.target.read_memory_block8(page.address, len(page.data))
    //             page.same = same(page.data, data)
    //             progress += page.get_verify_weight()
    //             count += 1
    //             if page.same:
    //                 same_count += 1

    //             # Update progress
    //             progress_cb(float(progress) / float(self.page_erase_weight))
    //     return progress

    // def _next_nonsame_page(self, i):
    //     if i >= len(self.page_list):
    //         return None, i
    //     page = self.page_list[i]
    //     while page.same:
    //         i += 1
    //         if i >= len(self.page_list):
    //             return None, i
    //         page = self.page_list[i]
    //     return page, i + 1

    // def _page_erase_program_double_buffer(self, progress_cb=_stub_progress):
    //     """
    //     Program by performing sector erases.
    //     """
    //     actual_page_erase_count = 0
    //     actual_page_erase_weight = 0
    //     progress = 0

    //     progress_cb(0.0)

    //     # Fill in same flag for all pages. This is done up front so we're not trying
    //     # to read from flash while simultaneously programming it.
    //     progress = self._scan_pages_for_same(progress_cb)

    //     # Set up page and buffer info.
    //     error_count = 0
    //     current_buf = 0
    //     next_buf = 1
    //     page, i = self._next_nonsame_page(0)

    //     # Make sure there are actually pages to program differently from current flash contents.
    //     if page is not None:
    //         # Load first page buffer
    //         self.flash.load_page_buffer(current_buf, page.address, page.data)

    //         while page is not None:
    //             assert page.same is not None

    //             # Kick off this page program.
    //             current_addr = page.address
    //             current_weight = page.get_erase_program_weight()

    //             self.flash.init(self.flash.Operation.ERASE)
    //             self.flash.erase_page(current_addr)
    //             self.flash.uninit()

    //             self.flash.init(self.flash.Operation.PROGRAM)
    //             self.flash.start_program_page_with_buffer(current_buf, current_addr)
                
    //             actual_page_erase_count += 1
    //             actual_page_erase_weight += page.get_erase_program_weight()

    //             # Get next page and load it.
    //             page, i = self._next_nonsame_page(i)
    //             if page is not None:
    //                 self.flash.load_page_buffer(next_buf, page.address, page.data)

    //             # Wait for the program to complete.
    //             result = self.flash.wait_for_completion()

    //             # check the return code
    //             if result != 0:
    //                 LOG.error('program_page(0x%x) error: %i', current_addr, result)
    //                 error_count += 1
    //                 if error_count > self.max_errors:
    //                     LOG.error("Too many page programming errors, aborting program operation")
    //                     break
                
    //             self.flash.uninit()
                
    //             # Swap buffers.
    //             temp = current_buf
    //             current_buf = next_buf
    //             next_buf = temp

    //             # Update progress
    //             progress += current_weight
    //             if self.page_erase_weight > 0:
    //                 progress_cb(float(progress) / float(self.page_erase_weight))

    //     progress_cb(1.0)

    //     LOG.debug("Estimated page erase count: %i", self.page_erase_count)
    //     LOG.debug("Actual page erase count: %i", actual_page_erase_count)

    //     return FlashBuilder.FLASH_PAGE_ERASE
