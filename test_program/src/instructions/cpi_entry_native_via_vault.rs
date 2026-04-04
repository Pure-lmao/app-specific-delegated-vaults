//! CPI → vault **`CpiEntryNative`** (`7`). Mirrors `deposit_via_cpi` layout: `vault_program` then the 7 vault accounts (instructions sysvar, then system program).

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
   let amount_native = parse_u64_instruction_data(data).map_err(|e| {
      log!("cpi_entry_native_via_vault: bad amount");
      e
   })?;
   if amount_native == 0 {
      log!("cpi_entry_native_via_vault: zero amount");
      return Err(ProgramError::InvalidInstructionData);
   }

   if accounts.len() != 8 {
      log!("cpi_entry_native_via_vault: expected exactly 8 accounts (vault_program + 7 cpi_entry_native)");
      return Err(ProgramError::NotEnoughAccountKeys);
   }
   let vault_program = &accounts[0];
   let inner = &accounts[1..];
   if inner.len() > MAX_STATIC_CPI_ACCOUNTS {
      return Err(ProgramError::InvalidArgument);
   }

   let mut buf = [0u8; 9];
   buf[0] = 7;
   buf[1..9].copy_from_slice(&amount_native.to_le_bytes());

   let ix_metas: Vec<InstructionAccount> = inner.iter().map(InstructionAccount::from).collect();
   let ix_refs: Vec<&AccountView> = inner.iter().collect();

   let ix = InstructionView {
      program_id: vault_program.address(),
      accounts: ix_metas.as_slice(),
      data: &buf,
   };

   invoke_signed_with_slice(&ix, ix_refs.as_slice(), &[]).map_err(|e| {
      log!("cpi_entry_native_via_vault: invoke failed");
      e
   })
}
