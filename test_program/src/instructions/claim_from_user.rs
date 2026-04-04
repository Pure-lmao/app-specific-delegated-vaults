//! SPL transfer treasury ATA → `dest_ata`; treasury PDA signs.

use crate::{
   constants::TREASURY_SEED,
   error::Error,
   helpers::{
      assert_spl_token_program, invoke_token_transfer_checked, parse_u64_instruction_data, require_signer,
      verify_mint_account, verify_token_account,
   },
};

use pinocchio::{
   cpi::{Seed, Signer},
   error::ProgramError,
   AccountView, Address, ProgramResult,
};
use pinocchio_log::log;

pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
   let amount = parse_u64_instruction_data(data).map_err(|e| {
      log!("claim_from_user: bad amount");
      e
   })?;
   if amount == 0 {
      log!("claim_from_user: zero amount");
      return Err(ProgramError::InvalidInstructionData);
   }

   let [signer, authority, treasury_pda, treasury_ata, dest_ata, mint, token_program] = accounts else {
      log!("claim_from_user: not enough keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(signer)?;
   assert_spl_token_program(token_program)?;

   let (expected_treasury, bump) = Address::find_program_address(&[TREASURY_SEED], program_id);
   if treasury_pda.address() != &expected_treasury {
      log!("claim_from_user: treasury pda mismatch");
      return Err(Error::TreasuryPdaMismatch.into());
   }

   verify_mint_account(mint, token_program)?;
   verify_token_account(dest_ata, token_program, authority.address(), mint.address())?;
   verify_token_account(treasury_ata, token_program, treasury_pda.address(), mint.address())?;

   let bump_seed = [bump];
   let signer_seeds = [Seed::from(TREASURY_SEED), Seed::from(&bump_seed[..])];
   let signers = [Signer::from(&signer_seeds[..])];

   invoke_token_transfer_checked(
      token_program,
      mint,
      treasury_ata,
      dest_ata,
      treasury_pda,
      amount,
      &signers,
   )
   .map_err(|e| {
      log!("claim_from_user: transfer failed");
      e
   })
}
