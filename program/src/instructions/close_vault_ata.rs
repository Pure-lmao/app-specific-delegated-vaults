//! Owner-signed SPL `CloseAccount` on one vault ATA (balance must be zero). Decrements `ata_count` on
//! the vault PDA; authority for close is the vault PDA (CPI with seeds).
//!
//! Accounts (7):
//! 0. `owner` (writable signer)
//! 1. `user_vault_pda` (writable)
//! 2. `app_address` (readonly)
//! 3. `user_vault_ata` (writable) — must be empty; authority is the vault PDA
//! 4. `destination` (writable) — receives rent (typically `owner`)
//! 5. `mint` (readonly)
//! 6. `token_program` (readonly)
//!
//! Data: `[discriminator (u8)]`

use crate::{
   constants::USER_VAULT_SEED,
   error::Error,
   helpers::{
      assert_spl_token_program, assert_user_vault_is_owned_by_program_and_correct_length,
      get_vault_ata_count, get_vault_bump, invoke_token_close_account,
      require_signer, verify_token_account, verify_vault_owner_and_app_address,
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
pub fn process(accounts: &mut [AccountView]) -> ProgramResult {
   let [
      owner,
      user_vault_pda,
      app_address,
      user_vault_ata,
      destination,
      mint,
      token_program,
   ] = accounts else {
      log!("close_vault_ata: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;
   assert_spl_token_program(token_program)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   verify_vault_owner_and_app_address(user_vault_pda, owner.address(), app_address.address())?;

   let ata_count = get_vault_ata_count(user_vault_pda);
   if unlikely(ata_count == 0) {
      log!("close_vault_ata: ata_count already zero");
      return Err(Error::UserVaultAtaCountZero.into());
   }

   let balance = verify_token_account(
      user_vault_ata,
      token_program,
      user_vault_pda.address(),
      mint.address(),
   )?;
   if unlikely(balance != 0) {
      log!("token account: balance not zero");
      return Err(Error::UserVaultAtaNotEmpty.into());
   }

   let bump_seed = [get_vault_bump(user_vault_pda)];
   let signer_seeds = [
      Seed::from(USER_VAULT_SEED),
      Seed::from(owner.address().as_ref()),
      Seed::from(app_address.address().as_ref()),
      Seed::from(&bump_seed[..]),
   ];
   let signers = [Signer::from(&signer_seeds[..])];

   invoke_token_close_account(token_program, user_vault_ata, destination, user_vault_pda, &signers).map_err(|e| {
      log!("close_vault_ata: close_account cpi failed");
      e
   })?;

   {
      let mut dst = user_vault_pda.try_borrow_mut()?;
      unsafe {
         core::ptr::write(dst.as_mut_ptr().add(2) as *mut u16, ata_count - 1);
      }
   }

   Ok(())
}
