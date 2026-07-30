#![cfg_attr(target_os = "solana", no_std)]

pub mod error;
mod processor;

pub use market_tick_abi as abi;

use pinocchio::{
    address::declare_id, error::ProgramError, AccountView, Address, ProgramResult,
};

declare_id!("bUD41ixzckBZ6bq2Zy1aUnNvnLF9vDuMq1AxjMVH25z");

/// Canonical V1 PDA bump for seeds `[b"market_tick", b"v1"]`.
pub const PDA_BUMP: u8 = 255;

/// Canonical V1 PDA address (const-derived; no syscall).
pub const PDA: Address = Address::derive_address_const(
    &[b"market_tick", b"v1"],
    Some(PDA_BUMP),
    &Address::from_str_const("bUD41ixzckBZ6bq2Zy1aUnNvnLF9vDuMq1AxjMVH25z"),
);

/// Client helpers for building instruction data (tag + LE payload).
pub mod instruction {
    /// Initialize V1 — tag `0`.
    pub fn initialize() -> [u8; 1] {
        [0]
    }

    /// Increment V1 — tag `1` + slot + timestamp_ns + interval.
    pub fn increment(slot: u64, timestamp_ns: i64, target_market_tick_interval_ns: u64) -> [u8; 25] {
        let mut data = [0u8; 25];
        data[0] = 1;
        data[1..9].copy_from_slice(&slot.to_le_bytes());
        data[9..17].copy_from_slice(&timestamp_ns.to_le_bytes());
        data[17..25].copy_from_slice(&target_market_tick_interval_ns.to_le_bytes());
        data
    }
}

#[cfg(target_os = "solana")]
use pinocchio::{no_allocator, nostd_panic_handler, program_entrypoint};

#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
program_entrypoint!(process_instruction);

#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
nostd_panic_handler!();

#[cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]
no_allocator!();

/// Program entrypoint.
pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    match instruction_data.split_first() {
        Some((&0, _)) => processor::initialize(accounts),
        Some((&1, rest)) => {
            let (slot, timestamp_ns, interval) = processor::parse_increment(rest)?;
            processor::increment(accounts, slot, timestamp_ns, interval)
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use market_tick_abi::find_v1_pda;
    use solana_pubkey::Pubkey;

    #[test]
    fn const_pda_matches_find_program_address() {
        let program = Pubkey::new_from_array(id().to_bytes());
        let (pda, bump) = find_v1_pda(&program);
        assert_eq!(bump, PDA_BUMP);
        assert_eq!(pda.to_bytes(), PDA.to_bytes());
    }
}
