use anchor_lang::{
    prelude::{AccountInfo, Clock, Pubkey, Rent, SolanaSysvar},
    solana_program::{entrypoint::ProgramResult, msg, system_instruction},
};
use market_tick_abi::{MarketTickV1, PDA_SEEDS_V1};

use crate::error::MarketTickError;

pub(super) fn initialize_v1<'info>(
    program_id: &Pubkey,
    payer: &AccountInfo<'info>,
    pda: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
) -> ProgramResult {
    let (_, bump) = Pubkey::find_program_address(&PDA_SEEDS_V1, program_id);
    let space = MarketTickV1::LEN;
    let rent = Rent::get()?;
    let signer_seeds = &[PDA_SEEDS_V1[0], PDA_SEEDS_V1[1], &[bump]];

    if pda.owner == program_id && !pda.data_is_empty() {
        return Err(MarketTickError::AlreadyInitialized.into());
    } else if pda.owner == &anchor_lang::system_program::ID && pda.data_is_empty() {
        let minimum_balance = rent.minimum_balance(space);
        let missing_lamports = minimum_balance.saturating_sub(pda.lamports());
        if missing_lamports > 0 {
            let transfer_ix = system_instruction::transfer(payer.key, pda.key, missing_lamports);
            invoke_signed_compat(
                &transfer_ix,
                &[payer.clone(), pda.clone(), system_program.clone()],
                &[],
            )?;
        }

        let allocate_ix = system_instruction::allocate(pda.key, space as u64);
        invoke_signed_compat(
            &allocate_ix,
            &[pda.clone(), system_program.clone()],
            &[signer_seeds],
        )?;

        let assign_ix = system_instruction::assign(pda.key, program_id);
        invoke_signed_compat(
            &assign_ix,
            &[pda.clone(), system_program.clone()],
            &[signer_seeds],
        )?;
    } else {
        return Err(MarketTickError::InvalidAccountData.into());
    }

    let mut data = pda.try_borrow_mut_data()?;
    MarketTickV1::new()
        .encode(&mut data)
        .map_err(|_| MarketTickError::InvalidAccountData)?;
    msg!("market-tick: initialized");
    Ok(())
}

#[cfg(target_os = "solana")]
fn invoke_signed_compat(
    instruction: &anchor_lang::solana_program::instruction::Instruction,
    account_infos: &[AccountInfo],
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    anchor_lang::solana_program::program::invoke_signed(instruction, account_infos, signer_seeds)
}

#[cfg(not(target_os = "solana"))]
fn invoke_signed_compat(
    instruction: &anchor_lang::solana_program::instruction::Instruction,
    account_infos: &[AccountInfo],
    signer_seeds: &[&[&[u8]]],
) -> ProgramResult {
    solana_program::program::invoke_signed(instruction, account_infos, signer_seeds)
}

pub(super) fn increment_v1(
    pda: &AccountInfo,
    signer: Pubkey,
    slot: u64,
    timestamp_ns: i64,
    target_market_tick_interval_ns: u64,
) -> ProgramResult {
    if slot != Clock::get()?.slot {
        return Err(MarketTickError::SlotMismatch.into());
    }

    if timestamp_ns <= 0 {
        return Err(MarketTickError::InvalidTimestamp.into());
    }
    if target_market_tick_interval_ns == 0 {
        return Err(MarketTickError::InvalidInterval.into());
    }

    let mut data = pda.try_borrow_mut_data()?;
    let mut acct = MarketTickV1::decode(&data).map_err(|_| MarketTickError::InvalidAccountData)?;
    let first_increment = acct.header.signer == Pubkey::default() || acct.slot != slot;

    if first_increment {
        acct.header.signer = signer;
        acct.slot = slot;
        acct.first_timestamp_ns = timestamp_ns;
        acct.timestamp_ns = timestamp_ns;
        acct.sequence = 0;
        acct.target_market_tick_interval_ns = target_market_tick_interval_ns;
    } else {
        if acct.header.signer != signer {
            return Err(MarketTickError::NotSlotSigner.into());
        }
        if acct.target_market_tick_interval_ns != target_market_tick_interval_ns {
            return Err(MarketTickError::IntervalMismatch.into());
        }
        if timestamp_ns <= acct.timestamp_ns {
            return Err(MarketTickError::NonMonotonic.into());
        }
        acct.sequence = acct
            .sequence
            .checked_add(1)
            .ok_or(MarketTickError::CounterOverflow)?;
        acct.timestamp_ns = timestamp_ns;
    }

    acct.encode(&mut data)
        .map_err(|_| MarketTickError::InvalidAccountData)?;
    Ok(())
}
