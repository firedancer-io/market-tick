# Out Of Protocol Market Tick

A Solana program maintaining a sub-slot clock designed to increment at a
regular interval throughout a slot.

## Program IDs and upgrade authority

The Market Tick program ID on all clusters is

```
tickUcsEQegChaAuo9VYQQztB4ZGApY6ZT4FkULWY6N
```

Each public deployment will transfer its upgrade authority immediately
after initial deployment to a Squads v4 multisig with three independent
members and a two-member approval threshold. The Squads vault can replace
program behavior without changing the program ID after the required
approvals. Public releases will use both Anchor and `solana-verify`
verifiable-build workflows, publish the source revision and generated IDL,
and verify the deployed bytecode against the published build.

The canonical V1 PDA is `4cG31VNF9TzFinNc7BmnjhFvGjxkY3sCETVMtMgbrhPs`,
derived from seeds `[b"market_tick", b"v1"]`. Account versions are
independent and may coexist. Future versions may receive new instruction
tags and account layouts. Active versions are announced off-chain with
program releases, and the upgrade authority controls their introduction.

## Build and test

```sh
# Host-side ABI and layout tests (no validator required)
cargo test --workspace --lib
cargo test -p market-tick-abi

# Build the SBF program, deploy it into Surfpool, and run RPC integration tests
anchor test --validator surfpool
```

## Account ABI v1

| off | size | field | type |
|-----|------|-------|------|
| 0 | 8 | discriminator = `MRKTKV01` | bytes |
| 8 | 32 | signer | Pubkey |
| 40 | 8 | slot | u64 |
| 48 | 8 | first_timestamp_ns | i64 |
| 56 | 8 | timestamp_ns | i64 |
| 64 | 8 | sequence | u64 |
| 72 | 8 | target_market_tick_interval_ns | u64 |
| 80 | 8 | observed_market_tick_interval_ns | u64 |

The V1 account is exactly 88 bytes. Its version is encoded in the
version-specific discriminator rather than a separate field. Initialization
writes the discriminator and zeroes all state fields; a default `signer` means
that no tick has been accepted yet. The first valid increment completes the
state initialization.

## Instructions

All multi-byte integer fields are little-endian. Trailing instruction
data is accepted and ignored.

| Tag | Name | Accounts |
|-----|------|----------|
| `0` | Initialize V1 | `payer (signer, writable)`, `pda (writable)`, `system_program` |
| `1` | Increment V1 | `pda (writable)`, `signer (signer)` |

### Initialize V1 (1-byte)

| off | size | field | type |
|-----|------|-------|------|
| 0 | 1 | tag = 0 | u8 |

### Increment V1 (25-byte)

| off | size | field | type |
|-----|------|-------|------|
| 0 | 1 | tag = 1 | u8 |
| 1 | 8 | slot | u64 |
| 9 | 8 | timestamp_ns | i64 |
| 17 | 8 | target_market_tick_interval_ns | u64 |

`timestamp_ns` must be positive and `target_market_tick_interval_ns`
must be nonzero. The first increment in a slot is untrusted and records
the signer, sets `first_timestamp_ns` from `timestamp_ns`, and resets
`sequence` to zero. Later increments require the same signer and target
interval, require `timestamp_ns` to be strictly greater than the
preceding timestamp, set `observed_market_tick_interval_ns` to the difference
between the new and preceding timestamps, and increment `sequence` in-program.
The observed interval is zero before the first tick and on the first tick of
each slot.

## Trust model

The program is permissionless and uses a trust-first-writer model: the
first valid increment accepted in each slot selects that slot's signer.
The program does not verify that this signer is the scheduled block
producer or otherwise trusted. Consumers must independently decide
whether to trust the recorded signer, such as by applying an off-chain
signer whitelist.

Timestamps and target intervals are also signer-supplied. The program
requires the instruction's `slot` to equal the current runtime `Clock`
slot, and enforces positive timestamps, nonzero target intervals, and
strictly increasing same-slot timestamps. The recorded slot is therefore
runtime-verified, but the program does not authenticate wall-clock time,
enforce cadence, or impose application-specific bounds. Consumers must
apply any additional plausibility and range checks.

### Consumer validation example

Consumers should validate the account identity and owner before decoding,
then apply their own trust policy. For example:

```rust
use market_tick_abi::{
    AbiError, MarketTickV1, PDA_SEED_MARKET_TICK, PDA_SEED_VERSION_V1,
};
use solana_pubkey::Pubkey;

#[derive(Debug)]
enum ConsumerError {
    Abi(AbiError),
    InvalidPda,
    InvalidOwner,
    FutureSlot,
    Stale,
    UnauthorizedSigner,
    InvalidTimestamp,
    InvalidInterval,
}

impl From<AbiError> for ConsumerError {
    fn from(error: AbiError) -> Self {
        Self::Abi(error)
    }
}

fn decode_market_tick(
    program_id: &Pubkey,
    account_key: &Pubkey,
    account_owner: &Pubkey,
    data: &[u8],
    current_slot: u64,
    max_slot_age: u64,
    signer_whitelist: Option<&[[u8; 32]]>,
) -> Result<MarketTickV1, ConsumerError> {
    let expected_pda = Pubkey::find_program_address(
        &[PDA_SEED_MARKET_TICK, PDA_SEED_VERSION_V1],
        program_id,
    )
    .0;
    if expected_pda != *account_key {
        return Err(ConsumerError::InvalidPda);
    }
    if account_owner != program_id {
        return Err(ConsumerError::InvalidOwner);
    }

    let account = MarketTickV1::decode(data)?;
    if account.slot > current_slot {
        return Err(ConsumerError::FutureSlot);
    }
    if current_slot.saturating_sub(account.slot) > max_slot_age {
        return Err(ConsumerError::Stale);
    }
    if signer_whitelist.is_some_and(|allowed| !allowed.contains(&account.signer)) {
        return Err(ConsumerError::UnauthorizedSigner);
    }
    if account.first_timestamp_ns <= 0 || account.timestamp_ns <= 0 {
        return Err(ConsumerError::InvalidTimestamp);
    }
    if account.target_market_tick_interval_ns == 0 {
        return Err(ConsumerError::InvalidInterval);
    }

    Ok(account)
}
```
