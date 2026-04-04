
//! Accounts (12): `vault_program` (readonly), then the 11 vault `cpi_entry` accounts.

use crate::helpers::parse_u64_instruction_data;

use alloc::vec::Vec;
use pinocchio::{
   cpi::{invoke_signed_with_slice, MAX_STATIC_CPI_ACCOUNTS},
   error::ProgramError,
   instruction::{InstructionAccount, InstructionView},
   AccountView, ProgramResult,
};
use pinocchio_log::log;

pub fn process(_program_id: &pinocchio::Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
   let amount = parse_u64_instruction_data(data).map_err(|e| {
      log!("claim_via_cpi: bad amount");
      e
   })?;
   if amount == 0 {
      log!("claim_via_cpi: zero amount");
      return Err(ProgramError::InvalidInstructionData);
   }

   if accounts.len() != 12 {
      log!("claim_via_cpi: expected exactly 12 accounts (vault_program + 11 cpi_entry)");
      return Err(ProgramError::NotEnoughAccountKeys);
   }
   let vault_program = &accounts[0];
   let inner = &accounts[1..];
   if inner.len() > MAX_STATIC_CPI_ACCOUNTS {
      return Err(ProgramError::InvalidArgument);
   }

   let mut buf = [0u8; 17];
   buf[0] = 6;
   buf[1..9].copy_from_slice(&0u64.to_le_bytes());
   buf[9..17].copy_from_slice(&amount.to_le_bytes());

   let ix_metas: Vec<InstructionAccount> = inner.iter().map(InstructionAccount::from).collect();
   let ix_refs: Vec<&AccountView> = inner.iter().collect();

   let ix = InstructionView {
      program_id: vault_program.address(),
      accounts: ix_metas.as_slice(),
      data: &buf,
   };

   invoke_signed_with_slice(&ix, ix_refs.as_slice(), &[]).map_err(|e| {
      log!("claim_via_cpi: invoke failed");
      e
   })
}
