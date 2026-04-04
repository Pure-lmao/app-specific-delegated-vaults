//! Owner-signed native withdrawal: debit lamports held on the vault PDA (e.g. rent headroom) to the
//! owner's wallet. Does not close the vault account.
//!
//! Accounts (4):
//! 0. `owner` (writable signer) — receives lamports
//! 1. `user_vault_pda` (writable) — vault PDA (program-owned; lamports debited)
//! 2. `app_address` (readonly)
//! 3. `system_program` (readonly)
//!
//! Data: `[discriminator (u8), amount (u64)]`

use crate::{
   helpers::{
      assert_system_program, load_user_vault, parse_u64_instruction_data, require_signer, transfer_lamports_from_user_vault_pda,
   },
};
use pinocchio::{
   error::ProgramError,
   AccountView, Address, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;

#[inline(never)]
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
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
      system_program,
   ] = accounts else {
      log!("withdraw_user_vault_native: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;
   assert_system_program(system_program)?;

   load_user_vault(program_id, user_vault_pda, owner.address(), app_address.address())?;

   transfer_lamports_from_user_vault_pda(
      user_vault_pda,
      owner,
      amount,
   )?;

   Ok(())
}
