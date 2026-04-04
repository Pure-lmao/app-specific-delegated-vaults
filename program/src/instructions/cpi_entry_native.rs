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
//! 6. `system_program` (readonly)
//!
//! Data: `[discriminator (u8), amount_native (u64)]`

use crate::{
   error::Error,
   helpers::{
      assert_system_program, load_user_vault, parse_u64_instruction_data, require_delegate_not_expired, require_signer,
      require_top_level_instruction_is_app, transfer_lamports_from_user_vault_pda,
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
      system_program,
   ] = accounts else {
      log!("cpi_entry_native: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_top_level_instruction_is_app(instructions_sysvar, app_address)?;

   require_signer(delegate)?;
   assert_system_program(system_program)?;

   let vault_state = load_user_vault(program_id, user_vault_pda, owner.address(), app_address.address())?;

   if unlikely(delegate.address().as_ref() != vault_state.delegate.as_ref()) {
      log!("cpi_entry_native: delegate does not match vault state");
      return Err(Error::InvalidUserVaultDelegate.into());
   }

   require_delegate_not_expired(vault_state.delegate_expires)?;

   transfer_lamports_from_user_vault_pda(user_vault_pda, lamports_dest, amount_native).map_err(|e| {
      log!("cpi_entry_native: native lamport transfer failed");
      e
   })?;

   Ok(())
}
