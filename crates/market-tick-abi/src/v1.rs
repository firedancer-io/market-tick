use crate::{read_i64, read_u64, write_i64, write_u64, AbiError, Header};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketTickV1 {
    pub header: Header,
    pub slot: u64,
    pub first_timestamp_ns: i64,
    pub timestamp_ns: i64,
    pub sequence: u64,
    pub target_market_tick_interval_ns: u64,
}

impl MarketTickV1 {
    pub const VERSION: u16 = 1;
    pub const LEN: usize = 88;

    const OFF_SLOT: usize = Header::LEN;
    const OFF_FIRST_TIMESTAMP: usize = Self::OFF_SLOT + 8;
    const OFF_TIMESTAMP: usize = Self::OFF_FIRST_TIMESTAMP + 8;
    const OFF_SEQUENCE: usize = Self::OFF_TIMESTAMP + 8;
    const OFF_TARGET_INTERVAL: usize = Self::OFF_SEQUENCE + 8;

    pub fn new() -> Self {
        Self {
            header: Header::new(Self::VERSION),
            slot: 0,
            first_timestamp_ns: 0,
            timestamp_ns: 0,
            sequence: 0,
            target_market_tick_interval_ns: 0,
        }
    }

    pub fn decode(data: &[u8]) -> Result<Self, AbiError> {
        let header = Header::decode(data)?;
        if !header.is_valid() {
            return Err(AbiError::BadDiscriminator);
        }
        if header.version != Self::VERSION {
            return Err(AbiError::UnknownVersion(header.version));
        }
        if data.len() < Self::LEN {
            return Err(AbiError::AccountTooSmall);
        }
        Ok(Self {
            header,
            slot: read_u64(data, Self::OFF_SLOT),
            first_timestamp_ns: read_i64(data, Self::OFF_FIRST_TIMESTAMP),
            timestamp_ns: read_i64(data, Self::OFF_TIMESTAMP),
            sequence: read_u64(data, Self::OFF_SEQUENCE),
            target_market_tick_interval_ns: read_u64(data, Self::OFF_TARGET_INTERVAL),
        })
    }

    pub fn encode(&self, data: &mut [u8]) -> Result<(), AbiError> {
        if data.len() < Self::LEN {
            return Err(AbiError::AccountTooSmall);
        }
        self.header.encode(data)?;
        write_u64(data, Self::OFF_SLOT, self.slot);
        write_i64(data, Self::OFF_FIRST_TIMESTAMP, self.first_timestamp_ns);
        write_i64(data, Self::OFF_TIMESTAMP, self.timestamp_ns);
        write_u64(data, Self::OFF_SEQUENCE, self.sequence);
        write_u64(
            data,
            Self::OFF_TARGET_INTERVAL,
            self.target_market_tick_interval_ns,
        );
        Ok(())
    }
}

impl Default for MarketTickV1 {
    fn default() -> Self {
        Self::new()
    }
}
