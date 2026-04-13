use crate::{
   constants::{ID, USER_VAULT_SEED},
   error::Error,
   state::UserVaultAccount,
};

use pinocchio::{
   AccountView, Address, address::address_eq, hint::unlikely,
};
use pinocchio_log::log;

use super::{
   sysvar::require_delegate_not_expired,
   token::{
      TOKEN_ACCT_MINT_OFFSET, TOKEN_ACCT_OWNER_OFFSET, TOKEN_ACCT_STATE_OFFSET, token_account_base_ptr,
   },
};

/// Per-user vault metadata PDA: `["vault", owner, app_address]`.
#[inline]
pub fn derive_user_vault_pda(owner: &Address, app_address: &Address, program_id: &Address) -> (Address, u8) {
   Address::find_program_address(&[USER_VAULT_SEED, owner.as_ref(), app_address.as_ref()], program_id)
}

#[inline(always)]
pub fn assert_user_vault_is_owned_by_program_and_correct_length(user_vault_pda: &AccountView) -> Result<(), Error> {
   if unlikely(!address_eq(user_vault_pda.owner(), &ID)) {
      log!("user vault: not found");
      return Err(Error::UserVaultNotFound.into());
   }
   if unlikely(user_vault_pda.data_len() != UserVaultAccount::LEN) {
      log!("user vault: bad account data length");
      return Err(Error::InvalidUserVaultAccountLength.into());
   }
   Ok(())
}

#[inline(always)]
pub fn verify_vault_owner_and_app_address(user_vault_pda: &AccountView, owner: &Address, app_address: &Address) -> Result<(), Error> {
   let p = user_vault_pda.data_ptr();
   unsafe {
      let vault_owner = &*(p.add(8) as *const Address);
      if unlikely(!address_eq(owner, vault_owner)) {
         log!("user vault: owner does not match");
         return Err(Error::UserVaultOwnerMismatch.into());
      }
      let vault_app_address = &*(p.add(40) as *const Address);
      if unlikely(!address_eq(app_address, vault_app_address)) {
         log!("user vault: app address does not match");
         return Err(Error::UserVaultAppAddressMismatch.into());
      }
   }
   Ok(())
}

/// Owner + app checks and PDA bump in one pass over vault data.
#[inline(always)]
pub fn verify_vault_owner_app_return_bump(
   user_vault_pda: &AccountView,
   owner: &Address,
   app_address: &Address,
) -> Result<u8, Error> {
   let p = user_vault_pda.data_ptr();
   unsafe {
      let vault_owner = &*(p.add(8) as *const Address);
      if unlikely(!address_eq(owner, vault_owner)) {
         log!("user vault: owner does not match");
         return Err(Error::UserVaultOwnerMismatch.into());
      }
      let vault_app_address = &*(p.add(40) as *const Address);
      if unlikely(!address_eq(app_address, vault_app_address)) {
         log!("user vault: app address does not match");
         return Err(Error::UserVaultAppAddressMismatch.into());
      }
      Ok(*p.add(1))
   }
}

/// Delegate CPI path: owner, app, delegate, expiry vs clock; returns vault bump for PDA seeds (single vault data pass).
#[inline(always)]
pub fn verify_delegate_authority_return_bump(
   user_vault_pda: &AccountView,
   owner: &Address,
   app_address: &Address,
   delegate: &Address,
   clock_sysvar: &AccountView,
) -> Result<u8, Error> {
   let p = user_vault_pda.data_ptr();
   unsafe {
      let vault_owner = &*(p.add(8) as *const Address);
      if unlikely(!address_eq(owner, vault_owner)) {
         log!("user vault: owner does not match");
         return Err(Error::UserVaultOwnerMismatch.into());
      }
      let vault_app_address = &*(p.add(40) as *const Address);
      if unlikely(!address_eq(app_address, vault_app_address)) {
         log!("user vault: app address does not match");
         return Err(Error::UserVaultAppAddressMismatch.into());
      }
      let vault_delegate = &*(p.add(72) as *const Address);
      if unlikely(!address_eq(delegate, vault_delegate)) {
         log!("user vault: delegate does not match");
         return Err(Error::InvalidUserVaultDelegate);
      }
      let delegate_expires = *(p.add(4) as *const u32);
      require_delegate_not_expired(delegate_expires, clock_sysvar)?;
      Ok(*p.add(1))
   }
}

/// Empty system-owned account suitable for `CreateAccount` (PDA pre-image).
#[inline]
pub fn assert_pda_uninitialized(account: &AccountView, system_id: &Address) -> Result<(), Error> {
   if unlikely(!address_eq(account.owner(), system_id)) {
      log!("pda already exists or wrong owner");
      return Err(Error::UserVaultAlreadyExists);
   }
   if unlikely(!account.is_data_empty() || account.lamports() != 0) {
      log!("pda already exists");
      return Err(Error::UserVaultAlreadyExists);
   }
   Ok(())
}

/// `true` if `account` is an initialized SPL token account for `wallet` + `mint` (e.g. vault ATA already exists).
pub fn vault_ata_exists(
   account: &AccountView,
   token_program: &AccountView,
   wallet: &Address,
   mint: &Address,
) -> Result<bool, Error> {
   let Some(ptr) = token_account_base_ptr(account, token_program) else {
      return Ok(false);
   };
   unsafe {
      if *ptr.add(TOKEN_ACCT_STATE_OFFSET) == 0 {
         return Ok(false);
      }
      if unlikely(!address_eq(&*(ptr.add(TOKEN_ACCT_MINT_OFFSET) as *const Address), mint)) {
         log!("vault ata: mint mismatch on existing account");
         return Err(Error::MintMismatch);
      }
      if unlikely(!address_eq(&*(ptr.add(TOKEN_ACCT_OWNER_OFFSET) as *const Address), wallet)) {
         log!("vault ata: owner mismatch on existing account");
         return Err(Error::InvalidAta);
      }
   }
   Ok(true)
}

#[inline(always)]
pub fn get_vault_bump(user_vault_pda: &AccountView) -> u8 {
   unsafe { *user_vault_pda.data_ptr().add(1) as u8 }
}

#[inline(always)]
pub fn get_vault_ata_count(user_vault_pda: &AccountView) -> u16 {
   unsafe { *(user_vault_pda.data_ptr().add(2) as *const u16) }
}

#[inline(always)]
pub fn get_vault_delegate_expires(user_vault_pda: &AccountView) -> u32 {
   unsafe { *(user_vault_pda.data_ptr().add(4) as *const u32) }
}

#[inline(always)]
pub fn verify_vault_delegate(user_vault_pda: &AccountView, delegate: &Address) -> Result<(), Error> {
   let p = user_vault_pda.data_ptr();
   let vault_delegate = unsafe { &*(p.add(72) as *const Address) };
   if unlikely(!address_eq(delegate, vault_delegate)) {
      log!("user vault: delegate does not match");
      return Err(Error::InvalidUserVaultDelegate);
   }
   Ok(())
}
