//! SPL + system checks.

use crate::constants::USDC_MINT;
use crate::error::Error;

use pinocchio::{
   address::address_eq,
   error::ProgramError,
   AccountView, Address,
};
use pinocchio_associated_token_account::check_id as ASSOCIATED_TOKEN_PROGRAM_CHECK_ID;
use pinocchio_log::log;
use pinocchio_system::check_id as SYSTEM_PROGRAM_CHECK_ID;
use pinocchio_token::{
   check_id as SPL_TOKEN_PROGRAM_CHECK_ID,
   state::Account as SplTokenAccountState,
};

#[inline]
pub fn require_signer(account: &AccountView) -> Result<(), Error> {
   if !account.is_signer() {
      log!("account is not a signer");
      return Err(Error::NotSigner);
   }
   Ok(())
}

#[inline]
pub fn assert_spl_token_program(program: &AccountView) -> Result<(), Error> {
   if !SPL_TOKEN_PROGRAM_CHECK_ID(program.address()) {
      log!("invalid spl token program");
      return Err(Error::InvalidTokenProgram);
   }
   Ok(())
}

#[inline]
pub fn assert_system_program(program: &AccountView) -> Result<(), Error> {
   if !SYSTEM_PROGRAM_CHECK_ID(program.address()) {
      log!("invalid system program");
      return Err(Error::InvalidSystemProgram);
   }
   Ok(())
}

#[inline]
pub fn assert_associated_token_program(program: &AccountView) -> Result<(), Error> {
   if !ASSOCIATED_TOKEN_PROGRAM_CHECK_ID(program.address()) {
      log!("invalid ata program");
      return Err(Error::InvalidAssociatedTokenProgram);
   }
   Ok(())
}

pub fn verify_vault_pool_atas(
   vault_ata: &AccountView,
   pool_ata: &AccountView,
   token_program: &AccountView,
   vault_pda: &Address,
   config_pda: &Address,
) -> Result<(), Error> {
   assert_spl_token_program(token_program)?;
   for (label, acct) in [("vault_ata", vault_ata), ("pool_ata", pool_ata)] {
      if !acct.owned_by(token_program.address()) {
         log!("{}: wrong owner program", label);
         return Err(Error::TokenOwnerMismatch);
      }
      let data = acct.try_borrow().map_err(|_| Error::TokenOwnerMismatch)?;
      if data.len() < SplTokenAccountState::LEN {
         log!("{}: data too short", label);
         return Err(Error::TokenOwnerMismatch);
      }
      let t = unsafe { SplTokenAccountState::from_bytes_unchecked(&data[..SplTokenAccountState::LEN]) };
      if !t.is_initialized() {
         log!("{}: not initialized", label);
         return Err(Error::TokenOwnerMismatch);
      }
      if !address_eq(t.mint(), &USDC_MINT) {
         log!("{}: mint mismatch", label);
         return Err(Error::MintMismatch);
      }
   }
   let vd = vault_ata.try_borrow().map_err(|_| Error::TokenOwnerMismatch)?;
   let vt = unsafe { SplTokenAccountState::from_bytes_unchecked(&vd[..SplTokenAccountState::LEN]) };
   if !address_eq(vt.owner(), vault_pda) {
      log!("vault ata authority mismatch");
      return Err(Error::TokenOwnerMismatch);
   }
   let pd = pool_ata.try_borrow().map_err(|_| Error::TokenOwnerMismatch)?;
   let pt = unsafe { SplTokenAccountState::from_bytes_unchecked(&pd[..SplTokenAccountState::LEN]) };
   if !address_eq(pt.owner(), config_pda) {
      log!("pool ata authority mismatch");
      return Err(Error::TokenOwnerMismatch);
   }
   Ok(())
}

pub fn pool_ata_exists(
   account: &AccountView,
   token_program: &AccountView,
   config_pda: &Address,
) -> Result<bool, Error> {
   assert_spl_token_program(token_program)?;
   if !account.owned_by(token_program.address()) {
      return Ok(false);
   }
   let data = match account.try_borrow() {
      Ok(d) => d,
      Err(_) => return Ok(false),
   };
   if data.len() < SplTokenAccountState::LEN {
      return Ok(false);
   }
   let t = unsafe { SplTokenAccountState::from_bytes_unchecked(&data[..SplTokenAccountState::LEN]) };
   if !t.is_initialized() {
      return Ok(false);
   }
   if !address_eq(t.mint(), &USDC_MINT) {
      return Err(Error::MintMismatch);
   }
   if !address_eq(t.owner(), config_pda) {
      return Err(Error::TokenOwnerMismatch);
   }
   Ok(true)
}

pub fn verify_usdc_mint(mint: &AccountView, token_program: &AccountView) -> Result<(), Error> {
   assert_spl_token_program(token_program)?;
   if !mint.owned_by(token_program.address()) {
      log!("mint: wrong owner");
      return Err(Error::MintMismatch);
   }
   if !address_eq(mint.address(), &USDC_MINT) {
      log!("mint address must be devnet USDC");
      return Err(Error::MintMismatch);
   }
   Ok(())
}

#[inline]
pub fn parse_flip_amount_odd(data: &[u8]) -> Result<(u64, bool), ProgramError> {
   if data.len() != 9 {
      return Err(ProgramError::InvalidInstructionData);
   }
   let amount = u64::from_le_bytes(data[..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?);
   let odd = data[8] != 0;
   Ok((amount, odd))
}
