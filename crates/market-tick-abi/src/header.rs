use crate::{read_u16, read_u64, write_u16, write_u64, AbiError};
use solana_pubkey::Pubkey;

pub const DISCRIMINATOR: u64 = u64::from_le_bytes(*b"MRKTTICK");
pub const CURRENT_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub discriminator: u64,
    pub version: u16,
    pub signer: Pubkey,
}

impl Header {
    pub const LEN: usize = 48;
    pub const OFF_DISCRIMINATOR: usize = 0;
    pub const OFF_VERSION: usize = 8;
    pub const OFF_SIGNER: usize = 16;

    pub fn new(version: u16) -> Self {
        Self {
            discriminator: DISCRIMINATOR,
            version,
            signer: Pubkey::default(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.discriminator == DISCRIMINATOR
    }

    pub fn decode(data: &[u8]) -> Result<Self, AbiError> {
        if data.len() < Self::LEN {
            return Err(AbiError::AccountTooSmall);
        }
        let mut signer = [0u8; 32];
        signer.copy_from_slice(&data[Self::OFF_SIGNER..Self::OFF_SIGNER + 32]);
        Ok(Self {
            discriminator: read_u64(data, Self::OFF_DISCRIMINATOR),
            version: read_u16(data, Self::OFF_VERSION),
            signer: Pubkey::new_from_array(signer),
        })
    }

    pub fn encode(&self, data: &mut [u8]) -> Result<(), AbiError> {
        if data.len() < Self::LEN {
            return Err(AbiError::AccountTooSmall);
        }
        write_u64(data, Self::OFF_DISCRIMINATOR, self.discriminator);
        write_u16(data, Self::OFF_VERSION, self.version);
        data[Self::OFF_VERSION + 2..Self::OFF_SIGNER].fill(0);
        data[Self::OFF_SIGNER..Self::OFF_SIGNER + 32].copy_from_slice(self.signer.as_ref());
        Ok(())
    }
}
