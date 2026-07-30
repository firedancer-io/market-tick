use anchor_lang::prelude::Pubkey;
use market_tick_abi::MarketTickV1;

use crate::error::MarketTickError;

pub(super) fn increment_v1(
    account: &mut MarketTickV1,
    signer: Pubkey,
    current_slot: u64,
    slot: u64,
    timestamp_ns: i64,
    target_market_tick_interval_ns: u64,
) -> anchor_lang::Result<()> {
    if slot != current_slot {
        return Err(MarketTickError::SlotMismatch.into());
    }
    if timestamp_ns <= 0 {
        return Err(MarketTickError::InvalidTimestamp.into());
    }
    if target_market_tick_interval_ns == 0 {
        return Err(MarketTickError::InvalidInterval.into());
    }

    let signer = signer.to_bytes();
    let first_increment = account.signer == [0; 32] || account.slot != slot;
    if first_increment {
        account.signer = signer;
        account.slot = slot;
        account.first_timestamp_ns = timestamp_ns;
        account.timestamp_ns = timestamp_ns;
        account.sequence = 0;
        account.target_market_tick_interval_ns = target_market_tick_interval_ns;
        account.observed_market_tick_interval_ns = 0;
    } else {
        if account.signer != signer {
            return Err(MarketTickError::NotSlotSigner.into());
        }
        if account.target_market_tick_interval_ns != target_market_tick_interval_ns {
            return Err(MarketTickError::IntervalMismatch.into());
        }
        if timestamp_ns <= account.timestamp_ns {
            return Err(MarketTickError::NonMonotonic.into());
        }
        account.sequence = account
            .sequence
            .checked_add(1)
            .ok_or(MarketTickError::CounterOverflow)?;
        account.observed_market_tick_interval_ns = (timestamp_ns - account.timestamp_ns) as u64;
        account.timestamp_ns = timestamp_ns;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SLOT: u64 = 42;
    const TIMESTAMP: i64 = 1_000;
    const INTERVAL: u64 = 10;

    fn signer(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn increment(
        account: &mut MarketTickV1,
        signer: Pubkey,
        current_slot: u64,
        slot: u64,
        timestamp_ns: i64,
        interval_ns: u64,
    ) -> anchor_lang::Result<()> {
        increment_v1(
            account,
            signer,
            current_slot,
            slot,
            timestamp_ns,
            interval_ns,
        )
    }

    #[test]
    fn first_increment_initializes_slot_state() {
        let mut account = MarketTickV1::default();
        let slot_signer = signer(1);

        increment(&mut account, slot_signer, SLOT, SLOT, TIMESTAMP, INTERVAL).unwrap();

        assert_eq!(account.signer, slot_signer.to_bytes());
        assert_eq!(account.slot, SLOT);
        assert_eq!(account.first_timestamp_ns, TIMESTAMP);
        assert_eq!(account.timestamp_ns, TIMESTAMP);
        assert_eq!(account.sequence, 0);
        assert_eq!(account.target_market_tick_interval_ns, INTERVAL);
        assert_eq!(account.observed_market_tick_interval_ns, 0);
    }

    #[test]
    fn new_slot_rotates_signer_and_resets_state() {
        let mut account = MarketTickV1::default();
        increment(&mut account, signer(1), SLOT, SLOT, TIMESTAMP, INTERVAL).unwrap();

        let next_slot = SLOT + 1;
        let next_signer = signer(2);
        let next_timestamp = TIMESTAMP + 500;
        let next_interval = INTERVAL + 5;
        increment(
            &mut account,
            next_signer,
            next_slot,
            next_slot,
            next_timestamp,
            next_interval,
        )
        .unwrap();

        assert_eq!(account.signer, next_signer.to_bytes());
        assert_eq!(account.slot, next_slot);
        assert_eq!(account.first_timestamp_ns, next_timestamp);
        assert_eq!(account.timestamp_ns, next_timestamp);
        assert_eq!(account.sequence, 0);
        assert_eq!(account.target_market_tick_interval_ns, next_interval);
        assert_eq!(account.observed_market_tick_interval_ns, 0);
    }

    #[test]
    fn same_slot_increment_requires_a_strictly_increasing_timestamp() {
        let mut account = MarketTickV1::default();
        let slot_signer = signer(1);
        increment(&mut account, slot_signer, SLOT, SLOT, TIMESTAMP, INTERVAL).unwrap();
        let initialized = account;

        assert!(increment(&mut account, slot_signer, SLOT, SLOT, TIMESTAMP, INTERVAL,).is_err());
        assert_eq!(account, initialized);

        assert!(increment(
            &mut account,
            slot_signer,
            SLOT,
            SLOT,
            TIMESTAMP - 1,
            INTERVAL,
        )
        .is_err());
        assert_eq!(account, initialized);

        increment(
            &mut account,
            slot_signer,
            SLOT,
            SLOT,
            TIMESTAMP + 1,
            INTERVAL,
        )
        .unwrap();
        assert_eq!(account.timestamp_ns, TIMESTAMP + 1);
        assert_eq!(account.sequence, 1);
        assert_eq!(account.observed_market_tick_interval_ns, 1);
    }

    #[test]
    fn sequence_overflow_is_rejected_without_mutating_state() {
        let slot_signer = signer(1);
        let mut account = MarketTickV1 {
            signer: slot_signer.to_bytes(),
            slot: SLOT,
            first_timestamp_ns: TIMESTAMP,
            timestamp_ns: TIMESTAMP,
            sequence: u64::MAX,
            target_market_tick_interval_ns: INTERVAL,
            observed_market_tick_interval_ns: 0,
        };
        let before = account;

        assert!(increment(
            &mut account,
            slot_signer,
            SLOT,
            SLOT,
            TIMESTAMP + 1,
            INTERVAL,
        )
        .is_err());
        assert_eq!(account, before);
    }

    #[test]
    fn instruction_slot_must_match_current_slot() {
        let mut account = MarketTickV1::default();

        assert!(increment(&mut account, signer(1), SLOT, SLOT - 1, TIMESTAMP, INTERVAL,).is_err());
        assert_eq!(account, MarketTickV1::default());
    }
}
