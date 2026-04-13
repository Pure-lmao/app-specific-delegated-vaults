//! Create the per-user vault PDA `["vault", owner, app_address]`: stores delegate metadata for app CPI
//! and direct `app_ix`. Owner signs and funds account creation.
//!
//! Accounts (6):
//! 0. `owner` (writable signer)
//! 1. `user_vault_pda` (writable, uninitialized)
//! 2. `app_address` (readonly)
//! 3. `delegate` (readonly) — initial delegate pubkey stored in state
//! 4. `rent_sysvar` (readonly) — `SysvarRent111111111111111111111111111111111`
//! 5. `system_program` (readonly)
//!
//! Data: `[discriminator (u8), delegate_expires (u32)]` — Unix seconds; use `u32::MAX` for no practical expiry.

use crate::{
   constants::{ID, USER_VAULT_DISCRIMINATOR, USER_VAULT_SEED},
   error::Error,
   helpers::{
      assert_pda_uninitialized, derive_user_vault_pda, parse_u32_instruction_data,
      rent_minimum_balance_from_sysvar, require_signer,
   },
   state::UserVaultAccount,
};
use pinocchio::{
   address::address_eq,
   cpi::{Seed, Signer},
   error::ProgramError,
   AccountView, Address, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_system::ID as SYSTEM_PROGRAM_ID;


#[inline(never)]
pub fn process(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let [
      owner,
      user_vault_pda,
      app_address,
      delegate_account,
      rent_sysvar,
      system_program,
   ] = accounts else {
      log!("create_user_vault: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;

   if unlikely(!address_eq(system_program.address(), &SYSTEM_PROGRAM_ID)) {
      log!("create_user_vault: system program account mismatch");
      return Err(ProgramError::IncorrectProgramId);
   }

   let (expected_pda, bump) = derive_user_vault_pda(owner.address(), app_address.address(), program_id);
   if unlikely(user_vault_pda.address() != &expected_pda) {
      log!("create_user_vault: pda address mismatch");
      return Err(Error::UserVaultPdaMismatch.into());
   }

   assert_pda_uninitialized(user_vault_pda, &SYSTEM_PROGRAM_ID)?;

   let expires = parse_u32_instruction_data(data).map_err(|e| {
      log!("create_user_vault: invalid instruction data");
      e
   })?;

   let lamports = rent_minimum_balance_from_sysvar(rent_sysvar, UserVaultAccount::LEN).map_err(|e| {
      log!("create_user_vault: rent minimum balance failed");
      e
   })?;

   let bump_seed = [bump];
   let signer_seeds = [
      Seed::from(USER_VAULT_SEED),
      Seed::from(owner.address().as_ref()),
      Seed::from(app_address.address().as_ref()),
      Seed::from(&bump_seed[..]),
   ];
   let signers = [Signer::from(&signer_seeds[..])];

   CreateAccount {
      from: owner,
      to: user_vault_pda,
      lamports,
      space: UserVaultAccount::LEN as u64,
      owner: &ID,
   }
   .invoke_signed(&signers)
   .map_err(|e| {
      log!("create_user_vault: create account failed");
      e
   })?;

   {
      let mut dst = user_vault_pda.try_borrow_mut()?;
      let ptr = dst.as_mut_ptr();
      unsafe {
         *ptr = USER_VAULT_DISCRIMINATOR;
         *ptr.add(1) = bump;
         core::ptr::write(ptr.add(2) as *mut u16, 0u16);
         core::ptr::write(ptr.add(4) as *mut u32, expires);
         core::ptr::copy_nonoverlapping(owner.address().as_ref().as_ptr(), ptr.add(8), 32);
         core::ptr::copy_nonoverlapping(app_address.address().as_ref().as_ptr(), ptr.add(40), 32);
         core::ptr::copy_nonoverlapping(delegate_account.address().as_ref().as_ptr(), ptr.add(72), 32);
      }
   }

   Ok(())
}
