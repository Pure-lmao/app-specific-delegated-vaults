//! Deposit SPL into the vault: create the vault ATA on first use, then `TransferChecked` from the
//! owner's source ATA. Increments `ata_count` only when the ATA is created.
//!
//! Accounts (9):
//! 0. `owner` (writable signer)
//! 1. `user_vault_pda` (writable)
//! 2. `user_vault_ata` (writable)
//! 3. `app_address` (readonly)
//! 4. `source_ata` (writable)
//! 5. `mint` (readonly)
//! 6. `system_program` (readonly)
//! 7. `token_program` (readonly)
//! 8. `associated_token_program` (readonly)
//!
//! Data: `[discriminator (u8), amount (u64)]`

use crate::{
   error::Error,
   helpers::{
      assert_associated_token_program, assert_spl_token_program, assert_system_program,
      assert_user_vault_is_owned_by_program_and_correct_length,
      get_vault_ata_count, invoke_token_transfer_checked_with_decimals,
      mint_base_decimals, parse_u64_instruction_data, require_signer,
      vault_ata_exists, verify_vault_owner_and_app_address,
   },
};
use pinocchio::{error::ProgramError, AccountView, ProgramResult, hint::unlikely};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_log::log;

#[inline(never)]
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let amount = parse_u64_instruction_data(data).map_err(|e| {
      log!("deposit_user_vault: invalid instruction data");
      e
   })?;
   if unlikely(amount == 0) {
      log!("deposit_user_vault: amount is zero");
      return Err(ProgramError::InvalidInstructionData);
   }

   let [
      owner,
      user_vault_pda,
      user_vault_ata,
      app_address,
      source_ata,
      mint,
      system_program,
      token_program,
      ata_program,
   ] = accounts else {
      log!("deposit_user_vault: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;
   assert_system_program(system_program)?;
   assert_spl_token_program(token_program)?;
   assert_associated_token_program(ata_program)?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   verify_vault_owner_and_app_address(user_vault_pda, owner.address(), app_address.address())?;

   let decimals = mint_base_decimals(mint).map_err(|e| {
      log!("deposit_user_vault: mint decimals read failed");
      e
   })?;

   let ata_exists = vault_ata_exists(user_vault_ata, token_program, user_vault_pda.address(), mint.address())?;

   if !ata_exists {
      Create {
         funding_account: owner,
         account: user_vault_ata,
         wallet: user_vault_pda,
         mint,
         system_program,
         token_program,
      }
      .invoke()
      .map_err(|e| {
         log!("deposit_user_vault: create ata failed");
         e
      })?;

      let count = get_vault_ata_count(user_vault_pda);
      let new_count = count.checked_add(1).ok_or_else(|| {
         log!("deposit_user_vault: ata_count overflow");
         Error::ArithmeticOverflow
      })?;
      {
         let mut dst = user_vault_pda.try_borrow_mut()?;
         unsafe {
            core::ptr::write(dst.as_mut_ptr().add(2) as *mut u16, new_count);
         }
      }
   }

   invoke_token_transfer_checked_with_decimals(
      token_program,
      mint,
      source_ata,
      user_vault_ata,
      owner,
      amount,
      &[],
      decimals,
   )
   .map_err(|e| {
      log!("deposit_user_vault: transfer failed");
      e
   })?;

   Ok(())
}
