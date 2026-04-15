//! Slot-parity flip when invoked as vault **`AppIx`** inner instruction: SPL `Transfer` between vault ATA and pool ATA.
//!
//! Accounts (no mint in list):
//! 0. `vault_ata` (writable)
//! 1. `pool_ata` (writable)
//! 2. `config_pda` (writable)
//! 3. `vault_pda` (writable)
//! 4. `token_program` (readonly)
//! 5. `clock_sysvar` (readonly)
//!
//! Data (after discriminator `1`): `amount: u64` LE, `odd: u8` (nonzero = bet odd).

use crate::{
   constants::CONFIG_SEED,
   error::Error,
   helpers::{assert_spl_token_program, parse_flip_amount_odd, verify_vault_pool_atas},
};

use pinocchio::{
   address::address_eq,
   cpi::{Seed, Signer},
   error::ProgramError,
   sysvars::clock::Clock,
   AccountView, Address, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;
use pinocchio_token::instructions::Transfer;

#[inline(never)]
pub fn flip_from_app_ix(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let (amount, bet_odd) = parse_flip_amount_odd(data).map_err(|e| {
      log!("flip_from_app_ix: bad amount/odd");
      e
   })?;
   if amount == 0 {
      log!("flip_from_app_ix: zero amount");
      return Err(ProgramError::InvalidInstructionData);
   }

   let [vault_ata, pool_ata, config_pda, vault_pda, token_program, clock_sysvar] = accounts else {
      log!("flip_from_app_ix: not enough accounts");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   assert_spl_token_program(token_program)?;

   let (expected_config, bump) = Address::find_program_address(&[CONFIG_SEED], program_id);
   if unlikely(!address_eq(config_pda.address(), &expected_config)) {
      log!("flip_from_app_ix: config pda mismatch");
      return Err(Error::ConfigPdaMismatch.into());
   }

   if unlikely(config_pda.data_len() < 1) {
      log!("flip_from_app_ix: config not initialized");
      return Err(Error::NotInitialized.into());
   }
   {
      let cfg = config_pda.try_borrow().map_err(|_| ProgramError::AccountBorrowFailed)?;
      if unlikely(cfg[0] == 0) {
         log!("flip_from_app_ix: inactive");
         return Err(Error::Inactive.into());
      }
   }

   verify_vault_pool_atas(
      vault_ata,
      pool_ata,
      token_program,
      vault_pda.address(),
      config_pda.address(),
   )?;

   let clock_ref = Clock::from_account_view(clock_sysvar).map_err(|_| {
      log!("flip_from_app_ix: bad clock");
      Error::InvalidClock
   })?;
   let slot = clock_ref.slot;
   let outcome_odd = (slot % 2) == 1;
   let win = (outcome_odd && bet_odd) || (!outcome_odd && !bet_odd);

   if win {
      let bump_seed = [bump];
      let signer_seeds = [Seed::from(CONFIG_SEED), Seed::from(&bump_seed[..])];
      let signers = [Signer::from(&signer_seeds[..])];
      Transfer::new(pool_ata, vault_ata, config_pda, amount)
         .invoke_signed(&signers)
         .map_err(|e| {
            log!("flip_from_app_ix: transfer pool->vault failed");
            e
         })?;
   } else {
      Transfer::new(vault_ata, pool_ata, vault_pda, amount).invoke().map_err(|e| {
         log!("flip_from_app_ix: transfer vault->pool failed");
         e
      })?;
   }

   Ok(())
}
