#[derive(PartialEq, Eq, Hash)]
pub struct FlashAlgorithm {}

pub enum FlashAlgorithmInstruction {
    PCInit,
    PCUninit,
    PCProgramPage,
    PCEraseSector,
    PCEraseAll,
}

pub enum FlashAlgorithmLocation {
    LoadAddress,
    StaticBase,
    BeginStack,
    BeginData,
    PageSize,
}

impl FlashAlgorithm {
    /// TODO: Implement a Macro that actually creates FlashAlgorithm for different targets!
    pub fn new() -> Self {
        Self {}
    }

    pub fn get_instruction(&self, location: FlashAlgorithmInstruction) -> u32 {
        match location {
            LoadAddress => 0,
            PCInit => 0,
            PCUninit => 0,
            PCProgramPage => 0,
            PCEraseSector => 0,
            PCEraseAll => 0,
        }
    }

    pub fn get_address(&self, location: FlashAlgorithmLocation) -> u32 {
        match location {
            StaticBase => 0,
            BeginStack => 0,
            BeginData => 0,
            PageSize => 0,
        }
    }

    pub fn get_instruction_list(&self) -> Vec<u32> {
        vec![]
    }
}