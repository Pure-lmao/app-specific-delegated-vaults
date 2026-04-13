use crate::error::Error;

use core::ptr::read;

use pinocchio::{
   AccountView, ProgramResult, error::ProgramError, hint::unlikely,
};
use pinocchio_log::log;

#[inline]
pub fn require_signer(account: &AccountView) -> Result<(), Error> {
   if unlikely(!account.is_signer()) {
      log!("account is not a signer");
      return Err(Error::NotSigner);
   }
   Ok(())
}

#[inline]
pub fn transfer_lamports_from_user_vault_pda(
   source: &mut AccountView,
   destination: &mut AccountView,
   lamports: u64,
) -> ProgramResult {
   // Caller must have loaded `source` as the program-owned vault PDA; no ownership check here.
   let src_bal = source.lamports();
   let dst_bal = destination.lamports();
   let new_src = src_bal.checked_sub(lamports).ok_or_else(|| {
      log!("native transfer: insufficient lamports");
      ProgramError::InsufficientFunds
   })?;
   let new_dst = dst_bal.checked_add(lamports).ok_or_else(|| {
      log!("native transfer: destination lamport overflow");
      Error::ArithmeticOverflow
   })?;
   source.set_lamports(new_src);
   destination.set_lamports(new_dst);
   Ok(())
}

/// Drain `account_to_close` lamports to `recipient` and close (program-owned PDA rent reclaim).
pub fn close_program_account_lamports_to(
   account_to_close: &mut AccountView,
   recipient: &mut AccountView,
) -> ProgramResult {
   let rent_lamports = account_to_close.lamports();
   let new_dest = recipient
      .lamports()
      .checked_add(rent_lamports)
      .ok_or_else(|| {
         log!("close account: lamport overflow");
         Error::ArithmeticOverflow
      })?;
   recipient.set_lamports(new_dest);
   account_to_close.set_lamports(0);
   account_to_close.close().map_err(|e| {
      log!("close account: close failed");
      e
   })
}

#[inline]
pub fn parse_u64_instruction_data(data: &[u8]) -> Result<u64, ProgramError> {
   if unlikely(data.len() != 8) {
      return Err(ProgramError::InvalidInstructionData);
   }
   Ok(unsafe { read(data.as_ptr() as *const u64) })
}

#[inline]
pub fn parse_two_u64_instruction_data(data: &[u8]) -> Result<(u64, u64), ProgramError> {
   if unlikely(data.len() != 16) {
      return Err(ProgramError::InvalidInstructionData);
   }
   let p = data.as_ptr();
   Ok(unsafe {
      (
         read(p as *const u64),
         read(p.add(8) as *const u64),
      )
   })
}

#[inline]
pub fn parse_u32_instruction_data(data: &[u8]) -> Result<u32, ProgramError> {
   if unlikely(data.len() != 4) {
      return Err(ProgramError::InvalidInstructionData);
   }
   Ok(unsafe { read(data.as_ptr() as *const u32) })
}
