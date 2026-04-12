//! Delegate-signed entrypoint: CPI from this program into `app_address`, with the vault PDA as a
//! signer (PDA seeds). Validates delegate + expiry; inner instruction uses `inner_accounts` and `data`.
//!
//! Accounts: 5 fixed, then all accounts required by the inner app instruction (same order as that ix).
//! 0. `delegate` (signer) — must match vault state
//! 1. `owner` (readonly)
//! 2. `user_vault_pda` (readonly)
//! 3. `app_address` (readonly)
//! 4. `clock_sysvar` (readonly) — `SysvarC1ock11111111111111111111111111111111`
//! 5.. `inner_accounts` — metas for the CPI to the app program
//!
//! Data: `[discriminator (u8), ...inner_instruction_data]`

use core::mem::MaybeUninit;

use crate::{
   constants::USER_VAULT_SEED,
   helpers::{
      assert_user_vault_is_owned_by_program_and_correct_length, get_vault_bump,
      get_vault_delegate_expires, require_delegate_not_expired, require_signer,
      verify_vault_delegate, verify_vault_owner_and_app_address,
   },
};
use pinocchio::{
   cpi::{invoke_signed_with_slice, Seed, Signer, MAX_STATIC_CPI_ACCOUNTS},
   error::ProgramError,
   instruction::{InstructionAccount, InstructionView},
   AccountView, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;

#[inline(never)]
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let [
      delegate,
      owner,
      user_vault_pda,
      app_address,
      clock_sysvar,
      inner_accounts @ ..,
   ] = accounts else {
      log!("app_ix: not enough account keys (delegate, owner, user_vault_pda, app_address, clock, ...)");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(delegate)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   verify_vault_owner_and_app_address(user_vault_pda, owner.address(), app_address.address())?;
   verify_vault_delegate(user_vault_pda, delegate.address())?;
   require_delegate_not_expired(
      get_vault_delegate_expires(user_vault_pda),
      clock_sysvar
   )?;

   let bump_seed = [get_vault_bump(user_vault_pda)];
   let signer_seeds = [
      Seed::from(USER_VAULT_SEED),
      Seed::from(owner.address().as_ref()),
      Seed::from(app_address.address().as_ref()),
      Seed::from(&bump_seed[..]),
   ];
   let signers = [Signer::from(&signer_seeds[..])];

   let n = inner_accounts.len();
   if unlikely(n > MAX_STATIC_CPI_ACCOUNTS) {
      log!("app_ix: inner accounts exceed CPI limit");
      return Err(ProgramError::InvalidArgument);
   }

   if unlikely(n == 0) {
      let ix = InstructionView {
         program_id: app_address.address(),
         accounts: &[],
         data,
      };
      let no_accounts: &[AccountView] = &[];
      return invoke_signed_with_slice(&ix, no_accounts, &signers).map_err(|e| {
         log!("app_ix: invoke failed");
         e
      });
   }

   let mut metas: [MaybeUninit<InstructionAccount>; MAX_STATIC_CPI_ACCOUNTS] =
      unsafe { MaybeUninit::uninit().assume_init() };
   let mut refs: [MaybeUninit<&AccountView>; MAX_STATIC_CPI_ACCOUNTS] =
      unsafe { MaybeUninit::uninit().assume_init() };

   for (i, a) in inner_accounts.iter().enumerate() {
      metas[i].write(InstructionAccount::from(a));
      refs[i].write(a);
   }

   let ix_metas = unsafe { 
      core::slice::from_raw_parts(
         metas.as_ptr() as *const InstructionAccount, n) 
   };
   let ix_refs = unsafe { 
      core::slice::from_raw_parts(
         refs.as_ptr() as *const &AccountView, n) 
   };

   let ix = InstructionView {
      program_id: app_address.address(),
      accounts: ix_metas,
      data,
   };

   invoke_signed_with_slice(&ix, ix_refs, &signers).map_err(|e| {
      log!("app_ix: invoke failed");
      e
   })?;

   Ok(())
}
