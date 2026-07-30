use anchor_lang::InstructionData;
use market_tick::instruction::Increment;
use market_tick_abi::{error_code, MarketTickV1};
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
use std::{
    env,
    error::Error,
    io::{self, Write},
    thread,
    time::Duration,
};

fn program_id() -> Pubkey {
    market_tick::id()
}

fn pda() -> Pubkey {
    Pubkey::find_program_address(
        &[
            market_tick_abi::PDA_SEED_MARKET_TICK,
            market_tick_abi::PDA_SEED_VERSION_V1,
        ],
        &program_id(),
    )
    .0
}

fn initialize_ix(payer: Pubkey) -> Instruction {
    initialize_ix_with_accounts(payer, pda(), solana_system_interface::program::id())
}

fn initialize_ix_with_accounts(
    payer: Pubkey,
    tick_account: Pubkey,
    system_program: Pubkey,
) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(tick_account, false),
            AccountMeta::new_readonly(system_program, false),
        ],
        data: market_tick::instruction::Initialize {}.data(),
    }
}

fn increment_ix(
    account_signer: Pubkey,
    slot: u64,
    timestamp_ns: i64,
    interval_ns: u64,
) -> Instruction {
    increment_ix_with_accounts(pda(), account_signer, true, slot, timestamp_ns, interval_ns)
}

fn increment_ix_with_accounts(
    tick_account: Pubkey,
    account_signer: Pubkey,
    signer_required: bool,
    slot: u64,
    timestamp_ns: i64,
    interval_ns: u64,
) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(tick_account, false),
            AccountMeta::new_readonly(account_signer, signer_required),
        ],
        data: Increment {
            slot,
            timestamp_ns,
            target_market_tick_interval_ns: interval_ns,
        }
        .data(),
    }
}

fn send(
    rpc: &RpcClient,
    payer: &Keypair,
    mut ix: Instruction,
    extra_signers: &[&Keypair],
) -> Result<String, ClientError> {
    // The Surfpool clock is paused during most assertions, so the recent
    // blockhash does not change. An extra dummy account gives repeated
    // instructions distinct transaction signatures.
    ix.accounts
        .push(AccountMeta::new_readonly(Keypair::new().pubkey(), false));

    let mut signers = vec![payer];
    signers.extend_from_slice(extra_signers);
    let blockhash = rpc.get_latest_blockhash()?;
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &signers, blockhash);
    rpc.send_transaction(&tx)
        .map(|signature| signature.to_string())
}

fn compute_units_consumed(rpc: &RpcClient, signature: &str) -> Result<u64, String> {
    let params = serde_json::json!([
        signature,
        {
            "commitment": "confirmed",
            "encoding": "json",
            "maxSupportedTransactionVersion": 0
        }
    ]);
    let mut last_observation = String::from("transaction was not queried");

    for _ in 0..200 {
        match rpc.send::<serde_json::Value>(
            RpcRequest::Custom {
                method: "getTransaction",
            },
            params.clone(),
        ) {
            Ok(response) => {
                if let Some(units) = response
                    .get("meta")
                    .and_then(|meta| meta.get("computeUnitsConsumed"))
                    .and_then(serde_json::Value::as_u64)
                {
                    return Ok(units);
                }
                last_observation = format!("RPC response was {response}");
            }
            Err(error) => last_observation = error.to_string(),
        }
        thread::sleep(Duration::from_millis(10));
    }

    Err(format!(
        "timed out waiting for compute units for transaction {signature}; last observation: {last_observation}"
    ))
}

fn print_compute_unit_summary(
    initialize: u64,
    first_increment: u64,
    second_increment: u64,
) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    writeln!(
        stdout,
        "\nCompute unit summary\n\
         initialize: {initialize} CU\n\
         increment0: {first_increment} CU\n\
         increment1: {second_increment} CU"
    )?;
    stdout.flush()
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
    MarketTickV1::decode(&account.data).map_err(|error| format!("ABI decode failed: {error:?}"))
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

fn assert_rejected(result: Result<String, ClientError>, description: &str) {
    assert!(result.is_err(), "{description} unexpectedly succeeded");
}

fn assert_state_unchanged(rpc: &RpcClient, expected: &MarketTickV1) {
    let actual = read_account(rpc).expect("tick account should remain readable");
    assert_eq!(
        &actual, expected,
        "rejected instruction changed account state"
    );
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
#[ignore = "requires Surfpool and Anchor provider configuration"]
fn integration_scenario() -> Result<(), Box<dyn Error>> {
    let rpc_url = env::var("ANCHOR_PROVIDER_URL")?;
    let wallet_path = env::var("ANCHOR_WALLET")?;
    let payer = read_keypair_file(wallet_path)?;
    let rpc = RpcClient::new_with_commitment(rpc_url, CommitmentConfig::confirmed());

    let prefunded_lamports = rpc.get_minimum_balance_for_rent_exemption(0)?;
    send(
        &rpc,
        &payer,
        solana_system_interface::instruction::transfer(&payer.pubkey(), &pda(), prefunded_lamports),
        &[],
    )?;
    assert_eq!(rpc.get_balance(&pda())?, prefunded_lamports);

    let signer = Keypair::new();
    let slot = pause_clock(&rpc)?;
    assert_rejected(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 1_000, 50),
            &[&signer],
        ),
        "increment with a System-owned derived PDA",
    );

    let mut suffixed_initialize = initialize_ix(payer.pubkey());
    suffixed_initialize.data.push(0);
    let initialize_signature = send(&rpc, &payer, suffixed_initialize, &[])?;
    let account = wait_for_account(
        &rpc,
        "V1 account initialization with trailing data",
        |account| account == &MarketTickV1::default(),
    );
    assert_eq!(account, MarketTickV1::default());
    let initialize_compute_units = compute_units_consumed(&rpc, &initialize_signature)?;

    assert_rejected(
        send(&rpc, &payer, initialize_ix(payer.pubkey()), &[]),
        "reinitialization of the existing V1 account",
    );
    assert_state_unchanged(&rpc, &account);

    assert_rejected(
        send(
            &rpc,
            &payer,
            initialize_ix_with_accounts(
                payer.pubkey(),
                Keypair::new().pubkey(),
                solana_system_interface::program::id(),
            ),
            &[],
        ),
        "initialization with an incorrect PDA",
    );
    assert_state_unchanged(&rpc, &account);

    assert_rejected(
        send(
            &rpc,
            &payer,
            initialize_ix_with_accounts(payer.pubkey(), pda(), Keypair::new().pubkey()),
            &[],
        ),
        "initialization with a false System Program",
    );
    assert_state_unchanged(&rpc, &account);

    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot + 1, 1_000, 50),
            &[&signer],
        )
        .unwrap_err(),
        error_code::SLOT_MISMATCH,
    );
    assert_state_unchanged(&rpc, &account);

    assert_rejected(
        send(
            &rpc,
            &payer,
            increment_ix_with_accounts(
                Keypair::new().pubkey(),
                signer.pubkey(),
                true,
                slot,
                1_000,
                50,
            ),
            &[&signer],
        ),
        "increment with a valid PDA",
    );
    assert_state_unchanged(&rpc, &account);

    assert_rejected(
        send(
            &rpc,
            &payer,
            increment_ix_with_accounts(pda(), signer.pubkey(), false, slot, 1_000, 50),
            &[],
        ),
        "increment without signer privilege",
    );
    assert_state_unchanged(&rpc, &account);

    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, -1, 50),
            &[&signer],
        )
        .unwrap_err(),
        error_code::INVALID_TIMESTAMP,
    );
    assert_state_unchanged(&rpc, &account);
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 0, 50),
            &[&signer],
        )
        .unwrap_err(),
        error_code::INVALID_TIMESTAMP,
    );
    assert_state_unchanged(&rpc, &account);
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 1_000, 0),
            &[&signer],
        )
        .unwrap_err(),
        error_code::INVALID_INTERVAL,
    );
    assert_state_unchanged(&rpc, &account);

    let mut suffixed_increment = increment_ix(signer.pubkey(), slot, 1_000, 50);
    suffixed_increment.data.push(0);
    let first_increment_signature = send(&rpc, &payer, suffixed_increment, &[&signer])?;
    let account = wait_for_account(&rpc, "first increment with trailing data", |account| {
        account.signer == signer.pubkey().to_bytes()
            && account.slot == slot
            && account.sequence == 0
            && account.timestamp_ns == 1_000
    });
    assert_eq!(account.signer, signer.pubkey().to_bytes());
    assert_eq!(account.slot, slot);
    assert_eq!(account.first_timestamp_ns, 1_000);
    assert_eq!(account.timestamp_ns, 1_000);
    assert_eq!(account.sequence, 0);
    assert_eq!(account.target_market_tick_interval_ns, 50);
    assert_eq!(account.observed_market_tick_interval_ns, 0);
    let first_increment_compute_units = compute_units_consumed(&rpc, &first_increment_signature)?;

    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 999, 50),
            &[&signer],
        )
        .unwrap_err(),
        error_code::NON_MONOTONIC,
    );
    assert_state_unchanged(&rpc, &account);
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 1_000, 50),
            &[&signer],
        )
        .unwrap_err(),
        error_code::NON_MONOTONIC,
    );
    assert_state_unchanged(&rpc, &account);
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), slot, 1_050, 51),
            &[&signer],
        )
        .unwrap_err(),
        error_code::INTERVAL_MISMATCH,
    );
    assert_state_unchanged(&rpc, &account);

    let foreign = Keypair::new();
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(foreign.pubkey(), slot, 1_050, 50),
            &[&foreign],
        )
        .unwrap_err(),
        error_code::NOT_SLOT_SIGNER,
    );
    assert_state_unchanged(&rpc, &account);

    let second_increment_signature = send(
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
    assert_eq!(account.observed_market_tick_interval_ns, 50);
    let second_increment_compute_units = compute_units_consumed(&rpc, &second_increment_signature)?;

    let attacker = Keypair::new();
    let contested_slot = advance_to_slot(&rpc, slot + 1)?;
    assert_eq!(contested_slot, slot + 1);
    send(
        &rpc,
        &payer,
        increment_ix(attacker.pubkey(), contested_slot, i64::MAX, u64::MAX),
        &[&attacker],
    )?;
    let captured = wait_for_account(&rpc, "first-writer slot capture", |account| {
        account.signer == attacker.pubkey().to_bytes()
            && account.slot == contested_slot
            && account.sequence == 0
            && account.timestamp_ns == i64::MAX
    });
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(attacker.pubkey(), contested_slot, i64::MAX, u64::MAX),
            &[&attacker],
        )
        .unwrap_err(),
        error_code::NON_MONOTONIC,
    );
    assert_state_unchanged(&rpc, &captured);
    assert_custom(
        send(
            &rpc,
            &payer,
            increment_ix(signer.pubkey(), contested_slot, i64::MAX, u64::MAX),
            &[&signer],
        )
        .unwrap_err(),
        error_code::NOT_SLOT_SIGNER,
    );
    assert_state_unchanged(&rpc, &captured);

    let next_slot = advance_to_slot(&rpc, contested_slot + 1)?;
    assert_eq!(next_slot, contested_slot + 1);
    let next_signer = Keypair::new();
    send(
        &rpc,
        &payer,
        increment_ix(next_signer.pubkey(), next_slot, 500, 100),
        &[&next_signer],
    )?;
    let account = wait_for_account(&rpc, "first increment in the next slot", |account| {
        account.signer == next_signer.pubkey().to_bytes()
            && account.slot == next_slot
            && account.sequence == 0
            && account.timestamp_ns == 500
    });
    assert_eq!(account.signer, next_signer.pubkey().to_bytes());
    assert_eq!(account.slot, next_slot);
    assert_eq!(account.sequence, 0);
    assert_eq!(account.first_timestamp_ns, 500);
    assert_eq!(account.target_market_tick_interval_ns, 100);
    assert_eq!(account.observed_market_tick_interval_ns, 0);

    print_compute_unit_summary(
        initialize_compute_units,
        first_increment_compute_units,
        second_increment_compute_units,
    )?;

    Ok(())
}
