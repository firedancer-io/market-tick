//! Binary ABI for the Market Tick program.
//!
//! This crate defines the complete V1 account layout, PDA seeds,
//! decoding errors, and program error numbers shared by on-chain code
//! and consumers. It does not depend on Anchor.
//!
//! # Account identification and trust
//!
//! Before decoding, consumers should:
//!
//! 1. derive the expected address from [`PDA_SEED_MARKET_TICK`] and
//!    [`PDA_SEED_VERSION_V1`];
//! 2. verify that the account key equals that address;
//! 3. verify that the account owner is the Market Tick program; and
//! 4. apply application-specific freshness, signer, timestamp, and
//!    interval policies after decoding [`MarketTickV1`].
//!
//! The on-chain protocol is permissionless. The first valid increment
//! in each slot selects that slot's signer, but the program does not
//! prove that the signer is the scheduled block producer or otherwise
//! trusted. Timestamps and target intervals are signer-supplied claims,
//! not authenticated wall-clock measurements.
//!
//! # Encoding
//!
//! V1 is an exact-length, 88-byte account beginning with
//! [`DISCRIMINATOR_V1`]. All multi-byte integers use little-endian byte
//! order.

#![cfg_attr(not(test), no_std)]
#![warn(missing_docs)]

/// Version 1 Market Tick account layout.
pub mod v1;

pub use v1::MarketTickV1;

/// Eight-byte discriminator at the beginning of every V1 account.
pub const DISCRIMINATOR_V1: [u8; 8] = *b"MRKTKV01";

/// Namespace seed used to derive Market Tick PDAs.
pub const PDA_SEED_MARKET_TICK: &[u8] = b"market_tick";

/// Version seed used to derive the V1 Market Tick PDA.
pub const PDA_SEED_VERSION_V1: &[u8] = b"v1";

/// Errors produced while decoding or encoding V1 account bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbiError {
    /// The supplied buffer length does not equal [`MarketTickV1::LEN`].
    InvalidAccountLength,
    /// The account does not begin with [`DISCRIMINATOR_V1`].
    BadDiscriminator,
}

/// Stable custom error numbers returned by the Market Tick program.
pub mod error_code {
    /// The instruction's slot does not equal the runtime clock slot.
    pub const SLOT_MISMATCH: u32 = 6000;
    /// A same-slot update was signed by someone other than that slot's first signer.
    pub const NOT_SLOT_SIGNER: u32 = 6001;
    /// A same-slot timestamp was not strictly greater than the preceding one.
    pub const NON_MONOTONIC: u32 = 6002;
    /// A same-slot update changed the target interval selected for that slot.
    pub const INTERVAL_MISMATCH: u32 = 6003;
    /// Incrementing the same-slot sequence would overflow `u64`.
    pub const COUNTER_OVERFLOW: u32 = 6004;
    /// The supplied timestamp was zero or negative.
    pub const INVALID_TIMESTAMP: u32 = 6005;
    /// The supplied target market-tick interval was zero.
    pub const INVALID_INTERVAL: u32 = 6006;
}

pub(crate) fn read_u64(data: &[u8], offset: usize) -> u64 {
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(bytes)
}

pub(crate) fn read_i64(data: &[u8], offset: usize) -> i64 {
    read_u64(data, offset) as i64
}

pub(crate) fn write_u64(data: &mut [u8], offset: usize, value: u64) {
    data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

pub(crate) fn write_i64(data: &mut [u8], offset: usize, value: i64) {
    write_u64(data, offset, value as u64);
}
