//! Flip as **top-level** flip-program instruction: lose path CPIs vault **`cpi_entry`** (vault ATA → pool);
//! win path uses SPL `Transfer` (pool → vault) with config PDA `invoke_signed`.
//!
//! `cpi_entry` requires the transaction top-level program to be `app_address` (this program).
//!
//! Accounts (13):
//! 0. `vault_program` (readonly) — must match [`crate::constants::VAULT_PROGRAM`]
//! 1–11. Vault `cpi_entry` layout (`program/src/instructions/cpi_entry.rs`): delegate, owner, `user_vault_pda`,
//!    `user_vault_ata`, `app_address` (this flip program), `lamports_dest`, `dest_ata` (= pool ATA), mint,
//!    token program, instructions sysvar, clock sysvar
//! 12. `config_pda` (writable) — pool→vault authority on win
//!
//! Data (after discriminator `2`): `amount: u64` LE, `odd: u8` (nonzero = bet odd).

use crate::{
   constants::{CONFIG_SEED, VAULT_PROGRAM},
   error::Error,
   helpers::{
      assert_spl_token_program, parse_flip_amount_odd, require_signer, verify_usdc_mint, verify_vault_pool_atas,
   },
};

use pinocchio::{
   address::address_eq,
   cpi::{invoke_signed_with_slice, Seed, Signer},
   error::ProgramError,
   instruction::{InstructionAccount, InstructionView},
   sysvars::clock::Clock,
   AccountView, Address, ProgramResult,
   hint::unlikely,
};
use pinocchio_log::log;
use pinocchio_token::instructions::Transfer;

#[inline(never)]
pub fn flip_via_cpi(program_id: &Address, accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let (amount, bet_odd) = parse_flip_amount_odd(data).map_err(|e| {
      log!("flip_via_cpi: bad amount/odd");
      e
   })?;
   if amount == 0 {
      log!("flip_via_cpi: zero amount");
      return Err(ProgramError::InvalidInstructionData);
   }

   if unlikely(accounts.len() != 13) {
      log!("flip_via_cpi: expected 13 accounts");
      return Err(ProgramError::NotEnoughAccountKeys);
   }

   let vault_program = &accounts[0];
   let inner = &accounts[1..12];
   let config_pda = &accounts[12];

   let delegate = &inner[0];
   let owner = &inner[1];
   let user_vault_pda = &inner[2];
   let user_vault_ata = &inner[3];
   let app_address = &inner[4];
   let lamports_dest = &inner[5];
   let pool_ata = &inner[6];
   let mint = &inner[7];
   let token_program = &inner[8];
   let instructions_sysvar = &inner[9];
   let clock_sysvar = &inner[10];

   if unlikely(!address_eq(vault_program.address(), &VAULT_PROGRAM)) {
      log!("flip_via_cpi: vault program mismatch");
      return Err(Error::VaultProgramMismatch.into());
   }

   require_signer(delegate)?;
   assert_spl_token_program(token_program)?;
   verify_usdc_mint(mint, token_program)?;

   if unlikely(!address_eq(app_address.address(), program_id)) {
      log!("flip_via_cpi: app_address must be this program");
      return Err(Error::AppAddressMismatch.into());
   }

   let (expected_config, bump) = Address::find_program_address(&[CONFIG_SEED], program_id);
   if unlikely(!address_eq(config_pda.address(), &expected_config)) {
      log!("flip_via_cpi: config pda mismatch");
      return Err(Error::ConfigPdaMismatch.into());
   }

   if unlikely(config_pda.data_len() < 1) {
      log!("flip_via_cpi: config not initialized");
      return Err(Error::NotInitialized.into());
   }
   {
      let cfg = config_pda.try_borrow().map_err(|_| ProgramError::AccountBorrowFailed)?;
      if unlikely(cfg[0] == 0) {
         log!("flip_via_cpi: inactive");
         return Err(Error::Inactive.into());
      }
   }

   verify_vault_pool_atas(
      user_vault_ata,
      pool_ata,
      token_program,
      user_vault_pda.address(),
      config_pda.address(),
   )?;

   let clock_ref = Clock::from_account_view(clock_sysvar).map_err(|_| {
      log!("flip_via_cpi: bad clock");
      Error::InvalidClock
   })?;
   let slot = clock_ref.slot;
   let outcome_odd = (slot % 2) == 1;
   let win = (outcome_odd && bet_odd) || (!outcome_odd && !bet_odd);

   if win {
      let bump_seed = [bump];
      let signer_seeds = [Seed::from(CONFIG_SEED), Seed::from(&bump_seed[..])];
      let signers = [Signer::from(&signer_seeds[..])];
      Transfer::new(pool_ata, user_vault_ata, config_pda, amount)
         .invoke_signed(&signers)
         .map_err(|e| {
            log!("flip_via_cpi: transfer pool->vault failed");
            e
         })?;
   } else {
      const CPI_ENTRY_ACCOUNTS: usize = 11;
      let ix_metas: [InstructionAccount; CPI_ENTRY_ACCOUNTS] = [
         InstructionAccount::from(delegate),
         InstructionAccount::from(owner),
         InstructionAccount::from(user_vault_pda),
         InstructionAccount::from(user_vault_ata),
         InstructionAccount::from(app_address),
         InstructionAccount::from(lamports_dest),
         InstructionAccount::from(pool_ata),
         InstructionAccount::from(mint),
         InstructionAccount::from(token_program),
         InstructionAccount::from(instructions_sysvar),
         InstructionAccount::from(clock_sysvar),
      ];
      let ix_refs: [&AccountView; CPI_ENTRY_ACCOUNTS] = [
         delegate,
         owner,
         user_vault_pda,
         user_vault_ata,
         app_address,
         lamports_dest,
         pool_ata,
         mint,
         token_program,
         instructions_sysvar,
         clock_sysvar,
      ];

      let mut buf = [0u8; 17];
      buf[0] = 6;
      buf[1..9].copy_from_slice(&0u64.to_le_bytes());
      buf[9..17].copy_from_slice(&amount.to_le_bytes());

      let ix = InstructionView {
         program_id: vault_program.address(),
         accounts: &ix_metas,
         data: &buf,
      };

      invoke_signed_with_slice(&ix, &ix_refs, &[]).map_err(|e| {
         log!("flip_via_cpi: vault cpi_entry failed");
         e
      })?;
   }

   Ok(())
}
