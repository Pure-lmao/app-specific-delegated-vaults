//! On-chain `UserVaultAccount` layout (`program/src/state.rs`).

use solana_pubkey::Pubkey;

use crate::constants::USER_VAULT_DISCRIMINATOR;
use crate::error::VaultSdkError;

const PUBKEY_BYTES: usize = 32;

/// Decoded `UserVaultAccount` account data (includes leading discriminator as first field when unpacking from wire).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserVaultAccount {
   pub owner: Pubkey,
   pub app_address: Pubkey,
   pub delegate: Pubkey,
   pub delegate_expires: u32,
   pub ata_count: u16,
   pub bump: u8,
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
      let mut o = 1usize;
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
      o += PUBKEY_BYTES;
      let delegate_expires = u32::from_le_bytes(
         data[o..o + 4]
            .try_into()
            .map_err(|_| VaultSdkError::WrongAccountDataLen {
               expected: Self::LEN,
               got: data.len(),
            })?,
      );
      o += 4;
      let ata_count = u16::from_le_bytes(
         data[o..o + 2]
            .try_into()
            .map_err(|_| VaultSdkError::WrongAccountDataLen {
               expected: Self::LEN,
               got: data.len(),
            })?,
      );
      o += 2;
      let bump = data[o];
      Ok(Self {
         owner,
         app_address,
         delegate,
         delegate_expires,
         ata_count,
         bump,
      })
   }
}
