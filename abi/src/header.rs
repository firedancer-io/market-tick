use crate::AbiError;
use bytemuck::{Pod, Zeroable};
use solana_pubkey::Pubkey;

pub const DISCRIMINATOR: u64 = u64::from_le_bytes(*b"MRKTTICK");
pub const CURRENT_VERSION: u16 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable)]
pub struct Header {
    pub discriminator: u64,
    pub version: u16,
    pub reserved: [u8; 6],
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
            reserved: [0; 6],
            signer: Pubkey::default(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.discriminator == DISCRIMINATOR
    }

    /// Owned decode (copy out of account bytes).
    pub fn decode(data: &[u8]) -> Result<Self, AbiError> {
        Ok(*Self::from_bytes(data)?)
    }

    /// Write this header into account bytes.
    pub fn encode(&self, data: &mut [u8]) -> Result<(), AbiError> {
        *cast_mut::<Self>(data)? = *self;
        Ok(())
    }

    /// Checked zerocopy view.
    pub fn from_bytes(data: &[u8]) -> Result<&Self, AbiError> {
        let header = cast_ref::<Self>(data)?;
        if !header.is_valid() {
            return Err(AbiError::BadDiscriminator);
        }
        Ok(header)
    }

    /// Checked zerocopy mutable view.
    pub fn from_bytes_mut(data: &mut [u8]) -> Result<&mut Self, AbiError> {
        let header = cast_mut::<Self>(data)?;
        if !header.is_valid() {
            return Err(AbiError::BadDiscriminator);
        }
        Ok(header)
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

const _: () = assert!(core::mem::size_of::<Header>() == Header::LEN);
const _: () = assert!(core::mem::align_of::<Header>() == 8);
