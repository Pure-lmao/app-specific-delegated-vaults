//! App-only CPI: optional native transfer from the vault PDA plus optional SPL `TransferChecked` from
//! the vault ATA. Delegate signs; `instructions_sysvar` ensures the transaction's top-level ix is
//! from `app_address` (prevents arbitrary programs from calling this handler).
//!
//! Accounts (11):
//! 0. `delegate` (signer) — must match `UserVaultAccount::delegate`
//! 1. `owner` (readonly)
//! 2. `user_vault_pda` (writable if amount_native > 0)
//! 3. `user_vault_ata` (writable if amount > 0)
//! 4. `app_address` (readonly)
//! 5. `lamports_dest` (writable if amount_native > 0)
//! 6. `dest_ata` (writable if amount > 0)
//! 7. `mint` (readonly (can be dummy if amount == 0))
//! 8. `token_program` (readonly (can be dummy if amount == 0))
//! 9. `instructions_sysvar` (readonly) — `Sysvar1nstructions1111111111111111111111111`
//! 10. `system_program` (readonly)
//!
//! Data: `[discriminator (u8), amount_native (u64), amount (u64)]` — either amount may be zero; not both.

use crate::{
   constants::USER_VAULT_SEED,
   error::Error,
   helpers::{
      assert_spl_token_program, assert_system_program, invoke_token_transfer_checked, load_user_vault,
      parse_two_u64_instruction_data, require_delegate_not_expired, require_signer,
      require_top_level_instruction_is_app, transfer_lamports_from_user_vault_pda, verify_token_account,
      verify_token_account_mint,
   },
};
use pinocchio::{
   cpi::{Seed, Signer},
   error::ProgramError,
   AccountView, Address, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;

#[inline(never)]
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
   let (amount_native, amount) = parse_two_u64_instruction_data(data).map_err(|e| {
      log!("cpi_entry: invalid instruction data");
      e
   })?;

   if unlikely(amount == 0 && amount_native == 0) {
      log!("cpi_entry: amounts are zero");
      return Err(ProgramError::InvalidInstructionData);
   }

   let [
      delegate,
      owner,
      user_vault_pda,
      user_vault_ata,
      app_address,
      lamports_dest,
      dest_ata,
      mint,
      token_program,
      instructions_sysvar,
      system_program,
   ] = accounts else {
      log!("cpi_entry: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_top_level_instruction_is_app(instructions_sysvar, app_address)?;

   require_signer(delegate)?;

   let vault_state = load_user_vault(program_id, user_vault_pda, owner.address(), app_address.address())?;

   if unlikely(delegate.address().as_ref() != vault_state.delegate.as_ref()) {
      log!("cpi_entry: delegate does not match vault state");
      return Err(Error::InvalidUserVaultDelegate.into());
   }

   require_delegate_not_expired(vault_state.delegate_expires)?;

   if amount_native > 0 {
      assert_system_program(system_program)?;
      transfer_lamports_from_user_vault_pda(user_vault_pda, lamports_dest, amount_native).map_err(|e| {
         log!("cpi_entry: native lamport transfer failed");
         e
      })?;
   }

   if amount > 0 {
      assert_spl_token_program(token_program)?;
      verify_token_account(user_vault_ata, token_program, user_vault_pda.address(), mint.address())?;
      verify_token_account_mint(dest_ata, token_program, mint.address())?;
  
      let bump_seed = [vault_state.bump];
      let signer_seeds = [
         Seed::from(USER_VAULT_SEED),
         Seed::from(owner.address().as_ref()),
         Seed::from(app_address.address().as_ref()),
         Seed::from(&bump_seed[..]),
      ];
      let signers = [Signer::from(&signer_seeds[..])];

      invoke_token_transfer_checked(
         token_program, 
         mint, 
         user_vault_ata, 
         dest_ata, 
         user_vault_pda, 
         amount, 
         &signers
      ).map_err(|e| {
         log!("cpi_entry: transfer cpi failed");
         e
      })?;
   }

   Ok(())
}
