use std::io::Write;

use anchor_lang::prelude::{
    AccountDeserialize, AccountSerialize, Discriminator, ErrorCode, Owner, Pubkey, Result,
};
use market_tick_abi::{MarketTickV1, DISCRIMINATOR_V1};

/// Anchor account adapter for the framework-independent V1 ABI.
#[derive(Clone, Debug)]
pub struct MarketTickV1Account(pub MarketTickV1);

impl MarketTickV1Account {
    /// Exact number of bytes allocated for the V1 account.
    pub const LEN: usize = MarketTickV1::LEN;
}

impl AccountSerialize for MarketTickV1Account {
    fn try_serialize<W: Write>(&self, writer: &mut W) -> Result<()> {
        let mut data = [0u8; Self::LEN];
        self.0
            .encode(&mut data)
            .map_err(|_| ErrorCode::AccountDidNotSerialize)?;
        writer
            .write_all(&data)
            .map_err(|_| ErrorCode::AccountDidNotSerialize.into())
    }
}

impl AccountDeserialize for MarketTickV1Account {
    fn try_deserialize(data: &mut &[u8]) -> Result<Self> {
        MarketTickV1::decode(data)
            .map(Self)
            .map_err(|_| ErrorCode::AccountDidNotDeserialize.into())
    }

    fn try_deserialize_unchecked(data: &mut &[u8]) -> Result<Self> {
        if data.len() != Self::LEN {
            return Err(ErrorCode::AccountDidNotDeserialize.into());
        }

        let mut normalized = [0u8; Self::LEN];
        normalized.copy_from_slice(data);
        normalized[..DISCRIMINATOR_V1.len()].copy_from_slice(&DISCRIMINATOR_V1);
        MarketTickV1::decode(&normalized)
            .map(Self)
            .map_err(|_| ErrorCode::AccountDidNotDeserialize.into())
    }
}

impl Discriminator for MarketTickV1Account {
    const DISCRIMINATOR: &'static [u8] = &DISCRIMINATOR_V1;
}

impl Owner for MarketTickV1Account {
    fn owner() -> Pubkey {
        crate::ID
    }
}

#[cfg(feature = "idl-build")]
impl anchor_lang::IdlBuild for MarketTickV1Account {
    fn create_type() -> Option<anchor_lang::idl::types::IdlTypeDef> {
        use anchor_lang::idl::types::{
            IdlDefinedFields, IdlField, IdlSerialization, IdlType, IdlTypeDef, IdlTypeDefTy,
        };

        let field = |name: &str, ty| IdlField {
            name: name.into(),
            docs: vec![],
            ty,
        };

        Some(IdlTypeDef {
            name: "MarketTickV1Account".into(),
            docs: vec!["Fixed-layout V1 Market Tick account.".into()],
            serialization: IdlSerialization::Custom("market-tick-v1".into()),
            repr: None,
            generics: vec![],
            ty: IdlTypeDefTy::Struct {
                fields: Some(IdlDefinedFields::Named(vec![
                    field("signer", IdlType::Pubkey),
                    field("slot", IdlType::U64),
                    field("first_timestamp_ns", IdlType::I64),
                    field("timestamp_ns", IdlType::I64),
                    field("sequence", IdlType::U64),
                    field("target_market_tick_interval_ns", IdlType::U64),
                    field("observed_market_tick_interval_ns", IdlType::U64),
                ])),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_preserves_the_v1_account_bytes() {
        let mut expected = MarketTickV1::default();
        expected.signer = [0x11; 32];
        expected.slot = 42;
        expected.timestamp_ns = 123;

        let mut direct = [0u8; MarketTickV1::LEN];
        expected.encode(&mut direct).unwrap();

        let mut through_anchor = Vec::new();
        MarketTickV1Account(expected)
            .try_serialize(&mut through_anchor)
            .unwrap();

        assert_eq!(through_anchor, direct);
        let mut encoded = through_anchor.as_slice();
        assert_eq!(
            MarketTickV1Account::try_deserialize(&mut encoded)
                .unwrap()
                .0,
            expected
        );
    }

    #[test]
    fn unchecked_zero_account_becomes_default_v1_state() {
        let zeroes = [0u8; MarketTickV1::LEN];
        let mut data = zeroes.as_slice();
        let account = MarketTickV1Account::try_deserialize_unchecked(&mut data).unwrap();

        assert_eq!(account.0, MarketTickV1::default());

        let mut encoded = Vec::new();
        account.try_serialize(&mut encoded).unwrap();
        assert_eq!(&encoded[..DISCRIMINATOR_V1.len()], &DISCRIMINATOR_V1);
        assert_eq!(
            MarketTickV1::decode(&encoded).unwrap(),
            MarketTickV1::default()
        );
    }

    #[test]
    fn unchecked_deserialization_ignores_discriminator_but_decodes_fields() {
        let expected = MarketTickV1 {
            signer: [0x22; 32],
            slot: 42,
            first_timestamp_ns: 100,
            timestamp_ns: 200,
            sequence: 3,
            target_market_tick_interval_ns: 10,
            observed_market_tick_interval_ns: 100,
        };
        let mut data = [0u8; MarketTickV1::LEN];
        expected.encode(&mut data).unwrap();
        data[..DISCRIMINATOR_V1.len()].fill(0xff);

        let account = MarketTickV1Account::try_deserialize_unchecked(&mut data.as_slice()).unwrap();

        assert_eq!(account.0, expected);
    }

    #[test]
    fn unchecked_deserialization_rejects_wrong_length_data() {
        let short = [0u8; MarketTickV1::LEN - 1];
        assert!(MarketTickV1Account::try_deserialize_unchecked(&mut short.as_slice()).is_err());
    }
}
