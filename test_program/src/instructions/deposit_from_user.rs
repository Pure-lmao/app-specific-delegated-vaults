//! SPL transfer `source_ata` → treasury ATA (PDA wallet). Optional ATA create.

use crate::{
   constants::TREASURY_SEED,
   error::Error,
   helpers::{
      assert_associated_token_program, assert_spl_token_program, assert_system_program, invoke_token_transfer_checked,
      parse_u64_instruction_data, require_signer, treasury_ata_exists, verify_mint_account, verify_token_account,
   },
};

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
use pinocchio_associated_token_account::instructions::Create;
use pinocchio_log::log;

pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
   let amount = parse_u64_instruction_data(data).map_err(|e| {
      log!("deposit_from_user: bad amount");
      e
   })?;
   if amount == 0 {
      log!("deposit_from_user: zero amount");
      return Err(ProgramError::InvalidInstructionData);
   }

   let [
      signer,
      authority,
      source_ata,
      treasury_pda,
      treasury_ata,
      mint,
      system_program,
      token_program,
      ata_program,
   ] = accounts
   else {
      log!("deposit_from_user: not enough keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(signer)?;
   assert_system_program(system_program)?;
   assert_spl_token_program(token_program)?;
   assert_associated_token_program(ata_program)?;

   let (expected_treasury, _bump) = Address::find_program_address(&[TREASURY_SEED], program_id);
   if treasury_pda.address() != &expected_treasury {
      log!("deposit_from_user: treasury pda mismatch");
      return Err(Error::TreasuryPdaMismatch.into());
   }

   verify_mint_account(mint, token_program)?;
   verify_token_account(source_ata, token_program, authority.address(), mint.address())?;

   let ata_ok = treasury_ata_exists(treasury_ata, token_program, treasury_pda.address(), mint.address())?;
   if !ata_ok {
      Create {
         funding_account: signer,
         account: treasury_ata,
         wallet: treasury_pda,
         mint,
         system_program,
         token_program,
      }
      .invoke()
      .map_err(|e| {
         log!("deposit_from_user: create ata failed");
         e
      })?;
   }

   verify_token_account(treasury_ata, token_program, treasury_pda.address(), mint.address())?;

   invoke_token_transfer_checked(
      token_program,
      mint,
      source_ata,
      treasury_ata,
      authority,
      amount,
      &[],
   )
   .map_err(|e| {
      log!("deposit_from_user: transfer failed");
      e
   })
}
