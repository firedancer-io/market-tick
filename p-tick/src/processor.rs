use market_tick_abi::{MarketTickV1, PDA_SEEDS_V1};
use pinocchio::{
    cpi::{Seed, Signer},
    error::ProgramError,
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    AccountView, ProgramResult,
};
use pinocchio_system::create_account_with_minimum_balance_signed;
use solana_pubkey::Pubkey;

use crate::{error::MarketTickError, ID, PDA, PDA_BUMP};

#[inline(always)]
pub(crate) fn initialize(accounts: &mut [AccountView]) -> ProgramResult {
    let [payer, pda, system_program, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if system_program.address() != &pinocchio_system::ID {
        return Err(ProgramError::IncorrectProgramId);
    }
    if !system_program.executable() {
        return Err(ProgramError::InvalidAccountData);
    }

    if !payer.is_writable() {
        return Err(ProgramError::Immutable);
    }
    if pda.address() != &PDA {
        return Err(ProgramError::InvalidSeeds);
    }
    if !pda.is_writable() {
        return Err(ProgramError::Immutable);
    }

    let _rent = Rent::get()?;

    if pda.owner() == &ID && pda.data_len() > 0 {
        return Err(MarketTickError::AlreadyInitialized.into());
    }
    if pda.owner() != &pinocchio_system::ID || pda.data_len() != 0 {
        return Err(MarketTickError::InvalidAccountData.into());
    }

    let bump = [PDA_BUMP];
    let seeds = [
        Seed::from(PDA_SEEDS_V1[0]),
        Seed::from(PDA_SEEDS_V1[1]),
        Seed::from(bump.as_slice()),
    ];
    let signers = [Signer::from(&seeds)];

    create_account_with_minimum_balance_signed(pda, MarketTickV1::LEN, &ID, payer, None, &signers)?;

    let mut data = pda.try_borrow_mut()?;
    MarketTickV1::new()
        .encode(&mut data)
        .map_err(|_| MarketTickError::InvalidAccountData)?;
    Ok(())
}

#[inline(always)]
pub(crate) fn increment(
    accounts: &mut [AccountView],
    slot: u64,
    timestamp_ns: i64,
    target_market_tick_interval_ns: u64,
) -> ProgramResult {
    let [pda, signer, ..] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    if !signer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }
    if pda.address() != &PDA {
        return Err(ProgramError::InvalidSeeds);
    }
    if !pda.is_writable() {
        return Err(ProgramError::Immutable);
    }
    if pda.owner() != &ID {
        return Err(ProgramError::InvalidAccountOwner);
    }

    if slot != Clock::get()?.slot {
        return Err(MarketTickError::SlotMismatch.into());
    }
    if timestamp_ns <= 0 {
        return Err(MarketTickError::InvalidTimestamp.into());
    }
    if target_market_tick_interval_ns == 0 {
        return Err(MarketTickError::InvalidInterval.into());
    }

    let mut data = pda.try_borrow_mut()?;
    let acct =
        MarketTickV1::from_bytes_mut(&mut data).map_err(|_| MarketTickError::InvalidAccountData)?;
    let first_increment = acct.header.signer == Pubkey::default() || acct.slot != slot;

    if first_increment {
        acct.header.signer = Pubkey::new_from_array(*signer.address().as_array());
        acct.slot = slot;
        acct.first_timestamp_ns = timestamp_ns;
        acct.timestamp_ns = timestamp_ns;
        acct.sequence = 0;
        acct.target_market_tick_interval_ns = target_market_tick_interval_ns;
    } else {
        if acct.header.signer.as_ref() != signer.address().as_ref() {
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

    Ok(())
}

#[inline(always)]
pub(crate) fn parse_increment(data: &[u8]) -> Result<(u64, i64, u64), ProgramError> {
    if data.len() < 24 {
        return Err(ProgramError::InvalidInstructionData);
    }
    let slot = u64::from_le_bytes(data[0..8].try_into().unwrap());
    let timestamp_ns = i64::from_le_bytes(data[8..16].try_into().unwrap());
    let interval = u64::from_le_bytes(data[16..24].try_into().unwrap());
    Ok((slot, timestamp_ns, interval))
}
