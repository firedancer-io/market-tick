use market_tick_abi::{
    header::{Header, CURRENT_VERSION, DISCRIMINATOR},
    read_header,
    v1::MarketTickV1,
    AbiError, PDA_SEEDS_V1,
};
use solana_pubkey::Pubkey;

fn encode(account: &MarketTickV1) -> Vec<u8> {
    let mut bytes = vec![0; MarketTickV1::LEN];
    account.encode(&mut bytes).unwrap();
    bytes
}

#[test]
fn discriminator_is_stable() {
    assert_eq!(DISCRIMINATOR.to_le_bytes(), *b"MRKTTICK");
}

#[test]
fn version_sizes_and_pda_seeds_are_stable() {
    assert_eq!(CURRENT_VERSION, 1);
    assert_eq!(MarketTickV1::VERSION, 1);
    assert_eq!(Header::LEN, 48);
    assert_eq!(MarketTickV1::LEN, 88);
    assert_eq!(PDA_SEEDS_V1, [b"market_tick".as_slice(), b"v1".as_slice()]);
    assert_eq!(MarketTickV1::new().header.version, MarketTickV1::VERSION);
}

#[test]
fn field_offsets_are_stable() {
    let mut account = MarketTickV1::new();
    account.header.signer = Pubkey::new_from_array([0x11; 32]);
    account.slot = 0x0102_0304_0506_0708;
    account.first_timestamp_ns = 0x1112_1314_1516_1718;
    account.timestamp_ns = 0x2122_2324_2526_2728;
    account.sequence = 0x3132_3334_3536_3738;
    account.target_market_tick_interval_ns = 0x4142_4344_4546_4748;

    let bytes = encode(&account);

    assert_eq!(&bytes[16..48], account.header.signer.as_ref());
    assert_eq!(&bytes[48..56], &account.slot.to_le_bytes());
    assert_eq!(&bytes[56..64], &account.first_timestamp_ns.to_le_bytes());
    assert_eq!(&bytes[64..72], &account.timestamp_ns.to_le_bytes());
    assert_eq!(&bytes[72..80], &account.sequence.to_le_bytes());
    assert_eq!(
        &bytes[80..88],
        &account.target_market_tick_interval_ns.to_le_bytes()
    );
}

#[test]
fn encoded_account_roundtrips() {
    let mut account = MarketTickV1::new();
    account.slot = 42;
    account.sequence = 7;

    let bytes = encode(&account);

    assert_eq!(MarketTickV1::decode(&bytes).unwrap(), account);
}

#[test]
fn from_bytes_mut_mutates_account_bytes_in_place() {
    let mut bytes = encode(&MarketTickV1::new());
    {
        let acct = MarketTickV1::from_bytes_mut(&mut bytes).unwrap();
        acct.slot = 42;
        acct.sequence = 7;
    }

    assert_eq!(MarketTickV1::from_bytes(&bytes).unwrap().slot, 42);
    assert_eq!(MarketTickV1::from_bytes(&bytes).unwrap().sequence, 7);
}

#[test]
fn rejects_invalid_discriminator() {
    let mut bytes = encode(&MarketTickV1::new());
    bytes[0] ^= 0xff;

    assert_eq!(read_header(&bytes), Err(AbiError::BadDiscriminator));
}

#[test]
fn rejects_unknown_version() {
    let mut account = MarketTickV1::new();
    account.header.version = 999;

    assert_eq!(
        MarketTickV1::decode(&encode(&account)),
        Err(AbiError::UnknownVersion(999))
    );
}

#[test]
fn rejects_account_smaller_than_header() {
    let bytes = vec![0; Header::LEN - 1];

    assert_eq!(read_header(&bytes), Err(AbiError::AccountTooSmall));
}
