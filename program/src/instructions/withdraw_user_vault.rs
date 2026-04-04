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
      assert_spl_token_program, invoke_token_transfer_checked, load_user_vault, parse_u64_instruction_data, require_signer,
      verify_token_account,
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

   let vault_state = load_user_vault(program_id, user_vault_pda, owner.address(), app_address.address())?;

   verify_token_account(dest_ata, token_program, owner.address(), mint.address())?;
   verify_token_account(user_vault_ata, token_program, user_vault_pda.address(), mint.address())?;

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
      &signers,
   ).map_err(|e| {
      log!("withdraw_user_vault: transfer cpi failed");
      e
   })?;

   Ok(())
}
