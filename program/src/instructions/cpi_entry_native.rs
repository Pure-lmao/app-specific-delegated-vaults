//! App-only CPI: native lamports from the vault PDA to `lamports_dest`. Delegate signs; top-level ix
//! must be from `app_address` (see `instructions_sysvar`).
//!
//! Accounts (7):
//! 0. `delegate` (signer) — must match `UserVaultAccount::delegate`
//! 1. `owner` (readonly)
//! 2. `user_vault_pda` (writable)
//! 3. `app_address` (readonly)
//! 4. `lamports_dest` (writable)
//! 5. `instructions_sysvar` (readonly) — `Sysvar1nstructions1111111111111111111111111`
//! 6. `clock_sysvar` (readonly) — `SysvarC1ock11111111111111111111111111111111`
//!
//! Data: `[discriminator (u8), amount_native (u64)]`

use crate::helpers::{
   assert_user_vault_is_owned_by_program_and_correct_length, get_vault_delegate_expires,
   parse_u64_instruction_data, require_delegate_not_expired, require_signer,
   require_top_level_instruction_is_app, transfer_lamports_from_user_vault_pda,
   verify_vault_delegate, verify_vault_owner_and_app_address,
};
use pinocchio::{
   error::ProgramError,
   AccountView, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;

#[inline(never)]
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let amount_native = parse_u64_instruction_data(data).map_err(|e| {
      log!("cpi_entry_native: invalid instruction data");
      e
   })?;

   if unlikely(amount_native == 0) {
      log!("cpi_entry_native: amount is zero");
      return Err(ProgramError::InvalidInstructionData);
   }

   let [
      delegate,
      owner,
      user_vault_pda,
      app_address,
      lamports_dest,
      instructions_sysvar,
      clock_sysvar,
   ] = accounts else {
      log!("cpi_entry_native: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_top_level_instruction_is_app(instructions_sysvar, app_address)?;
   require_signer(delegate)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   verify_vault_owner_and_app_address(user_vault_pda, owner.address(), app_address.address())?;
   verify_vault_delegate(user_vault_pda, delegate.address())?;
   require_delegate_not_expired(
      get_vault_delegate_expires(user_vault_pda),
      clock_sysvar
   )?;

   transfer_lamports_from_user_vault_pda(user_vault_pda, lamports_dest, amount_native).map_err(|e| {
      log!("cpi_entry_native: native lamport transfer failed");
      e
   })?;

   Ok(())
}
