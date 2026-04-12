//! Update stored delegate and optional expiry. Owner must sign; delegate pubkey is not required to sign.
//!
//! Accounts (4):
//! 0. `owner` (writable signer)
//! 1. `user_vault_pda` (writable)
//! 2. `app_address` (readonly)
//! 3. `delegate` (readonly)
//!
//! Data: `[discriminator (u8), delegate_expires (u32)]` — Unix seconds; use `u32::MAX` for no practical expiry.

use crate::helpers::{
   assert_user_vault_is_owned_by_program_and_correct_length, parse_u32_instruction_data,
   require_signer, verify_vault_owner_and_app_address,
};
use pinocchio::{AccountView, ProgramResult, error::ProgramError};
use pinocchio_log::log;

#[inline(never)]
pub fn process(accounts: &mut [AccountView], data: &[u8]) -> ProgramResult {
   let [
      owner,
      user_vault_pda,
      app_address,
      delegate_account,
   ] = accounts else {
      log!("update_user_vault_delegate: not enough account keys");
      return Err(ProgramError::NotEnoughAccountKeys);
   };

   require_signer(owner)?;

   let expires = parse_u32_instruction_data(data).map_err(|e| {
      log!("update_user_vault_delegate: invalid instruction data");
      e
   })?;

   assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda)?;
   verify_vault_owner_and_app_address(user_vault_pda, owner.address(), app_address.address())?;

   {
      let mut dst = user_vault_pda.try_borrow_mut()?;
      let ptr = dst.as_mut_ptr();
      unsafe {
         core::ptr::copy_nonoverlapping(
            delegate_account.address().as_ref().as_ptr(),
            ptr.add(72),
            32,
         );
         core::ptr::write(ptr.add(4) as *mut u32, expires);
      }
   }

   Ok(())
}
