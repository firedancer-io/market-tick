# Out Of Protocol Market Tick

A Solana program maintaining a sub-slot clock which block producers
increment at a regular interval throughout a slot.

The program is permissionless and uses a trust-first-writer model: the
first valid increment accepted in each slot selects that slot's
publisher. The program does not verify that this publisher is the
scheduled block producer or otherwise trusted. Consumers must
independently decide whether to trust the recorded publisher, such as by
applying an off-chain publisher whitelist.

## Program ID

`bUD41ixzckBZ6bq2Zy1aUnNvnLF9vDuMq1AxjMVH25z` (local testing only)

This keypair is at `market-tick/tests/fixtures/keys/local.json` to make
and tests reproducible.

The V1 PDA uses seeds `[b"market_tick", b"v1"]`.

Account versions are independent and may coexist. Future versions may
receive new instruction tags and account layouts. Active versions are
announced off-chain with program releases. The program upgrade authority
controls the introduction of future versions.

## Build and test

The project requires Rust 1.89 or newer, Anchor CLI 1.1.2, the Solana 3.1.12
build toolchain, and Surfpool 1.5.x. `Anchor.toml` pins the Anchor and Solana
versions and configures an offline, clock-driven Surfpool network.

```sh
# Install the shared host-local program keypair where Anchor expects it
./scripts/setup-tests.sh

# Host-side ABI and layout tests (no validator required)
cargo test --workspace --lib
cargo test -p market-tick-abi

# Build the SBF program, deploy it into Surfpool, and run RPC integration tests
anchor test --validator surfpool
```

## Account ABI v1

| off | size | field | type |
|-----|------|-------|------|
| 0 | 8 | discriminator | u64 |
| 8 | 2 | version | u16 |
| 10 | 6 | reserved | bytes |
| 16 | 32 | signer | Pubkey |
| 48 | 8 | slot | u64 |
| 56 | 8 | first_timestamp_ns | i64 |
| 64 | 8 | timestamp_ns | i64 |
| 72 | 8 | sequence | u64 |
| 80 | 8 | target_market_tick_interval_ns | u64 |

## Instructions

All multi-byte integer fields are little-endian.

| Tag | Name | Accounts |
|-----|------|----------|
| `0` | Initialize V1 | `payer (signer, writable)`, `v1_pda (writable)`, `system_program` |
| `1` | Increment V1 | `v1_pda (writable)`, `signer (signer)` |

### Initialize V1 (1 byte)

| off | size | field | type |
|-----|------|-------|------|
| 0 | 1 | tag = 0 | u8 |

### Increment V1 (25 bytes)

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
preceding timestamp, and increment `sequence` in-program.

## Consumer validation example

Consumers should validate the account identity and owner before decoding,
then apply their own trust policy. For example:

```rust
use market_tick_abi::{find_v1_pda, AbiError, MarketTickV1};
use solana_pubkey::Pubkey;

#[derive(Debug)]
enum ConsumerError {
    Abi(AbiError),
    InvalidPda,
    InvalidOwner,
    FutureSlot,
    Stale,
    UnauthorizedPublisher,
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
    publisher_whitelist: Option<&[Pubkey]>,
) -> Result<MarketTickV1, ConsumerError> {
    if find_v1_pda(program_id).0 != *account_key {
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
    if publisher_whitelist
        .is_some_and(|allowed| !allowed.contains(&account.header.signer))
    {
        return Err(ConsumerError::UnauthorizedPublisher);
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

Applications may need stricter timestamp or interval checks. All
timestamps remain publisher supplied and are not authenticated
wall-clock time.
