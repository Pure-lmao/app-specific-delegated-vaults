//! Update stored delegate and optional expiry. Owner must sign; delegate pubkey is not required to sign.
//!
//! Accounts (4):
//! 0. `owner` (writable signer)
//! 1. `user_vault_pda` (writable)
//! 2. `app_address` (readonly)
//! 3. `delegate` (readonly)
//!
//! Data: `[discriminator (u8), delegate_expires (u32)]` — `delegate_expires == 0` means no expiry.

use crate::helpers::{load_user_vault, parse_u32_instruction_data, require_signer};
use pinocchio::{AccountView, Address, ProgramResult, error::ProgramError};
use pinocchio_log::log;

#[inline(never)]
pub fn process(program_id: &Address, accounts: &[AccountView], data: &[u8]) -> ProgramResult {
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

   let mut vault = load_user_vault(program_id, user_vault_pda, owner.address(), app_address.address())?;
   vault.delegate = delegate_account.address().clone();
   vault.delegate_expires = expires;
   {
      let mut dst = user_vault_pda.try_borrow_mut()?;
      vault.pack(&mut dst).map_err(|e| {
         log!("update_user_vault_delegate: pack failed");
         e
      })?;
   }

   Ok(())
}
