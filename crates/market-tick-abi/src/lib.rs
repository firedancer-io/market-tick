//! Versioned binary ABI for the Market Tick program.

#![cfg_attr(not(test), no_std)]

pub mod header;
pub mod v1;

pub use header::{Header, CURRENT_VERSION, DISCRIMINATOR};
pub use v1::MarketTickV1;

use solana_pubkey::Pubkey;

pub const PDA_SEEDS_V1: [&[u8]; 2] = [b"market_tick", b"v1"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbiError {
    AccountTooSmall,
    BadDiscriminator,
    UnknownVersion(u16),
}

pub(crate) fn read_u16(d: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([d[off], d[off + 1]])
}

pub(crate) fn read_u64(d: &[u8], off: usize) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[off..off + 8]);
    u64::from_le_bytes(b)
}

pub(crate) fn read_i64(d: &[u8], off: usize) -> i64 {
    read_u64(d, off) as i64
}

pub(crate) fn write_u16(d: &mut [u8], off: usize, v: u16) {
    d[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

pub(crate) fn write_u64(d: &mut [u8], off: usize, v: u64) {
    d[off..off + 8].copy_from_slice(&v.to_le_bytes());
}

pub(crate) fn write_i64(d: &mut [u8], off: usize, v: i64) {
    write_u64(d, off, v as u64);
}

/// Use this to inspect `version` before a full decode.
pub fn read_header(data: &[u8]) -> Result<Header, AbiError> {
    let header = Header::decode(data)?;
    if !header.is_valid() {
        return Err(AbiError::BadDiscriminator);
    }
    Ok(header)
}

/// Derive the V1 PDA address and bump for a given program id.
pub fn find_v1_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&PDA_SEEDS_V1, program_id)
}
