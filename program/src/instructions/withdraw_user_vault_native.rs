//! Owner-signed native withdrawal: debit lamports held on the vault PDA (e.g. rent headroom) to the
//! owner's wallet. Does not close the vault account.
//!
//! Accounts (3):
//! 0. `owner` (writable signer) — receives lamports
//! 1. `user_vault_pda` (writable) — vault PDA (program-owned; lamports debited)
//! 2. `app_address` (readonly)
//!
//! Data: `[discriminator (u8), amount (u64)]`

use crate::helpers::{
   assert_user_vault_is_owned_by_program_and_correct_length, parse_u64_instruction_data, require_signer,
   transfer_lamports_from_user_vault_pda, verify_vault_owner_and_app_address,
};
use pinocchio::{
   error::ProgramError,
   AccountView, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;

#[inline(never)]
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let amount = parse_u64_instruction_data(data).map_err(|e| {
      log!("withdraw_user_vault_native: invalid instruction data");
      e
   })?;
   if unlikely(amount == 0) {
      log!("withdraw_user_vault_native: amount is zero");
      return Err(ProgramError::InvalidInstructionData);
   }

   let [
      owner,
      user_vault_pda,
      app_address,
   ] = accounts else {
      log!("withdraw_user_vault_native: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   verify_vault_owner_and_app_address(user_vault_pda, owner.address(), app_address.address())?;

   transfer_lamports_from_user_vault_pda(
      user_vault_pda,
      owner,
      amount,
   )?;

   Ok(())
}
