use pinocchio::error::ProgramError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum MarketTickError {
    InvalidAccountData = 0,
    AlreadyInitialized = 1,
    SlotMismatch = 2,
    NotSlotSigner = 3,
    NonMonotonic = 4,
    IntervalMismatch = 5,
    CounterOverflow = 6,
    InvalidTimestamp = 7,
    InvalidInterval = 8,
}

impl From<MarketTickError> for ProgramError {
    #[inline(always)]
    fn from(e: MarketTickError) -> Self {
        ProgramError::Custom(e as u32)
    }
}
