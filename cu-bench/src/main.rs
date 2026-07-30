use mollusk_svm::Mollusk;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use std::path::PathBuf;
use std::str::FromStr;

const PROGRAM_ID: &str = "bUD41ixzckBZ6bq2Zy1aUnNvnLF9vDuMq1AxjMVH25z";
const DISCRIMINATOR: &[u8; 8] = b"MRKTTICK";
const ACCOUNT_LEN: usize = 88;
const SLOT: u64 = 42;
const INTERVAL_NS: u64 = 50;

fn program_id() -> Pubkey {
    Pubkey::from_str(PROGRAM_ID).unwrap()
}

fn pda() -> Pubkey {
    Pubkey::find_program_address(&[b"market_tick", b"v1"], &program_id()).0
}

fn initialized_account(signer: &Pubkey, slot: u64, ts: i64, seq: u64, interval: u64) -> Account {
    let mut data = vec![0u8; ACCOUNT_LEN];
    data[0..8].copy_from_slice(DISCRIMINATOR);
    data[8..10].copy_from_slice(&1u16.to_le_bytes());
    data[16..48].copy_from_slice(signer.as_ref());
    data[48..56].copy_from_slice(&slot.to_le_bytes());
    data[56..64].copy_from_slice(&ts.to_le_bytes());
    data[64..72].copy_from_slice(&ts.to_le_bytes());
    data[72..80].copy_from_slice(&seq.to_le_bytes());
    data[80..88].copy_from_slice(&interval.to_le_bytes());
    Account {
        lamports: 1_000_000_000,
        data,
        owner: program_id(),
        executable: false,
        rent_epoch: 0,
    }
}

fn fresh_account() -> Account {
    initialized_account(&Pubkey::default(), 0, 0, 0, 0)
}

fn increment_ix(signer: Pubkey, slot: u64, timestamp_ns: i64, interval_ns: u64) -> Instruction {
    let mut data = vec![1u8];
    data.extend_from_slice(&slot.to_le_bytes());
    data.extend_from_slice(&timestamp_ns.to_le_bytes());
    data.extend_from_slice(&interval_ns.to_le_bytes());
    Instruction {
        program_id: program_id(),
        accounts: vec![
            AccountMeta::new(pda(), false),
            AccountMeta::new_readonly(signer, true),
        ],
        data,
    }
}

fn market_tick_so_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/deploy/market_tick")
}

fn p_tick_so_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/deploy/p_tick")
}

fn bench(name: &str, so_path: PathBuf) {
    let mut mollusk = Mollusk::new(&program_id(), so_path.to_str().unwrap());
    mollusk.warp_to_slot(SLOT);

    let signer = Pubkey::new_unique();

    let ix = increment_ix(signer, SLOT, 1_000, INTERVAL_NS);
    let accounts = [
        (pda(), fresh_account()),
        (signer, Account::default()),
    ];
    let result = mollusk.process_instruction(&ix, &accounts);
    assert!(
        result.raw_result.is_ok(),
        "{name} first crank failed: {:?}",
        result.raw_result
    );
    println!(
        "{name}/crank/first_in_slot: {} CU",
        result.compute_units_consumed
    );

    let ix = increment_ix(signer, SLOT, 1_050, INTERVAL_NS);
    let accounts = [
        (
            pda(),
            initialized_account(&signer, SLOT, 1_000, 0, INTERVAL_NS),
        ),
        (signer, Account::default()),
    ];
    let result = mollusk.process_instruction(&ix, &accounts);
    assert!(
        result.raw_result.is_ok(),
        "{name} subsequent crank failed: {:?}",
        result.raw_result
    );
    println!(
        "{name}/crank/subsequent: {} CU",
        result.compute_units_consumed
    );
}

fn main() {
    bench("market-tick", market_tick_so_path());
    bench("p-tick", p_tick_so_path());
}
