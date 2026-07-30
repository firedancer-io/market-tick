use market_tick_abi::{find_v1_pda, MarketTickV1};
use solana_commitment_config::CommitmentConfig;
use solana_epoch_info::EpochInfo;
use solana_instruction::{error::InstructionError, AccountMeta, Instruction};
use solana_keypair::{read_keypair_file, Keypair};
use solana_pubkey::Pubkey;
use solana_rpc_client::{
    api::{client_error::Error as ClientError, request::RpcRequest},
    rpc_client::RpcClient,
};
use solana_signer::Signer;
use solana_transaction::Transaction;
use solana_transaction_error::TransactionError;
use std::{env, error::Error, thread, time::Duration};

const CUSTOM_ERROR_NOT_SLOT_SIGNER: u32 = 3;
const CUSTOM_ERROR_NON_MONOTONIC: u32 = 4;
const CUSTOM_ERROR_INTERVAL_MISMATCH: u32 = 5;
const CUSTOM_ERROR_INVALID_TIMESTAMP: u32 = 7;
const CUSTOM_ERROR_INVALID_INTERVAL: u32 = 8;

fn program_id() -> Pubkey {
    Pubkey::new_from_array(p_tick::id().to_bytes())
}

fn pda() -> Pubkey {
    find_v1_pda(&program_id()).0
}

fn initialize_ix(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(pda(), false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
        ],
        data: p_tick::instruction::initialize().to_vec(),
    }
}

fn increment_ix(
    account_signer: Pubkey,
    slot: u64,
    timestamp_ns: i64,
    interval_ns: u64,
) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(pda(), false),
            AccountMeta::new_readonly(account_signer, true),
        ],
        data: p_tick::instruction::increment(slot, timestamp_ns, interval_ns).to_vec(),
    }
}

fn send(
    rpc: &RpcClient,
    payer: &Keypair,
    ix: Instruction,
    extra_signers: &[&Keypair],
) -> Result<(), ClientError> {
    let mut signers = vec![payer];
    signers.extend_from_slice(extra_signers);
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &signers, blockhash);
    rpc.send_transaction(&tx).map(|_| ())
}

fn read_account(rpc: &RpcClient) -> Result<MarketTickV1, String> {
    let account = rpc.get_account(&pda()).map_err(|error| error.to_string())?;
    if account.owner != program_id() {
        return Err(format!(
            "unexpected account owner: expected {}, got {}",
            program_id(),
            account.owner
        ));
    }
    MarketTickV1::from_bytes(&account.data)
        .copied()
        .map_err(|error| format!("ABI decode failed: {error:?}"))
}

fn wait_for_account(
    rpc: &RpcClient,
    description: &str,
    predicate: impl Fn(&MarketTickV1) -> bool,
) -> MarketTickV1 {
    let mut last_observation = String::from("account was not queried");

    for _ in 0..200 {
        match read_account(rpc) {
            Ok(account) if predicate(&account) => return account,
            Ok(account) => last_observation = format!("account state was {account:?}"),
            Err(error) => last_observation = error,
        }
        thread::sleep(Duration::from_millis(10));
    }

    panic!("timed out waiting for {description}; last observation: {last_observation}");
}

fn custom_code(error: &ClientError) -> Option<u32> {
    match error.get_transaction_error() {
        Some(TransactionError::InstructionError(_, InstructionError::Custom(code))) => Some(code),
        _ => None,
    }
}

fn assert_custom(error: ClientError, expected: u32) {
    let actual = custom_code(&error);
    assert_eq!(actual, Some(expected), "unexpected RPC error: {error}");
}

fn surfnet_clock(rpc: &RpcClient, method: &'static str) -> Result<EpochInfo, ClientError> {
    rpc.send(RpcRequest::Custom { method }, serde_json::json!([]))
}

fn pause_clock(rpc: &RpcClient) -> Result<u64, ClientError> {
    surfnet_clock(rpc, "surfnet_pauseClock")?;
    Ok(rpc.get_epoch_info()?.absolute_slot)
}

fn advance_to_slot(rpc: &RpcClient, slot: u64) -> Result<u64, ClientError> {
    rpc.send(
        RpcRequest::Custom {
            method: "surfnet_timeTravel",
        },
        serde_json::json!([{ "absoluteSlot": slot }]),
    )
    .map(|info: EpochInfo| info.absolute_slot)
}

#[test]
fn instruction_layout_matches_documented_v1_abi() {
    let initialize = initialize_ix(Pubkey::new_from_array([0x11; 32]));
    assert_eq!(initialize.data, [0]);
    assert_eq!(initialize.accounts.len(), 3);
    assert!(initialize.accounts[0].is_signer);
    assert!(initialize.accounts[0].is_writable);
    assert_eq!(initialize.accounts[1].pubkey, pda());
    assert!(!initialize.accounts[1].is_signer);
    assert!(initialize.accounts[1].is_writable);
    assert_eq!(
        initialize.accounts[2].pubkey,
        solana_system_interface::program::id()
    );
    assert!(!initialize.accounts[2].is_signer);
    assert!(!initialize.accounts[2].is_writable);

    let signer = Pubkey::new_from_array([0x22; 32]);
    let slot = 0x0102_0304_0506_0708;
    let timestamp_ns = -0x0102_0304_0506_0708;
    let interval_ns = 0x1112_1314_1516_1718;
    let increment = increment_ix(signer, slot, timestamp_ns, interval_ns);
    let mut expected = vec![1];
    expected.extend_from_slice(&slot.to_le_bytes());
    expected.extend_from_slice(&timestamp_ns.to_le_bytes());
    expected.extend_from_slice(&interval_ns.to_le_bytes());

    assert_eq!(increment.data, expected);
    assert_eq!(increment.data.len(), 25);
    assert_eq!(increment.accounts.len(), 2);
    assert_eq!(increment.accounts[0].pubkey, pda());
    assert!(!increment.accounts[0].is_signer);
    assert!(increment.accounts[0].is_writable);
    assert_eq!(increment.accounts[1].pubkey, signer);
    assert!(increment.accounts[1].is_signer);
    assert!(!increment.accounts[1].is_writable);
}

#[test]
fn surfpool_sbf_scenario() -> Result<(), Box<dyn Error>> {
    let rpc_url = env::var("ANCHOR_PROVIDER_URL")?;
    let wallet_path = env::var("ANCHOR_WALLET")?;
    let payer = read_keypair_file(wallet_path)?;
    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    send(&rpc, &payer, initialize_ix(payer.pubkey()), &[])?;
    let account = wait_for_account(&rpc, "V1 account initialization", |account| {
        account == &MarketTickV1::new()
    });
    assert_eq!(account, MarketTickV1::new());

    let signer = Keypair::new();
    let slot = pause_clock(&rpc)?;

    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 0, 50),
            &[&signer],
        )
        .unwrap_err(),
        CUSTOM_ERROR_INVALID_TIMESTAMP,
    );
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 1_000, 0),
            &[&signer],
        )
        .unwrap_err(),
        CUSTOM_ERROR_INVALID_INTERVAL,
    );

    send(
        &rpc,
        &payer,
        increment_ix(signer.pubkey(), slot, 1_000, 50),
        &[&signer],
    )?;
    let account = wait_for_account(&rpc, "first increment", |account| {
        account.header.signer == signer.pubkey()
            && account.slot == slot
            && account.sequence == 0
            && account.timestamp_ns == 1_000
    });
    assert_eq!(account.header.signer, signer.pubkey());
    assert_eq!(account.slot, slot);
    assert_eq!(account.first_timestamp_ns, 1_000);
    assert_eq!(account.timestamp_ns, 1_000);
    assert_eq!(account.sequence, 0);
    assert_eq!(account.target_market_tick_interval_ns, 50);

    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 999, 50),
            &[&signer],
        )
        .unwrap_err(),
        CUSTOM_ERROR_NON_MONOTONIC,
    );
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 1_050, 51),
            &[&signer],
        )
        .unwrap_err(),
        CUSTOM_ERROR_INTERVAL_MISMATCH,
    );

    let foreign = Keypair::new();
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(foreign.pubkey(), slot, 1_050, 50),
            &[&foreign],
        )
        .unwrap_err(),
        CUSTOM_ERROR_NOT_SLOT_SIGNER,
    );

    send(
        &rpc,
        &payer,
        increment_ix(signer.pubkey(), slot, 1_050, 50),
        &[&signer],
    )?;
    let account = wait_for_account(&rpc, "second increment", |account| {
        account.slot == slot && account.sequence == 1 && account.timestamp_ns == 1_050
    });
    assert_eq!(account.sequence, 1);
    assert_eq!(account.timestamp_ns, 1_050);
    assert_eq!(account.first_timestamp_ns, 1_000);

    let next_slot = advance_to_slot(&rpc, slot + 1)?;
    assert_eq!(next_slot, slot + 1);
    let next_signer = Keypair::new();
    send(
        &rpc,
        &payer,
        increment_ix(next_signer.pubkey(), next_slot, 500, 100),
        &[&next_signer],
    )?;
    let account = wait_for_account(&rpc, "first increment in the next slot", |account| {
        account.header.signer == next_signer.pubkey()
            && account.slot == next_slot
            && account.sequence == 0
            && account.timestamp_ns == 500
    });
    assert_eq!(account.header.signer, next_signer.pubkey());
    assert_eq!(account.slot, next_slot);
    assert_eq!(account.sequence, 0);
    assert_eq!(account.first_timestamp_ns, 500);
    assert_eq!(account.target_market_tick_interval_ns, 100);

    Ok(())
}
