use anchor_lang::prelude::*;

/// Errors returned by Market Tick state transitions.
#[error_code]
pub enum MarketTickError {
    #[msg("Instruction slot does not match the runtime clock slot")]
    SlotMismatch,
    #[msg("Signer is not the signer selected for this slot")]
    NotSlotSigner,
    #[msg("Timestamp must strictly increase within a slot")]
    NonMonotonic,
    #[msg("Target interval cannot change within a slot")]
    IntervalMismatch,
    #[msg("Sequence counter overflow")]
    CounterOverflow,
    #[msg("Timestamp must be positive")]
    InvalidTimestamp,
    #[msg("Target interval must be nonzero")]
    InvalidInterval,
}

#[cfg(test)]
mod tests {
    use super::*;
    use market_tick_abi::error_code;

    #[test]
    fn anchor_error_numbers_match_the_public_abi() {
        assert_eq!(
            u32::from(MarketTickError::SlotMismatch),
            error_code::SLOT_MISMATCH
        );
        assert_eq!(
            u32::from(MarketTickError::NotSlotSigner),
            error_code::NOT_SLOT_SIGNER
        );
        assert_eq!(
            u32::from(MarketTickError::NonMonotonic),
            error_code::NON_MONOTONIC
        );
        assert_eq!(
            u32::from(MarketTickError::IntervalMismatch),
            error_code::INTERVAL_MISMATCH
        );
        assert_eq!(
            u32::from(MarketTickError::CounterOverflow),
            error_code::COUNTER_OVERFLOW
        );
        assert_eq!(
            u32::from(MarketTickError::InvalidTimestamp),
            error_code::INVALID_TIMESTAMP
        );
        assert_eq!(
            u32::from(MarketTickError::InvalidInterval),
            error_code::INVALID_INTERVAL
        );
    }
}
