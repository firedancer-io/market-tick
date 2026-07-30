mod account;
pub mod error;
mod processor;

pub use market_tick_abi as abi;

use account::MarketTickV1Account;
use anchor_lang::prelude::*;
use market_tick_abi::{PDA_SEED_MARKET_TICK, PDA_SEED_VERSION_V1};

declare_id!("tickUcsEQegChaAuo9VYQQztB4ZGApY6ZT4FkULWY6N");

const PDA_V1: Pubkey = pubkey!("4cG31VNF9TzFinNc7BmnjhFvGjxkY3sCETVMtMgbrhPs");

#[program]
pub mod market_tick {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        msg!("market-tick: initialized");
        Ok(())
    }

    #[instruction(discriminator = 1)]
    pub fn increment(
        ctx: Context<Increment>,
        slot: u64,
        timestamp_ns: i64,
        target_market_tick_interval_ns: u64,
    ) -> Result<()> {
        if ctx.accounts.pda.key() != PDA_V1 {
            return Err(anchor_lang::error::ErrorCode::ConstraintSeeds.into());
        }

        processor::increment_v1(
            &mut ctx.accounts.pda.0,
            ctx.accounts.signer.key(),
            Clock::get()?.slot,
            slot,
            timestamp_ns,
            target_market_tick_interval_ns,
        )
    }
}

/// Anchor creates and initializes the canonical V1 PDA, including support for
/// an address that was prefunded before initialization.
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        init,
        payer = payer,
        space = MarketTickV1Account::LEN,
        seeds = [PDA_SEED_MARKET_TICK, PDA_SEED_VERSION_V1],
        bump
    )]
    pub pda: Account<'info, MarketTickV1Account>,
    pub system_program: Program<'info, System>,
}

/// Anchor validates that the signer signed and that the tick account is
/// writable, program-owned, and decodable. The handler validates its canonical
/// address with a direct key comparison to avoid deriving the PDA on every tick.
#[derive(Accounts)]
pub struct Increment<'info> {
    #[account(mut)]
    pub pda: Account<'info, MarketTickV1Account>,
    pub signer: Signer<'info>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::{InstructionData, ToAccountMetas};

    #[test]
    fn increment_address_matches_v1_seeds() {
        assert_eq!(
            PDA_V1,
            Pubkey::find_program_address(&[PDA_SEED_MARKET_TICK, PDA_SEED_VERSION_V1], &crate::ID,)
                .0
        );
    }

    #[test]
    fn instruction_layout_matches_documented_v1_abi() {
        let payer = Pubkey::new_from_array([0x11; 32]);
        let pda =
            Pubkey::find_program_address(&[PDA_SEED_MARKET_TICK, PDA_SEED_VERSION_V1], &crate::ID)
                .0;
        let initialize_data = crate::instruction::Initialize {}.data();
        let initialize_accounts = crate::accounts::Initialize {
            payer,
            pda,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None);

        assert_eq!(initialize_data, [0]);
        assert_eq!(initialize_accounts.len(), 3);
        assert!(initialize_accounts[0].is_signer);
        assert!(initialize_accounts[0].is_writable);
        assert_eq!(initialize_accounts[1].pubkey, pda);
        assert!(!initialize_accounts[1].is_signer);
        assert!(initialize_accounts[1].is_writable);
        assert_eq!(
            initialize_accounts[2].pubkey,
            anchor_lang::system_program::ID
        );
        assert!(!initialize_accounts[2].is_signer);
        assert!(!initialize_accounts[2].is_writable);

        let signer = Pubkey::new_from_array([0x22; 32]);
        let slot = 0x0102_0304_0506_0708;
        let timestamp_ns = -0x0102_0304_0506_0708;
        let interval_ns = 0x1112_1314_1516_1718;
        let increment_data = crate::instruction::Increment {
            slot,
            timestamp_ns,
            target_market_tick_interval_ns: interval_ns,
        }
        .data();
        let increment_accounts = crate::accounts::Increment { pda, signer }.to_account_metas(None);
        let mut expected = vec![1];
        expected.extend_from_slice(&slot.to_le_bytes());
        expected.extend_from_slice(&timestamp_ns.to_le_bytes());
        expected.extend_from_slice(&interval_ns.to_le_bytes());

        assert_eq!(increment_data, expected);
        assert_eq!(increment_data.len(), 25);
        assert_eq!(increment_accounts.len(), 2);
        assert_eq!(increment_accounts[0].pubkey, pda);
        assert!(!increment_accounts[0].is_signer);
        assert!(increment_accounts[0].is_writable);
        assert_eq!(increment_accounts[1].pubkey, signer);
        assert!(increment_accounts[1].is_signer);
        assert!(!increment_accounts[1].is_writable);
    }
}
