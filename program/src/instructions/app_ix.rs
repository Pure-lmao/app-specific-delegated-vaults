//! Delegate-signed entrypoint: CPI from this program into `app_address`, with the vault PDA as a
//! signer (PDA seeds). Validates delegate + expiry; inner instruction uses `inner_accounts` and `data`.
//!
//! Accounts: 5 fixed, then all accounts required by the inner app instruction (same order as that ix).
//! 0. `delegate` (signer) — must match vault state
//! 1. `owner` (readonly)
//! 2. `user_vault_pda` (writable)
//! 3. `app_address` (readonly)
//! 4. `clock_sysvar` (readonly) — `SysvarC1ock11111111111111111111111111111111`
//! 5.. `inner_accounts` — metas for the CPI to the app program
//!
//! Data: `[discriminator (u8), ...inner_instruction_data]`
//!
//! CPI uses `invoke_signed_unchecked` (no Pinocchio borrow/address scan before the syscall). Inner
//! `InstructionAccount` metas live in the caller frame; `CpiAccount` rows are built in a nested
//! `#[inline(never)]` frame so each stack frame stays under the SBF 4KiB limit. A **single** fused
//! loop fills both from each `AccountView` (one pass over the inner account list).

use core::mem::MaybeUninit;

use crate::{
   constants::USER_VAULT_SEED,
   helpers::{
      assert_user_vault_is_owned_by_program_and_correct_length, require_signer,
      verify_delegate_authority_return_bump,
   },
};
use pinocchio::{
   cpi::{invoke_signed_unchecked, CpiAccount, Seed, Signer, MAX_STATIC_CPI_ACCOUNTS},
   error::ProgramError,
   hint::unlikely,
   instruction::{InstructionAccount, InstructionView},
   AccountView, Address, ProgramResult,
};
use pinocchio_log::log;
use solana_address::address_eq;

/// Fused meta + CPI row build and invoke. `metas_out` is caller-owned storage (smaller array on the
/// outer frame); `CpiAccount` buffer stays here so we never stack both full arrays in `process`.
#[inline(never)]
unsafe fn invoke_app_ix_fused<'a>(
   program_id: &Address,
   data: &[u8],
   inner_accounts: &'a [AccountView],
   signers: &[Signer],
   n: usize,
   metas_out: &mut [MaybeUninit<InstructionAccount<'a>>; MAX_STATIC_CPI_ACCOUNTS],
   vault_pda_address: &Address,
) {
   let mut cpi_accounts: [MaybeUninit<CpiAccount>; MAX_STATIC_CPI_ACCOUNTS] =
      unsafe { MaybeUninit::uninit().assume_init() };

   // SAFETY: `n == inner_accounts.len()` (caller); `i < n` ⇒ valid index.
   for i in 0..n {
      let a = unsafe { inner_accounts.get_unchecked(i) };
      if unlikely(address_eq(a.address(), vault_pda_address)) {
         metas_out[i].write(InstructionAccount::writable_signer(a.address()));
      } else {
         metas_out[i].write(InstructionAccount::from(a));
      }
      CpiAccount::init_from_account_view(a, &mut cpi_accounts[i]);
   }

   let ix_metas =
      unsafe { core::slice::from_raw_parts(metas_out.as_ptr() as *const InstructionAccount, n) };
   let ix = InstructionView {
      program_id,
      accounts: ix_metas,
      data,
   };
   let cpi_slice = core::slice::from_raw_parts(cpi_accounts.as_ptr() as *const CpiAccount, n);
   invoke_signed_unchecked(&ix, cpi_slice, signers);
}

#[inline(never)]
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let [
      delegate,
      owner,
      user_vault_pda,
      app_address,
      clock_sysvar,
      inner_accounts @ ..,
   ] = accounts
   else {
      log!("app_ix: not enough account keys (delegate, owner, user_vault_pda, app_address, clock, ...)");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(delegate)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   let bump = verify_delegate_authority_return_bump(
      user_vault_pda,
      owner.address(),
      app_address.address(),
      delegate.address(),
      clock_sysvar,
   )?;

   let bump_seed = [bump];
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

   let mut metas: [MaybeUninit<InstructionAccount>; MAX_STATIC_CPI_ACCOUNTS] =
      unsafe { MaybeUninit::uninit().assume_init() };

   unsafe {
      invoke_app_ix_fused(
         app_address.address(),
         data,
         inner_accounts,
         &signers,
         n,
         &mut metas,
         user_vault_pda.address(),
      );
   }

   Ok(())
}
