//! Instruction discriminators and dispatch.

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};

mod flip_from_app_ix;
mod flip_via_cpi;
mod init;

use pinocchio_log::log;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Instruction {
   Init = 0,
   /// Inner ix for vault `AppIx` (same as previous `Flip`).
   FlipFromAppIx = 1,
   /// Top-level ix: lose path uses vault `cpi_entry`; win path uses config-signed `Transfer`.
   FlipViaCpi = 2,
}

#[inline(never)]
pub fn dispatch(program_id: &Address, d: u8, data: &[u8], accounts: &mut [AccountView]) -> ProgramResult {
   match d {
      0 => init::process(program_id, accounts, data),
      1 => flip_from_app_ix::flip_from_app_ix(program_id, accounts, data),
      2 => flip_via_cpi::flip_via_cpi(program_id, accounts, data),
      _ => {
         log!("unknown instruction");
         Err(ProgramError::InvalidInstructionData)
      }
   }
}
