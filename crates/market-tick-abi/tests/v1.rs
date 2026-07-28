use market_tick_abi::{
    error_code, AbiError, MarketTickV1, DISCRIMINATOR_V1, PDA_SEED_MARKET_TICK, PDA_SEED_VERSION_V1,
};

fn encode(account: &MarketTickV1) -> Vec<u8> {
    let mut bytes = vec![0; MarketTickV1::LEN];
    account.encode(&mut bytes).unwrap();
    bytes
}

#[test]
fn discriminator_is_stable() {
    assert_eq!(DISCRIMINATOR_V1, *b"MRKTKV01");
}

#[test]
fn size_and_pda_seeds_are_stable() {
    assert_eq!(MarketTickV1::LEN, 88);
    assert_eq!(PDA_SEED_MARKET_TICK, b"market_tick");
    assert_eq!(PDA_SEED_VERSION_V1, b"v1");
}

#[test]
fn default_state_has_no_tick() {
    let account = MarketTickV1::default();
    assert_eq!(account.signer, [0; 32]);
    assert_eq!(account.observed_market_tick_interval_ns, 0);
}

#[test]
fn field_offsets_are_stable() {
    let mut account = MarketTickV1::default();
    account.signer = [0x11; 32];
    account.slot = 0x0102_0304_0506_0708;
    account.first_timestamp_ns = 0x1112_1314_1516_1718;
    account.timestamp_ns = 0x2122_2324_2526_2728;
    account.sequence = 0x3132_3334_3536_3738;
    account.target_market_tick_interval_ns = 0x4142_4344_4546_4748;
    account.observed_market_tick_interval_ns = 0x5152_5354_5556_5758;

    let bytes = encode(&account);

    assert_eq!(&bytes[0..8], &DISCRIMINATOR_V1);
    assert_eq!(&bytes[8..40], &account.signer);
    assert_eq!(&bytes[40..48], &account.slot.to_le_bytes());
    assert_eq!(&bytes[48..56], &account.first_timestamp_ns.to_le_bytes());
    assert_eq!(&bytes[56..64], &account.timestamp_ns.to_le_bytes());
    assert_eq!(&bytes[64..72], &account.sequence.to_le_bytes());
    assert_eq!(
        &bytes[72..80],
        &account.target_market_tick_interval_ns.to_le_bytes()
    );
    assert_eq!(
        &bytes[80..88],
        &account.observed_market_tick_interval_ns.to_le_bytes()
    );
}

#[test]
fn encoded_account_roundtrips() {
    let mut account = MarketTickV1::default();
    account.slot = 42;
    account.sequence = 7;

    let bytes = encode(&account);

    assert_eq!(MarketTickV1::decode(&bytes).unwrap(), account);
}

#[test]
fn rejects_invalid_discriminator() {
    let mut bytes = encode(&MarketTickV1::default());
    bytes[0] ^= 0xff;

    assert_eq!(
        MarketTickV1::decode(&bytes),
        Err(AbiError::BadDiscriminator)
    );
}

#[test]
fn rejects_inexact_account_lengths() {
    let short = vec![0; MarketTickV1::LEN - 1];
    let long = vec![0; MarketTickV1::LEN + 1];

    assert_eq!(
        MarketTickV1::decode(&short),
        Err(AbiError::InvalidAccountLength)
    );
    assert_eq!(
        MarketTickV1::decode(&long),
        Err(AbiError::InvalidAccountLength)
    );
}

#[test]
fn program_error_numbers_are_stable() {
    assert_eq!(error_code::SLOT_MISMATCH, 6000);
    assert_eq!(error_code::NOT_SLOT_SIGNER, 6001);
    assert_eq!(error_code::NON_MONOTONIC, 6002);
    assert_eq!(error_code::INTERVAL_MISMATCH, 6003);
    assert_eq!(error_code::COUNTER_OVERFLOW, 6004);
    assert_eq!(error_code::INVALID_TIMESTAMP, 6005);
    assert_eq!(error_code::INVALID_INTERVAL, 6006);
}
