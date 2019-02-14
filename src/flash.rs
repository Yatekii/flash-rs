// # Program to compute the CRC of sectors.  This works on cortex-m processors.
// # Code is relocatable and only needs to be on a 4 byte boundary.
// # 200 bytes of executable data below + 1024 byte crc table = 1224 bytes
// # Usage requirements:
// # -In memory reserve 0x600 for code & table
// # -Make sure data buffer is big enough to hold 4 bytes for each page that could be checked (ie.  >= num pages * 4)
// analyzer = (
//     0x2780b5f0, 0x25004684, 0x4e2b2401, 0x447e4a2b, 0x0023007f, 0x425b402b, 0x40130868, 0x08584043,
//     0x425b4023, 0x40584013, 0x40200843, 0x40104240, 0x08434058, 0x42404020, 0x40584010, 0x40200843,
//     0x40104240, 0x08434058, 0x42404020, 0x40584010, 0x40200843, 0x40104240, 0x08584043, 0x425b4023,
//     0x40434013, 0xc6083501, 0xd1d242bd, 0xd01f2900, 0x46602301, 0x469c25ff, 0x00894e11, 0x447e1841,
//     0x88034667, 0x409f8844, 0x2f00409c, 0x2201d012, 0x4252193f, 0x34017823, 0x402b4053, 0x599b009b,
//     0x405a0a12, 0xd1f542bc, 0xc00443d2, 0xd1e74281, 0xbdf02000, 0xe7f82200, 0x000000b2, 0xedb88320,
//     0x00000042, 
//     )


use crate::flash_algorithm::{
    FlashAlgorithm,
    FlashAlgorithmInstruction::*,
    FlashAlgorithmLocation::*,
};
use crate::target::Target;
use crate::memory_map::MemoryRegion;

#[derive(Debug)]
pub struct PageInfo {
    pub(crate) base_addr: u32, // Page start address
    pub(crate) size: u32, // Page size
    pub(crate) erase_weight: f32, // Time it takes to erase a page
    pub(crate) program_weight: f32, // Time it takes to program a page (Not including data transfer time)
}

impl PageInfo {
    pub fn new(base_addr: u32, size: u32, erase_weight: f32, program_weight: f32) -> Self {
        Self {
            base_addr,
            erase_weight,
            program_weight,
            size
        }
    }
}

pub struct FlashInfo {
    pub(crate) rom_start: u32,
    pub(crate) erase_weight: f32,
    pub(crate) crc_supported: bool,
}

impl FlashInfo {
    pub fn new(rom_start: u32, erase_weight: f32, crc_supported: bool) -> Self {
        Self {
            rom_start, // Starting address of ROM
            erase_weight, // Time it takes to perform a chip erase
            crc_supported // Is the function compute_crcs supported?
        }   
    }
}

/// Low-level control of flash programming algorithms.
/// 
/// Instances of this struct are bound to a flash memory region (FlashRegion) and support
/// programming only within that region's address range. To program images that cross flash
/// memory region boundaries, use the FlashLoader or FileProgrammer structs.
pub struct Flash {
    target: Target,
    pub(crate) region: MemoryRegion,
    flash_algorithm: FlashAlgorithm,
    pub is_erase_all_supported: bool,
    pub is_double_buffering_supported: bool,
    did_prepare_target: bool,
    active_operation: FlashOperation,
}

pub enum FlashError {
    Init(u32),
    Uninit(u32),
    EraseAll(u32),
    ErasePage(u32, u32), // (err_code, address)
    ProgramPage(u32, u32), // (err_code, address)
    WrongOperationOngoing(FlashOperation),
    EraseAllNotSupported,
}

pub enum FlashOperation {
    // Erase all or page erase.
    Erase = 1,
    // Program page or phrase.
    Program = 2,
    // Currently unused, but defined as part of the flash algorithm specification.
    Verify = 3,
    // Nothing ongoing.
    None,
}

impl Flash {
    const DEFAULT_PAGE_PROGRAM_WEIGHT: f32 = 0.130;
    const DEFAULT_PAGE_ERASE_WEIGHT: f32 = 0.048;
    const DEFAULT_CHIP_ERASE_WEIGHT: f32 = 0.174;

    pub fn new(target: Target, region: MemoryRegion, flash_algorithm: FlashAlgorithm) -> Self {
        // self.target = target
        // self.flash_algorithm = flash_algorithm
        // self.flash_algo_debug = False
        // self.active_operation = None
        // if flash_algorithm is not None:
        //     self.is_valid = True
        //     self.use_analyzer = flash_algorithm['analyzer_supported']
        //     self.end_flash_algo = flash_algorithm['load_address'] + len(flash_algorithm['instructions']) * 4
        //     self.begin_stack = flash_algorithm['begin_stack']
        //     self.begin_data = flash_algorithm['begin_data']
        //     self.static_base = flash_algorithm['static_base']
        //     self.min_program_length = flash_algorithm.get('min_program_length', 0)

        //     # Validate required APIs.
        //     assert self._is_api_valid('pc_erase_sector')
        //     assert self._is_api_valid('pc_program_page')

        //     # Check for double buffering support.
        //     if 'page_buffers' in flash_algorithm:
        //         self.page_buffers = flash_algorithm['page_buffers']
        //     else:
        //         self.page_buffers = [self.begin_data]

        //     self.double_buffer_supported = len(&self.page_buffers) > 1

        // else:
        //     self.is_valid = False
        //     self.use_analyzer = False
        //     self.end_flash_algo = None
        //     self.begin_stack = None
        //     self.begin_data = None
        //     self.static_base = None
        //     self.min_program_length = 0
        //     self.page_buffers = []
        //     self.double_buffer_supported = False
        Self {
            target,
            region,
            flash_algorithm,
            is_erase_all_supported: true,
            is_double_buffering_supported: false,
            did_prepare_target: false,
            active_operation: FlashOperation::None,
        }
    }
        
    // fn _is_api_valid(&self, api_name):
    //     return (api_name in self.flash_algorithm) \
    //             and (&self.flash_algorithm[api_name] >= self.flash_algorithm['load_address']) \
    //             and (&self.flash_algorithm[api_name] < self.end_flash_algo)

    /// Get info about the page that contains this address.
    ///
    /// Override this method if variable page sizes are supported.
    pub fn get_page_info(&self, address: u32) -> Option<PageInfo> {
        if !self.region.contains_address(address) {
            None
        } else {
            Some(PageInfo::new(address - (address % self.region.blocksize), self.region.blocksize, Self::DEFAULT_PAGE_ERASE_WEIGHT, Self::DEFAULT_PAGE_PROGRAM_WEIGHT))
        }
    }

    /// Get info about the flash.
    ///
    /// Override this method to return different values.
    pub fn get_flash_info(&self) -> FlashInfo {
        FlashInfo::new(self.region.start, Self::DEFAULT_CHIP_ERASE_WEIGHT, false) // self.use_analyzer (TODO:)
    }

    pub fn cleanup(&mut self) -> Result<(), FlashError> {
        self.uninit()?;
        self.did_prepare_target = false;
        Ok(())
    }

    pub fn uninit(&self) -> Result<(), FlashError> {
        match self.active_operation {
            FlashOperation::None => (),
            o => {
                // update core register to execute the uninit subroutine
                let result = self.call_function_and_wait(
                    self.flash_algorithm.get_instruction(PCUninit),
                    Some(o as u32),
                    None,
                    None,
                    None,
                    false
                );
                
                // check the return code
                if result != 0 { return Err(FlashError::Uninit(result)); }
            }
        }
        self.active_operation = FlashOperation::None;
        Ok(())
    }

    /// Prepare the flash algorithm for performing erase and program operations.
    pub fn init(&self, operation: FlashOperation) -> Result<(), FlashError> {
        let address = self.get_flash_info().rom_start;
        let clock = 0; // TODO: Maybe make this generic?
        
        self.target.halt();
        if !self.did_prepare_target {
            self.target.set_target_state("PROGRAM");
            // TODO: This was pass;
            // self.prepare_target();

            // Load flash algo code into target RAM.
            self.target.write_memory_block32(
                self.flash_algorithm.get_address(LoadAddress),
                self.flash_algorithm.get_instruction_list()
            );

            self.did_prepare_target = true;
        }

        // update core register to execute the init subroutine
        let result = self.call_function_and_wait(
            self.flash_algorithm.get_instruction(PCInit),
            Some(address),
            Some(clock),
            Some(operation as u32),
            None,
            true
        );

        // check the return code
        if result != 0 { return Err(FlashError::Init(result)); }
        
        self.active_operation = operation;
        Ok(())
    }

    /// Erase all the flash.
    pub fn erase_all(&self) -> Result<(), FlashError> {
        if let FlashOperation::Erase = self.active_operation {
            if self.is_erase_all_supported {
                // update core register to execute the erase_all subroutine
                let result = self.call_function_and_wait(
                    self.flash_algorithm.get_instruction(PCEraseAll),
                    None,
                    None,
                    None,
                    None,
                    true
                );

                // check the return code
                if result != 0 { return Err(FlashError::EraseAll(result)); }
                Ok(())
            } else {
                Err(FlashError::EraseAllNotSupported)
            }
        } else {
            Err(FlashError::WrongOperationOngoing(self.active_operation))
        }
    }

    /// Erase one page.
    pub fn erase_page(&self, address: u32) -> Result<(), FlashError> {
        if let FlashOperation::Erase = self.active_operation {
            // update core register to execute the erase_page subroutine
            let result = self.call_function_and_wait(
                self.flash_algorithm.get_instruction(PCEraseSector),
                Some(address),
                None,
                None,
                None,
                true
            );

            // check the return code
            if result != 0 { return Err(FlashError::ErasePage(result, address)); }
            Ok(())
        } else {
            Err(FlashError::WrongOperationOngoing(self.active_operation))
        }
    }

    /// Flash one or more pages.
    pub fn program_page(&self, address: u32, data: &[u8]) -> Result<(), FlashError> {
        if let FlashOperation::Program = self.active_operation {
            // prevent security settings from locking the device
            self.override_security_bits(address, data);

            // first transfer in RAM
            self.target.write_memory_block8(self.flash_algorithm.get_address(BeginData), data);

            // update core register to execute the program_page subroutine
            let result = self.call_function_and_wait(
                self.flash_algorithm.get_instruction(PCProgramPage),
                Some(address),
                Some(data.len() as u32),
                Some(self.flash_algorithm.get_address(BeginData)),
                None,
                true
            );

            // check the return code
            if result != 0 { return Err(FlashError::ProgramPage(result, address)); }
            Ok(())
        } else {
            Err(FlashError::WrongOperationOngoing(self.active_operation))
        }
    }

    fn call_function(
        &self,
        pc: u32,
        r0: Option<u32>,
        r1: Option<u32>,
        r2: Option<u32>,
        r3: Option<u32>,
        init: bool
    ) {
        let instruction_list = vec![];

        // if self.flash_algo_debug {
        //     // Save vector catch state for use in wait_for_completion()
        //     self.saved_vector_catch = self.target.get_vector_catch();
        //     self.target.set_vector_catch(Target.CATCH_ALL);
        // }

        instruction_list.push(("pc", pc));
        if let Some(r0) = r0 {
            instruction_list.push(("r0", r0));
        }
        if let Some(r1) = r1 {
            instruction_list.push(("r1", r1));
        }
        if let Some(r2) = r0 {
            instruction_list.push(("r2", r2));
        }
        if let Some(r3) = r3 {
            instruction_list.push(("r3", r3));
        }
        if init {
            instruction_list.push(("r9", self.flash_algorithm.get_address(StaticBase)));
            instruction_list.push(("sp", self.flash_algorithm.get_address(BeginStack)));
        }

        instruction_list.push(("lr", self.flash_algorithm.get_address(LoadAddress) + 1));
        self.target.write_core_registers_raw(instruction_list);

        // resume target
        self.target.resume();
    }

    // Wait until the breakpoint is hit.
    fn wait_for_completion(&self) -> u32 {
        while self.target.get_state() == Target.TARGET_RUNNING {};

        // if self.flash_algo_debug {
        //     regs = self.target.read_core_registers_raw(list(range(19)) + [20])
        //     println!("ALGO DBG: Registers after flash algo: [%s]", " ".join(regs.map(format!("{:08x}", r))));

        //     let expected_fp = self.flash_algorithm.get_address(StaticBase);
        //     let expected_sp = self.flash_algorithm.get_address(BeginStack);
        //     let expected_pc = self.flash_algorithm.get_address(LoadAddress);
        //     let expected_flash_algo = self.flash_algorithm.get_instructions();
        //     if self.use_analyzer:
        //         expected_analyzer = analyzer
        //     final_ipsr = self.target.read_core_register('ipsr')
        //     final_fp = self.target.read_core_register('r9')
        //     final_sp = self.target.read_core_register('sp')
        //     final_pc = self.target.read_core_register('pc')
        //     #TODO - uncomment if Read/write and zero init sections can be moved into a separate flash algo section
        //     #final_flash_algo = self.target.read_memory_block32(&self.flash_algorithm['load_address'], len(&self.flash_algorithm['instructions']))
        //     #if self.use_analyzer:
        //     #    final_analyzer = self.target.read_memory_block32(&self.flash_algorithm['analyzer_address'], len(analyzer))

        //     error = False
        //     if final_ipsr != 0:
        //         LOG.error("IPSR should be 0 but is 0x%x", final_ipsr)
        //         error = True
        //     if final_fp != expected_fp:
        //         # Frame pointer should not change
        //         LOG.error("Frame pointer should be 0x%x but is 0x%x" % (expected_fp, final_fp))
        //         error = True
        //     if final_sp != expected_sp:
        //         # Stack pointer should return to original value after function call
        //         LOG.error("Stack pointer should be 0x%x but is 0x%x" % (expected_sp, final_sp))
        //         error = True
        //     if final_pc != expected_pc:
        //         # PC should be pointing to breakpoint address
        //         LOG.error("PC should be 0x%x but is 0x%x" % (expected_pc, final_pc))
        //         error = True
        //     #TODO - uncomment if Read/write and zero init sections can be moved into a separate flash algo section
        //     #if not _same(expected_flash_algo, final_flash_algo):
        //     #    LOG.error("Flash algorithm overwritten!")
        //     #    error = True
        //     #if self.use_analyzer and not _same(expected_analyzer, final_analyzer):
        //     #    LOG.error("Analyzer overwritten!")
        //     #    error = True
        //     assert error == False
        //     self.target.set_vector_catch(&self._saved_vector_catch)
        // }

        self.target.read_core_register("r0")
    }

    fn call_function_and_wait(&self, pc: u32, r0: Option<u32>, r1: Option<u32>, r2: Option<u32>, r3: Option<u32>, init: bool) -> u32 {
        self.call_function(pc, r0, r1, r2, r3, init);
        self.wait_for_completion()
    }

    /// TODO: does this function have any use (maybe overridden by another class)
    fn override_security_bits(&self, address: u32, data: &[u8]) {
        // Returned data in the PyOCD version ...
    }
}
    
    // fn restore_target(&self):
    //     """! @brief Subclasses can override this method to undo any target configuration changes."""
    //     pass

    // fn compute_crcs(&self, sectors):
    //     assert self.use_analyzer
        
    //     data = []

    //     # Load analyzer code into target RAM.
    //     self.target.write_memory_block32(&self.flash_algorithm['analyzer_address'], analyzer)

    //     # Convert address, size pairs into commands
    //     # for the crc computation algorithm to preform
    //     for addr, size in sectors:
    //         size_val = msb(size)
    //         addr_val = addr // size
    //         # Size must be a power of 2
    //         assert (1 << size_val) == size
    //         # Address must be a multiple of size
    //         assert (addr % size) == 0
    //         val = (size_val << 0) | (addr_val << 16)
    //         data.append(val)

    //     self.target.write_memory_block32(&self.begin_data, data)

    //     # update core register to execute the subroutine
    //     result = self._call_function_and_wait(&self.flash_algorithm['analyzer_address'], self.begin_data, len(data))

    //     # Read back the CRCs for each section
    //     data = self.target.read_memory_block32(&self.begin_data, len(data))
    //     return data

    // fn start_program_page_with_buffer(&self, bufferNumber, flashPtr):
    //     """!
    //     @brief Start flashing one or more pages.
    //     """
    //     assert bufferNumber < len(&self.page_buffers), "Invalid buffer number"
    //     assert self.active_operation == self.Operation.PROGRAM

    //     # get info about this page
    //     page_info = self.get_page_info(flashPtr)

    //     # update core register to execute the program_page subroutine
    //     result = self._call_function(&self.flash_algorithm['pc_program_page'], flashPtr, page_info.size, self.page_buffers[bufferNumber])

    // fn load_page_buffer(&self, bufferNumber, flashPtr, bytes):
    //     """!
    //     @brief Load data to a numbered page buffer.
        
    //     This method is used in conjunction with start_program_page_with_buffer() to implement
    //     double buffered programming.
    //     """
    //     assert bufferNumber < len(&self.page_buffers), "Invalid buffer number"

    //     # prevent security settings from locking the device
    //     bytes = self.override_security_bits(flashPtr, bytes)

    //     # transfer the buffer to device RAM
    //     self.target.write_memory_block8(&self.page_buffers[bufferNumber], bytes)

    // fn program_phrase(&self, flashPtr, bytes):
    //     """!
    //     @brief Flash a portion of a page.
        
    //     @exception FlashFailure The address or data length is not aligned to the minimum
    //         programming length specified in the flash algorithm.
    //     """
    //     assert self.active_operation == self.Operation.PROGRAM

    //     # Get min programming length. If one was not specified, use the page size.
    //     if self.min_program_length:
    //         min_len = self.min_program_length
    //     else:
    //         min_len = self.get_page_info(flashPtr).size

    //     # Require write address and length to be aligned to min write size.
    //     if flashPtr % min_len:
    //         raise FlashFailure("unaligned flash write address")
    //     if len(bytes) % min_len:
    //         raise FlashFailure("phrase length is unaligned or too small")

    //     # prevent security settings from locking the device
    //     bytes = self.override_security_bits(flashPtr, bytes)

    //     # first transfer in RAM
    //     self.target.write_memory_block8(&self.begin_data, bytes)

    //     # update core register to execute the program_page subroutine
    //     result = self._call_function_and_wait(&self.flash_algorithm['pc_program_page'], flashPtr, len(bytes), self.begin_data)

    //     # check the return code
    //     if result != 0:
    //         LOG.error('program_phrase(0x%x) error: %i', flashPtr, result)

    // fn flash_block(&self, addr, data, smart_flash=True, chip_erase=None, progress_cb=None, fast_verify=False):
    //     """!
    //     @brief Flash a block of data.
    //     """
    //     assert self.region is not None
    //     assert self.region.contains_range(start=addr, length=len(data))
        
    //     fb = FlashBuilder(&self, self.region.start)
    //     fb.add_data(addr, data)
    //     info = fb.program(chip_erase, progress_cb, smart_flash, fast_verify)
    //     return info

    // fn set_flash_algo_debug(&self, enable):
    //     """!
    //     @brief Turn on extra flash algorithm checking

    //     When set this may slow down flash algo performance.
    //     """
    //     self.flash_algo_debug = enable
