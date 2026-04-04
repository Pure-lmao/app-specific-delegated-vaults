//! Instruction discriminators `0` … `5`.

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

mod claim_from_user;
mod claim_via_cpi;
mod cpi_entry_dual_via_vault;
mod cpi_entry_native_via_vault;
mod deposit_from_user;
mod deposit_via_cpi;

use pinocchio_log::log;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instruction {
   DepositViaCpi = 0,
   ClaimViaCpi = 1,
   DepositFromUser = 2,
   ClaimFromUser = 3,
   CpiEntryNativeViaVault = 4,
   CpiEntryDualViaVault = 5,
}

#[inline(never)]
pub fn dispatch(program_id: &Address, d: u8, data: &[u8], accounts: &[AccountView]) -> ProgramResult {
   if d == Instruction::DepositViaCpi as u8 {
      deposit_via_cpi::process(program_id, accounts, data)
   } else if d == Instruction::ClaimViaCpi as u8 {
      claim_via_cpi::process(program_id, accounts, data)
   } else if d == Instruction::DepositFromUser as u8 {
      deposit_from_user::process(program_id, accounts, data)
   } else if d == Instruction::ClaimFromUser as u8 {
      claim_from_user::process(program_id, accounts, data)
   } else if d == Instruction::CpiEntryNativeViaVault as u8 {
      cpi_entry_native_via_vault::process(program_id, accounts, data)
   } else if d == Instruction::CpiEntryDualViaVault as u8 {
      cpi_entry_dual_via_vault::process(program_id, accounts, data)
   } else {
      log!("unknown instruction");
      Err(ProgramError::InvalidInstructionData)
   }
}
