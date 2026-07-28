use anchor_lang::InstructionData;
use market_tick::error::MarketTickError;
use market_tick_abi::{find_v1_pda, MarketTickV1};
use solana_instruction::{error::InstructionError, AccountMeta, Instruction};
use solana_keypair::Keypair;
use solana_program_test::{processor, BanksClient, ProgramTest, ProgramTestContext};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::Transaction;

fn program_id() -> Pubkey {
    market_tick::id()
}

fn pda() -> Pubkey {
    find_v1_pda(&program_id()).0
}

fn process_anchor_entry<'a, 'b, 'c, 'd>(
    program_id: &'a Pubkey,
    accounts: &'b [anchor_lang::prelude::AccountInfo<'c>],
    data: &'d [u8],
) -> Result<(), anchor_lang::solana_program::program_error::ProgramError> {
    // Anchor's generated entrypoint ties the slice and AccountInfo lifetimes
    // together, while ProgramTest exposes them independently.
    let accounts: &'c [anchor_lang::prelude::AccountInfo<'c>] =
        unsafe { core::mem::transmute(accounts) };
    market_tick::entry(program_id, accounts, data)
}

async fn setup() -> ProgramTestContext {
    ProgramTest::new(
        "market_tick",
        program_id(),
        processor!(process_anchor_entry),
    )
    .start_with_context()
    .await
}

fn initialize_ix(payer: Pubkey) -> Instruction {
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(pda(), false),
            AccountMeta::new_readonly(solana_system_interface::program::id(), false),
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
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(pda(), false),
            AccountMeta::new_readonly(account_signer, true),
        ],
        data: market_tick::instruction::Increment {
            slot,
            timestamp_ns,
            target_market_tick_interval_ns: interval_ns,
        }
        .data(),
    }
}

async fn send(
    ctx: &mut ProgramTestContext,
    ix: Instruction,
    extra_signers: &[&Keypair],
) -> Result<(), solana_program_test::BanksClientError> {
    let mut signers = vec![&ctx.payer];
    signers.extend_from_slice(extra_signers);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &signers,
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await
}

async fn init(ctx: &mut ProgramTestContext) {
    send(ctx, initialize_ix(ctx.payer.pubkey()), &[])
        .await
        .unwrap();
}

async fn prefund_pda(ctx: &mut ProgramTestContext, lamports: u64) {
    let ix = solana_system_interface::instruction::transfer(&ctx.payer.pubkey(), &pda(), lamports);
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        ctx.last_blockhash,
    );
    ctx.banks_client.process_transaction(tx).await.unwrap();
}

async fn read_account(banks: &mut BanksClient) -> market_tick_abi::MarketTickV1 {
    let account = banks.get_account(pda()).await.unwrap().unwrap();
    MarketTickV1::decode(&account.data).unwrap()
}

fn assert_custom(err: solana_program_test::BanksClientError, expected: MarketTickError) {
    use solana_transaction_error::TransactionError;
    match err {
        solana_program_test::BanksClientError::TransactionError(
            TransactionError::InstructionError(_, InstructionError::Custom(code)),
        ) => assert_eq!(code, expected as u32),
        other => panic!("expected custom error, got {other:?}"),
    }
}

#[test]
fn anchor_instruction_layout_matches_documented_v1_abi() {
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

#[tokio::test]
async fn initialize_creates_v1_account() {
    let mut ctx = setup().await;
    init(&mut ctx).await;
    let account = read_account(&mut ctx.banks_client).await;
    assert_eq!(account.header.version, 1);
    assert_eq!(account.header.signer, Pubkey::default());
}

#[tokio::test]
async fn initialize_handles_prefunded_v1_pda() {
    let mut ctx = setup().await;
    prefund_pda(&mut ctx, 1_000_000).await;

    init(&mut ctx).await;

    let raw = ctx.banks_client.get_account(pda()).await.unwrap().unwrap();
    assert_eq!(raw.owner, program_id());
    assert_eq!(raw.data.len(), MarketTickV1::LEN);
    assert_eq!(
        MarketTickV1::decode(&raw.data).unwrap(),
        MarketTickV1::new()
    );
}

#[tokio::test]
async fn first_increment_opens_slot_and_later_increments_counter() {
    let mut ctx = setup().await;
    init(&mut ctx).await;
    ctx.warp_to_slot(5).unwrap();
    let signer = Keypair::new();

    send(
        &mut ctx,
        increment_ix(signer.pubkey(), 5, 1_000, 50),
        &[&signer],
    )
    .await
    .unwrap();
    let account = read_account(&mut ctx.banks_client).await;
    assert_eq!(account.header.signer, signer.pubkey());
    assert_eq!(account.slot, 5);
    assert_eq!(account.first_timestamp_ns, 1_000);
    assert_eq!(account.timestamp_ns, 1_000);
    assert_eq!(account.sequence, 0);
    assert_eq!(account.target_market_tick_interval_ns, 50);

    send(
        &mut ctx,
        increment_ix(signer.pubkey(), 5, 1_050, 50),
        &[&signer],
    )
    .await
    .unwrap();
    let account = read_account(&mut ctx.banks_client).await;
    assert_eq!(account.sequence, 1);
    assert_eq!(account.timestamp_ns, 1_050);
    assert_eq!(account.first_timestamp_ns, 1_000);
}

#[tokio::test]
async fn new_slot_resets_counter_and_accepts_new_signer() {
    let mut ctx = setup().await;
    init(&mut ctx).await;
    ctx.warp_to_slot(5).unwrap();
    let first = Keypair::new();
    send(
        &mut ctx,
        increment_ix(first.pubkey(), 5, 1_000, 50),
        &[&first],
    )
    .await
    .unwrap();

    ctx.warp_to_slot(6).unwrap();
    let second = Keypair::new();
    send(
        &mut ctx,
        increment_ix(second.pubkey(), 6, 500, 100),
        &[&second],
    )
    .await
    .unwrap();
    let account = read_account(&mut ctx.banks_client).await;
    assert_eq!(account.header.signer, second.pubkey());
    assert_eq!(account.sequence, 0);
    assert_eq!(account.first_timestamp_ns, 500);
    assert_eq!(account.target_market_tick_interval_ns, 100);
}

#[tokio::test]
async fn rejects_foreign_signer_within_slot() {
    let mut ctx = setup().await;
    init(&mut ctx).await;
    ctx.warp_to_slot(5).unwrap();
    let owner = Keypair::new();
    send(
        &mut ctx,
        increment_ix(owner.pubkey(), 5, 1_000, 50),
        &[&owner],
    )
    .await
    .unwrap();

    let foreign = Keypair::new();
    let err = send(
        &mut ctx,
        increment_ix(foreign.pubkey(), 5, 1_050, 50),
        &[&foreign],
    )
    .await
    .unwrap_err();
    assert_custom(err, MarketTickError::NotSlotSigner);
}

#[tokio::test]
async fn rejects_invalid_values_wrong_slot_and_interval_change() {
    let mut ctx = setup().await;
    init(&mut ctx).await;
    ctx.warp_to_slot(5).unwrap();
    let signer = Keypair::new();

    let err = send(
        &mut ctx,
        increment_ix(signer.pubkey(), 6, 1_000, 50),
        &[&signer],
    )
    .await
    .unwrap_err();
    assert_custom(err, MarketTickError::SlotMismatch);

    let err = send(
        &mut ctx,
        increment_ix(signer.pubkey(), 5, 0, 50),
        &[&signer],
    )
    .await
    .unwrap_err();
    assert_custom(err, MarketTickError::InvalidTimestamp);

    let err = send(
        &mut ctx,
        increment_ix(signer.pubkey(), 5, 1_000, 0),
        &[&signer],
    )
    .await
    .unwrap_err();
    assert_custom(err, MarketTickError::InvalidInterval);

    send(
        &mut ctx,
        increment_ix(signer.pubkey(), 5, 1_000, 50),
        &[&signer],
    )
    .await
    .unwrap();

    let err = send(
        &mut ctx,
        increment_ix(signer.pubkey(), 5, 999, 50),
        &[&signer],
    )
    .await
    .unwrap_err();
    assert_custom(err, MarketTickError::NonMonotonic);

    let err = send(
        &mut ctx,
        increment_ix(signer.pubkey(), 5, 1_050, 51),
        &[&signer],
    )
    .await
    .unwrap_err();
    assert_custom(err, MarketTickError::IntervalMismatch);
}
