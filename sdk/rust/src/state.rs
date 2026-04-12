//! On-chain `UserVaultAccount` layout (`program/src/state.rs`).

use solana_pubkey::Pubkey;

use crate::constants::USER_VAULT_DISCRIMINATOR;
use crate::error::VaultSdkError;

const PUBKEY_BYTES: usize = 32;

/// Decoded `UserVaultAccount` account data (matches `#[repr(C)]` on-chain: discriminator, scalars, then pubkeys).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserVaultAccount {
   pub discriminator: u8,
   pub bump: u8,
   pub ata_count: u16,
   pub delegate_expires: u32,
   pub owner: Pubkey,
   pub app_address: Pubkey,
   pub delegate: Pubkey,
}

impl UserVaultAccount {
   /// On-chain `UserVaultAccount::LEN` (matches `program/src/state.rs`).
   pub const LEN: usize = 104;

   /// Parse raw account `data` (full [`Self::LEN`] bytes).
   pub fn unpack(data: &[u8]) -> Result<Self, VaultSdkError> {
      if data.len() != Self::LEN {
         return Err(VaultSdkError::WrongAccountDataLen {
            expected: Self::LEN,
            got: data.len(),
         });
      }
      if data[0] != USER_VAULT_DISCRIMINATOR {
         return Err(VaultSdkError::InvalidDiscriminator(data[0]));
      }
      let bump = data[1];
      let ata_count = u16::from_le_bytes(
         data[2..4]
            .try_into()
            .map_err(|_| VaultSdkError::WrongAccountDataLen {
               expected: Self::LEN,
               got: data.len(),
            })?,
      );
      let delegate_expires = u32::from_le_bytes(
         data[4..8]
            .try_into()
            .map_err(|_| VaultSdkError::WrongAccountDataLen {
               expected: Self::LEN,
               got: data.len(),
            })?,
      );
      let mut o = 8usize;
      let owner = Pubkey::new_from_array(
         data[o..o + PUBKEY_BYTES]
            .try_into()
            .map_err(|_| VaultSdkError::WrongAccountDataLen {
               expected: Self::LEN,
               got: data.len(),
            })?,
      );
      o += PUBKEY_BYTES;
      let app_address = Pubkey::new_from_array(
         data[o..o + PUBKEY_BYTES]
            .try_into()
            .map_err(|_| VaultSdkError::WrongAccountDataLen {
               expected: Self::LEN,
               got: data.len(),
            })?,
      );
      o += PUBKEY_BYTES;
      let delegate = Pubkey::new_from_array(
         data[o..o + PUBKEY_BYTES]
            .try_into()
            .map_err(|_| VaultSdkError::WrongAccountDataLen {
               expected: Self::LEN,
               got: data.len(),
            })?,
      );
      Ok(Self {
         discriminator: data[0],
         bump,
         ata_count,
         delegate_expires,
         owner,
         app_address,
         delegate,
      })
   }
}
