use crate::{read_i64, read_u64, write_i64, write_u64, AbiError, DISCRIMINATOR_V1};

/// Decoded state of a complete V1 Market Tick account.
///
/// A newly initialized account contains [`DISCRIMINATOR_V1`] followed
/// by zero in every state field. The default signer means no tick has
/// been accepted yet. The first accepted increment sets all fields and
/// selects the signer for that slot.
///
/// The first writer in a slot is permissionless. The program does not
/// prove that [`signer`](Self::signer) is a block producer, and
/// timestamp fields are signer-supplied claims rather than
/// authenticated wall-clock measurements. Consumers must apply their
/// own signer, freshness, and plausibility policy.
///
/// All fields are encoded explicitly; Rust's in-memory struct layout is
/// not part of the ABI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MarketTickV1 {
    /// Signer selected by the first accepted increment in `slot`, at
    /// bytes `8..40`.
    ///
    /// This is all zeroes before the first accepted tick.
    pub signer: [u8; 32],
    /// Solana runtime slot associated with the current state, at bytes
    /// `40..48` as a little-endian `u64`.
    pub slot: u64,
    /// First signer-supplied timestamp accepted in `slot`, at bytes
    /// `48..56` as a little-endian `i64`.
    ///
    /// The value must be positive when written by the program. It is a
    /// POSIX nanosecond timestamp by protocol convention, but its
    /// wall-clock accuracy is not authenticated on-chain.
    pub first_timestamp_ns: i64,
    /// Most recent signer-supplied timestamp in `slot`, at bytes
    /// `56..64` as a little-endian `i64`.
    ///
    /// It equals [`first_timestamp_ns`](Self::first_timestamp_ns) when
    /// `sequence == 0` and must strictly increase on later same-slot
    /// updates.
    pub timestamp_ns: i64,
    /// Zero-based number of accepted updates after the first update in
    /// `slot`, at bytes `64..72` as a little-endian `u64`.
    ///
    /// The first accepted update has sequence zero. A newly initialized
    /// account also contains zero, so this field alone does not
    /// indicate that a tick exists.
    pub sequence: u64,
    /// Signer-supplied target interval in nanoseconds, at bytes
    /// `72..80` as a little-endian `u64`.
    ///
    /// It must be nonzero and remain unchanged within a slot. The
    /// program does not enforce that observed timestamp differences
    /// match this target.
    pub target_market_tick_interval_ns: u64,
    /// Difference between the two most recently accepted timestamps in
    /// `slot`, at bytes `80..88` as a little-endian `u64`.
    ///
    /// This is zero before any tick and before a second update in a
    /// slot establishes an observed interval.
    pub observed_market_tick_interval_ns: u64,
}

impl MarketTickV1 {
    /// Exact encoded V1 account length in bytes.
    pub const LEN: usize = 88;

    const OFF_SIGNER: usize = DISCRIMINATOR_V1.len();
    const OFF_SLOT: usize = Self::OFF_SIGNER + 32;
    const OFF_FIRST_TIMESTAMP: usize = Self::OFF_SLOT + 8;
    const OFF_TIMESTAMP: usize = Self::OFF_FIRST_TIMESTAMP + 8;
    const OFF_SEQUENCE: usize = Self::OFF_TIMESTAMP + 8;
    const OFF_TARGET_INTERVAL: usize = Self::OFF_SEQUENCE + 8;
    const OFF_OBSERVED_INTERVAL: usize = Self::OFF_TARGET_INTERVAL + 8;

    /// Decodes and validates a complete V1 account.
    ///
    /// This method requires exactly [`Self::LEN`] bytes and verifies
    /// [`DISCRIMINATOR_V1`]. It does not validate the account's
    /// address, owner, signer trust, freshness, or semantic field
    /// ranges.
    pub fn decode(data: &[u8]) -> Result<Self, AbiError> {
        if data.len() != Self::LEN {
            return Err(AbiError::InvalidAccountLength);
        }
        if data[..DISCRIMINATOR_V1.len()] != DISCRIMINATOR_V1 {
            return Err(AbiError::BadDiscriminator);
        }

        let mut signer = [0u8; 32];
        signer.copy_from_slice(&data[Self::OFF_SIGNER..Self::OFF_SIGNER + 32]);
        Ok(Self {
            signer,
            slot: read_u64(data, Self::OFF_SLOT),
            first_timestamp_ns: read_i64(data, Self::OFF_FIRST_TIMESTAMP),
            timestamp_ns: read_i64(data, Self::OFF_TIMESTAMP),
            sequence: read_u64(data, Self::OFF_SEQUENCE),
            target_market_tick_interval_ns: read_u64(data, Self::OFF_TARGET_INTERVAL),
            observed_market_tick_interval_ns: read_u64(data, Self::OFF_OBSERVED_INTERVAL),
        })
    }

    /// Encodes this value as a complete V1 account.
    ///
    /// This method requires exactly [`Self::LEN`] bytes, always writes
    /// [`DISCRIMINATOR_V1`], and does not enforce the program's state
    /// transition invariants.
    pub fn encode(&self, data: &mut [u8]) -> Result<(), AbiError> {
        if data.len() != Self::LEN {
            return Err(AbiError::InvalidAccountLength);
        }

        data[..DISCRIMINATOR_V1.len()].copy_from_slice(&DISCRIMINATOR_V1);
        data[Self::OFF_SIGNER..Self::OFF_SIGNER + 32].copy_from_slice(&self.signer);
        write_u64(data, Self::OFF_SLOT, self.slot);
        write_i64(data, Self::OFF_FIRST_TIMESTAMP, self.first_timestamp_ns);
        write_i64(data, Self::OFF_TIMESTAMP, self.timestamp_ns);
        write_u64(data, Self::OFF_SEQUENCE, self.sequence);
        write_u64(
            data,
            Self::OFF_TARGET_INTERVAL,
            self.target_market_tick_interval_ns,
        );
        write_u64(
            data,
            Self::OFF_OBSERVED_INTERVAL,
            self.observed_market_tick_interval_ns,
        );
        Ok(())
    }
}
