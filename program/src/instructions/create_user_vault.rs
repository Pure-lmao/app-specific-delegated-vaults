//! Create the per-user vault PDA `["vault", owner, app_address]`: stores delegate metadata for app CPI
//! and direct `app_ix`. Owner signs and funds account creation.
//!
//! Accounts (5):
//! 0. `owner` (writable signer)
//! 1. `user_vault_pda` (writable, uninitialized)
//! 2. `app_address` (readonly)
//! 3. `delegate` (readonly) — initial delegate pubkey stored in state
//! 4. `system_program` (readonly)
//!
//! Data: `[discriminator (u8), delegate_expires (u32)]` — `delegate_expires == 0` means no expiry.

use crate::{
   constants::{ID, USER_VAULT_SEED}, error::Error, helpers::{
      assert_pda_uninitialized, assert_system_program, derive_user_vault_pda, parse_u32_instruction_data, require_signer
   }, state::UserVaultAccount
};
use pinocchio::{
   cpi::{Seed, Signer},
   error::ProgramError,
   AccountView, Address, ProgramResult,
   sysvars::{Sysvar,rent::Rent},
   hint::unlikely,
};
use pinocchio_log::log;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_system::ID as SYSTEM_PROGRAM_ID;


#[inline(never)]
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
   let [
      owner,
      user_vault_pda,
      app_address,
      delegate_account,
      system_program,
   ] = accounts else {
      log!("create_user_vault: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;
   assert_system_program(system_program)?;
   let delegate = delegate_account.address().clone();

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

   let rent = Rent::get()?;
   let lamports = rent.try_minimum_balance(UserVaultAccount::LEN).map_err(|e| {
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

   let state = UserVaultAccount {
      owner: owner.address().clone(),
      app_address: app_address.address().clone(),
      delegate,
      delegate_expires: expires,
      ata_count: 0,
      bump,
   };
   {
      let mut dst = user_vault_pda.try_borrow_mut()?;
      state.pack(&mut dst).map_err(|e| {
         log!("create_user_vault: pack failed");
         e
      })?;
   }

   Ok(())
}
