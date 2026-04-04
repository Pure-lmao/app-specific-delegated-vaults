#![no_std]

extern crate alloc;

pub mod constants;
pub mod error;
pub mod helpers;
pub mod instructions;

use pinocchio::{error::ProgramError, AccountView, Address, ProgramResult};
use pinocchio_log::log;

pub use constants::ID;

pub fn process_instruction(
   program_id: &Address,
   accounts: &[AccountView],
   instruction_data: &[u8],
) -> ProgramResult {
   let Some((discriminator, data)) = instruction_data.split_first() else {
      log!("instruction data empty");
      return Err(ProgramError::InvalidInstructionData);
   };
   instructions::dispatch(program_id, *discriminator, data, accounts)
}

#[cfg(feature = "bpf-entrypoint")]
mod bpf_entrypoint {
   use pinocchio::{AccountView, Address, ProgramResult};
   use super::process_instruction as route_instruction;

   pinocchio::program_entrypoint!(process_instruction);
   pinocchio::default_allocator!();
   pinocchio::nostd_panic_handler!();

   fn process_instruction(
      program_id: &Address,
      accounts: &[AccountView],
      instruction_data: &[u8],
   ) -> ProgramResult {
      route_instruction(program_id, accounts, instruction_data)
   }
}
