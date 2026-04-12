//! Close the user vault PDA and return rent to the owner. Requires `ata_count == 0` (every vault ATA
//! must already be closed with the CloseVaultAta instruction).
//!
//! Accounts (3):
//! 0. `owner` (writable signer)
//! 1. `user_vault_pda` (writable)
//! 2. `app_address` (readonly)
//!
//! Data: `[discriminator (u8)]`

use crate::{
   error::Error,
   helpers::{
      assert_user_vault_is_owned_by_program_and_correct_length, close_program_account_lamports_to,
      get_vault_ata_count, require_signer, verify_vault_owner_and_app_address,
   },
};
use pinocchio::{error::ProgramError, AccountView, ProgramResult, hint::unlikely};
use pinocchio_log::log;

#[inline(never)]
pub fn process(accounts: &mut [AccountView]) -> ProgramResult {
   let [
      owner,
      user_vault_pda,
      app_address,
   ] = accounts else {
      log!("close_user_vault: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   verify_vault_owner_and_app_address(user_vault_pda, owner.address(), app_address.address())?;

   if unlikely(get_vault_ata_count(user_vault_pda) != 0) {
      log!("close_user_vault: ata_count must be zero");
      return Err(Error::UserVaultHasOpenAtas.into());
   }

   close_program_account_lamports_to(user_vault_pda, owner)
}
