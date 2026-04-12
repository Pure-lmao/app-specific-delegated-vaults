//! User vault PDA (`["vault", owner, app_address]`): metadata for delegate-authorized spends from the user vault ATA.

use core::mem::{size_of, MaybeUninit};

use pinocchio::{Address, error::ProgramError};

use crate::constants::USER_VAULT_DISCRIMINATOR;

pub const ADDRESS_BYTES: usize = 32;

/// On-wire layout (aligned): discriminator, bump, `ata_count`, `delegate_expires`, then three pubkeys.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserVaultAccount {
   pub discriminator: u8,
   pub bump: u8,
   pub ata_count: u16,
   pub delegate_expires: u32,
   pub owner: Address,
   pub app_address: Address,
   pub delegate: Address,
}

impl UserVaultAccount {
   pub const LEN: usize = size_of::<Self>();

   #[inline(never)]
   pub fn pack(&self, dst: &mut [u8]) -> Result<(), ProgramError> {
      if dst.len() != Self::LEN {
         return Err(ProgramError::InvalidAccountData);
      }
      unsafe {
         core::ptr::copy_nonoverlapping(
            self as *const Self as *const u8,
            dst.as_mut_ptr(),
            Self::LEN,
         );
      }
      Ok(())
   }

   #[inline(never)]
   pub fn unpack(src: &[u8]) -> Result<Self, ProgramError> {
      let mut buf: MaybeUninit<AlignedWire> = MaybeUninit::uninit();
      unsafe {
         core::ptr::copy_nonoverlapping(
            src.as_ptr(),
            buf.as_mut_ptr() as *mut u8,
            Self::LEN,
         );
      }
      let s = unsafe { core::ptr::read(buf.as_ptr() as *const Self) };
      if s.discriminator != USER_VAULT_DISCRIMINATOR {
         return Err(ProgramError::InvalidAccountData);
      }
      Ok(s)
   }
}

#[repr(C, align(8))]
struct AlignedWire([u8; UserVaultAccount::LEN]);

