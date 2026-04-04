//! User vault PDA (`["vault", owner, app_address]`): metadata for delegate-authorized spends from the user vault ATA.

use pinocchio::{error::ProgramError, Address};

use crate::constants::USER_VAULT_DISCRIMINATOR;

pub const ADDRESS_BYTES: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserVaultAccount {
   pub owner: Address,
   pub app_address: Address,
   pub delegate: Address,
   pub delegate_expires: u32,
   pub ata_count: u16,
   pub bump: u8,
}

impl UserVaultAccount {
   pub const LEN: usize = 1 + ADDRESS_BYTES + ADDRESS_BYTES + ADDRESS_BYTES + 4 + 2 + 1;

   #[inline(never)]
   pub fn pack(&self, dst: &mut [u8]) -> Result<(), ProgramError> {
      if dst.len() != Self::LEN {
         return Err(ProgramError::InvalidAccountData);
      }
      dst[0] = USER_VAULT_DISCRIMINATOR;
      let mut o = 1;
      dst[o..o + ADDRESS_BYTES].copy_from_slice(self.owner.as_ref());
      o += ADDRESS_BYTES;
      dst[o..o + ADDRESS_BYTES].copy_from_slice(self.app_address.as_ref());
      o += ADDRESS_BYTES;
      dst[o..o + ADDRESS_BYTES].copy_from_slice(self.delegate.as_ref());
      o += ADDRESS_BYTES;
      dst[o..o + 4].copy_from_slice(&self.delegate_expires.to_le_bytes());
      o += 4;
      dst[o..o + 2].copy_from_slice(&self.ata_count.to_le_bytes());
      o += 2;
      dst[o] = self.bump;
      Ok(())
   }

   #[inline(never)]
   pub fn unpack(src: &[u8]) -> Result<Self, ProgramError> {
      if src.len() != Self::LEN {
         return Err(ProgramError::InvalidAccountData);
      }
      if src[0] != USER_VAULT_DISCRIMINATOR {
         return Err(ProgramError::InvalidAccountData);
      }
      let o = 1;
      let owner = Address::new_from_array(
         src[o..o + ADDRESS_BYTES].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
      );
      let o = o + ADDRESS_BYTES;
      let app_address = Address::new_from_array(
         src[o..o + ADDRESS_BYTES].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
      );
      let o = o + ADDRESS_BYTES;
      let delegate = Address::new_from_array(
         src[o..o + ADDRESS_BYTES].try_into().map_err(|_| ProgramError::InvalidAccountData)?,
      );
      let o = o + ADDRESS_BYTES;
      let delegate_expires = u32::from_le_bytes(src[o..o + 4].try_into().map_err(|_| ProgramError::InvalidAccountData)?);
      let o = o + 4;
      let ata_count = u16::from_le_bytes(src[o..o + 2].try_into().map_err(|_| ProgramError::InvalidAccountData)?);
      let o = o + 2;
      let bump = u8::from_le_bytes(src[o..o + 1].try_into().map_err(|_| ProgramError::InvalidAccountData)?);
      Ok(Self { owner, app_address, delegate, delegate_expires, ata_count, bump })
   }
}
