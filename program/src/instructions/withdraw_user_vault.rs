//! Owner-signed SPL withdrawal: transfer from the vault ATA to a destination ATA (same mint). The
//! destination must be an ATA owned by `owner` (enforced before CPI).
//!
//! Accounts (7):
//! 0. `owner` (writable signer)
//! 1. `user_vault_pda` (readonly)
//! 2. `user_vault_ata` (writable)
//! 3. `app_address` (readonly)
//! 4. `dest_ata` (writable)
//! 5. `mint` (readonly)
//! 6. `token_program` (readonly)
//!
//! Data: `[discriminator (u8), amount (u64)]`

use crate::{
   constants::USER_VAULT_SEED,
   helpers::{
      assert_spl_token_program, assert_user_vault_is_owned_by_program_and_correct_length, get_vault_bump, invoke_token_transfer_checked_with_decimals, mint_base_decimals, parse_u64_instruction_data, require_signer, verify_token_account, verify_vault_owner_and_app_address,
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
   let amount = parse_u64_instruction_data(data).map_err(|e| {
      log!("withdraw_user_vault: invalid instruction data");
      e
   })?;
   if unlikely(amount == 0) {
      log!("withdraw_user_vault: amount is zero");
      return Err(ProgramError::InvalidInstructionData);
   }

   let [
      owner,
      user_vault_pda,
      user_vault_ata,
      app_address,
      dest_ata,
      mint,
      token_program,
   ] = accounts else {
      log!("withdraw_user_vault: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;
   assert_spl_token_program(token_program)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   verify_token_account(dest_ata, token_program, owner.address(), mint.address())?;
   verify_vault_owner_and_app_address(user_vault_pda, owner.address(), app_address.address())?;
   let vault_bump = get_vault_bump(user_vault_pda);

   let decimals = mint_base_decimals(mint).map_err(|e| {
      log!("withdraw_user_vault: mint decimals read failed");
      e
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
      log!("withdraw_user_vault: transfer cpi failed");
      e
   })?;

   Ok(())
}
