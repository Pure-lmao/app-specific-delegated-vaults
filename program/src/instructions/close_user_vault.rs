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
   helpers::{close_program_account_lamports_to, load_user_vault, require_signer},
};
use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult, hint::unlikely};
use pinocchio_log::log;

#[inline(never)]
pub fn process(program_id: &Address, accounts: &[AccountView]) -> ProgramResult {
   let [
      owner,
      user_vault_pda,
      app_address,
   ] = accounts else {
      log!("close_user_vault: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;

   let vault = load_user_vault(program_id, user_vault_pda, owner.address(), app_address.address())?;

   if unlikely(vault.ata_count != 0) {
      log!("close_user_vault: ata_count must be zero");
      return Err(Error::UserVaultHasOpenAtas.into());
   }

   close_program_account_lamports_to(user_vault_pda, owner)
}
