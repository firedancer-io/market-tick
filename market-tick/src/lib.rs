pub mod error;
mod processor;

pub use market_tick_abi as abi;

use anchor_lang::prelude::*;
use market_tick_abi::PDA_SEEDS_V1;

declare_id!("Fg6PaFpoGXkYsidMpWxTWqkZK6W2BeZ7FEfcYkgMQhg");

#[program]
pub mod market_tick {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        processor::initialize_v1(
            ctx.program_id,
            &ctx.accounts.payer.to_account_info(),
            &ctx.accounts.pda.to_account_info(),
            &ctx.accounts.system_program.to_account_info(),
        )
        .map_err(Into::into)
    }

    #[instruction(discriminator = 1)]
    pub fn increment(
        ctx: Context<Increment>,
        slot: u64,
        timestamp_ns: i64,
        target_market_tick_interval_ns: u64,
    ) -> Result<()> {
        processor::increment_v1(
            &ctx.accounts.pda.to_account_info(),
            ctx.accounts.signer_account.key(),
            slot,
            timestamp_ns,
            target_market_tick_interval_ns,
        )
        .map_err(Into::into)
    }
}

/// Anchor validates that the payer signed, the PDA is canonical and writable,
/// and the supplied system program is the real System Program before dispatch.
#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(mut, seeds = [PDA_SEEDS_V1[0], PDA_SEEDS_V1[1]], bump)]
    pub pda: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// Anchor validates that the publisher signed and that the tick account is the
/// canonical, writable, program-owned V1 PDA before dispatch.
#[derive(Accounts)]
pub struct Increment<'info> {
    #[account(
            mut,
            seeds = [PDA_SEEDS_V1[0], PDA_SEEDS_V1[1]],
            bump,
            owner = crate::ID
        )]
    pub pda: UncheckedAccount<'info>,
    pub signer_account: Signer<'info>,
}
