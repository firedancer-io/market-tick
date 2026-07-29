use crate::{AbiError, Header};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
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

    pub const OFF_SLOT: usize = Header::LEN;
    pub const OFF_FIRST_TIMESTAMP: usize = Self::OFF_SLOT + 8;
    pub const OFF_TIMESTAMP: usize = Self::OFF_FIRST_TIMESTAMP + 8;
    pub const OFF_SEQUENCE: usize = Self::OFF_TIMESTAMP + 8;
    pub const OFF_TARGET_INTERVAL: usize = Self::OFF_SEQUENCE + 8;

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

    /// Owned decode (copy out of account bytes).
    pub fn decode(data: &[u8]) -> Result<Self, AbiError> {
        Ok(*Self::from_bytes(data)?)
    }

    /// Write this account into account bytes.
    pub fn encode(&self, data: &mut [u8]) -> Result<(), AbiError> {
        *cast_mut::<Self>(data)? = *self;
        Ok(())
    }

    /// Checked zerocopy view. Validates length, alignment, discriminator, and version.
    pub fn from_bytes(data: &[u8]) -> Result<&Self, AbiError> {
        let acct = cast_ref::<Self>(data)?;
        Self::check_header(&acct.header)?;
        Ok(acct)
    }

    /// Checked zerocopy mutable view. Validates length, alignment, discriminator, and version.
    pub fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, AbiError> {
        let acct = cast_mut::<Self>(data)?;
        Self::check_header(&acct.header)?;
        Ok(acct)
    }

    fn check_header(header: &Header) -> Result<(), AbiError> {
        if !header.is_valid() {
            return Err(AbiError::BadDiscriminator);
        }
        if header.version != Self::VERSION {
            return Err(AbiError::UnknownVersion(header.version));
        }
        Ok(())
    }
}

impl Default for MarketTickV1 {
    fn default() -> Self {
        Self::new()
    }
}

fn cast_ref<T: Pod>(data: &[u8]) -> Result<&T, AbiError> {
    let data = data
        .get(..core::mem::size_of::<T>())
        .ok_or(AbiError::AccountTooSmall)?;
    bytemuck::try_from_bytes(data).map_err(|_| AbiError::AccountTooSmall)
}

fn cast_mut<T: Pod>(data: &mut [u8]) -> Result<&mut T, AbiError> {
    let data = data
        .get_mut(..core::mem::size_of::<T>())
        .ok_or(AbiError::AccountTooSmall)?;
    bytemuck::try_from_bytes_mut(data).map_err(|_| AbiError::AccountTooSmall)
}

const _: () = assert!(core::mem::size_of::<MarketTickV1>() == MarketTickV1::LEN);
const _: () = assert!(core::mem::align_of::<MarketTickV1>() == 8);
