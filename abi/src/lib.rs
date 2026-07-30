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

/// Use this to inspect `version` before a full decode.
pub fn read_header(data: &[u8]) -> Result<Header, AbiError> {
    Header::decode(data)
}

/// Derive the V1 PDA address and bump for a given program id.
pub fn find_v1_pda(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&PDA_SEEDS_V1, program_id)
}
