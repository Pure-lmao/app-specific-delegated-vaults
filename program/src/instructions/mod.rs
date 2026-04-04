//! [`Instruction`] discriminators and [`dispatch`] routing on the first byte of instruction data.
//! `#[inline(never)]` on [`dispatch`] and each handler `process` keeps nested stack usage bounded (SBF 4KiB stack).

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};


mod create_user_vault;
mod deposit_user_vault;
mod update_user_vault_delegate;
mod withdraw_user_vault;
mod withdraw_user_vault_native;
mod app_ix;
mod cpi_entry;
mod cpi_entry_native;
mod close_vault_ata;
mod close_user_vault;

use pinocchio_log::log;

/// Single-byte instruction indices (`0` … `255`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instruction {
   CreateUserVault = 0,
   DepositUserVault = 1,
   UpdateUserVaultDelegate = 2,
   WithdrawUserVault = 3,
   WithdrawUserVaultNative = 4,
   AppIx = 5,
   CpiEntry = 6,
   CpiEntryNative = 7,
   CloseVaultAta = 8,
   CloseUserVault = 9,
}

#[inline(never)]
pub fn dispatch(program_id: &Address, d: u8, data: &[u8], accounts: &[AccountView]) -> ProgramResult {
   if d == Instruction::CreateUserVault as u8 {create_user_vault::process(program_id, accounts, data)}
   else if d == Instruction::UpdateUserVaultDelegate as u8 {update_user_vault_delegate::process(program_id, accounts, data)}
   else if d == Instruction::DepositUserVault as u8 {deposit_user_vault::process(program_id, accounts, data)}
   else if d == Instruction::WithdrawUserVault as u8 {withdraw_user_vault::process(program_id, accounts, data)}
   else if d == Instruction::WithdrawUserVaultNative as u8 {withdraw_user_vault_native::process(program_id, accounts, data)}
   else if d == Instruction::AppIx as u8 {app_ix::process(program_id, accounts, data)}
   else if d == Instruction::CpiEntry as u8 {cpi_entry::process(program_id, accounts, data)}
   else if d == Instruction::CpiEntryNative as u8 {cpi_entry_native::process(program_id, accounts, data)}
   else if d == Instruction::CloseVaultAta as u8 {close_vault_ata::process(program_id, accounts)}
   else if d == Instruction::CloseUserVault as u8 {close_user_vault::process(program_id, accounts)}
   else {
      log!("unknown instruction discriminator");
      Err(ProgramError::InvalidInstructionData)
   }
}
