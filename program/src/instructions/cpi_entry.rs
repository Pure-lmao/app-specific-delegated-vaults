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
//! 10. `clock_sysvar` (readonly) — `SysvarC1ock11111111111111111111111111111111`
//!
//! Data: `[discriminator (u8), amount_native (u64), amount (u64)]` — either amount may be zero; not both.

use crate::{
   constants::USER_VAULT_SEED,
   helpers::{
      assert_user_vault_is_owned_by_program_and_correct_length,
      invoke_token_transfer_checked_with_decimals, read_mint_decimals,
      parse_two_u64_instruction_data, require_signer,
      require_top_level_instruction_is_app, transfer_lamports_from_user_vault_pda,
      verify_delegate_authority_return_bump,
   },
};
use pinocchio::{
   cpi::{Seed, Signer},
   error::ProgramError,
   AccountView, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;

#[inline(never)]
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
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
      clock_sysvar,
   ] = accounts else {
      log!("cpi_entry: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_top_level_instruction_is_app(instructions_sysvar, app_address)?;
   require_signer(delegate)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   let vault_bump = verify_delegate_authority_return_bump(
      user_vault_pda,
      owner.address(),
      app_address.address(),
      delegate.address(),
      clock_sysvar,
   )?;

   if amount_native > 0 {
      transfer_lamports_from_user_vault_pda(user_vault_pda, lamports_dest, amount_native).map_err(|e| {
         log!("cpi_entry: native lamport transfer failed");
         e
      })?;
   }

   if amount > 0 {
      let decimals = read_mint_decimals(mint).map_err(|e| {
         log!("cpi_entry: mint verification failed");
         ProgramError::from(e)
      })?;

      let bump_seed = [vault_bump];
      let signer_seeds = [
         Seed::from(USER_VAULT_SEED),
         Seed::from(owner.address().as_ref()),
         Seed::from(app_address.address().as_ref()),
         Seed::from(&bump_seed[..]),
      ];
      let signers = [Signer::from(&signer_seeds[..])];

      invoke_token_transfer_checked_with_decimals(
         token_program,
         mint,
         user_vault_ata,
         dest_ata,
         user_vault_pda,
         amount,
         &signers,
         decimals,
      )
      .map_err(|e| {
         log!("cpi_entry: transfer cpi failed");
         e
      })?;
   }

   Ok(())
}
