//! Create config PDA + pool ATA for devnet USDC.
//!
//! Accounts:
//! 0. `payer` (writable signer)
//! 1. `config_pda` (writable signer PDA)
//! 2. `pool_ata` (writable)
//! 3. `mint` (readonly) — must be devnet USDC
//! 4. `system_program` (readonly)
//! 5. `token_program` (readonly)
//! 6. `ata_program` (readonly)
//! 7. `rent_sysvar` (readonly)
//!
//! Data: empty (discriminator `0` only in top-level ix).

use crate::{
   constants::{CONFIG_DATA_LEN, CONFIG_SEED},
   error::Error,
   helpers::{
      assert_associated_token_program, assert_system_program, assert_spl_token_program, pool_ata_exists,
      require_signer, verify_usdc_mint,
   },
};

use pinocchio::{
   address::address_eq,
   cpi::{Seed, Signer},
   error::ProgramError,
   AccountView, Address, ProgramResult,
   hint::unlikely,
};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_log::log;
use pinocchio_system::instructions::CreateAccount;
use pinocchio_system::ID as SYSTEM_PROGRAM_ID;

#[inline(never)]
pub fn process(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   if unlikely(!data.is_empty()) {
      log!("init: unexpected instruction data");
      return Err(Error::BadInstructionData.into());
   }

   let [
      payer,
      config_pda,
      pool_ata,
      mint,
      system_program,
      token_program,
      ata_program,
      rent_sysvar,
   ] = accounts
   else {
      log!("init: not enough accounts");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(payer)?;
   assert_system_program(system_program)?;
   assert_spl_token_program(token_program)?;
   assert_associated_token_program(ata_program)?;

   if unlikely(!address_eq(system_program.address(), &SYSTEM_PROGRAM_ID)) {
      log!("init: system program id mismatch");
      return Err(ProgramError::IncorrectProgramId);
   }

   verify_usdc_mint(mint, token_program)?;

   let (expected_config, bump) = Address::find_program_address(&[CONFIG_SEED], program_id);
   if unlikely(config_pda.address() != &expected_config) {
      log!("init: config pda mismatch");
      return Err(Error::ConfigPdaMismatch.into());
   }

   if unlikely(config_pda.data_len() > 0) {
      log!("init: already initialized");
      return Err(Error::AlreadyInitialized.into());
   }

   let create_config = CreateAccount::with_minimum_balance(
      payer,
      config_pda,
      CONFIG_DATA_LEN as u64,
      program_id,
      Some(rent_sysvar),
   )
   .map_err(|e| {
      log!("init: rent minimum balance");
      e
   })?;

   let bump_seed = [bump];
   let signer_seeds = [Seed::from(CONFIG_SEED), Seed::from(&bump_seed[..])];
   let signers = [Signer::from(&signer_seeds[..])];

   create_config.invoke_signed(&signers).map_err(|e| {
      log!("init: create config failed");
      e
   })?;

   {
      let mut dst = config_pda.try_borrow_mut().map_err(|_| ProgramError::AccountBorrowFailed)?;
      dst[0] = 1u8;
   }

   if unlikely(pool_ata_exists(pool_ata, token_program, config_pda.address())?) {
      log!("init: pool ata already exists");
      return Err(Error::AlreadyInitialized.into());
   }

   Create {
      funding_account: payer,
      account: pool_ata,
      wallet: config_pda,
      mint,
      system_program,
      token_program,
   }
   .invoke_signed(&signers)
   .map_err(|e| {
      log!("init: create pool ata failed");
      e
   })?;

   Ok(())
}
