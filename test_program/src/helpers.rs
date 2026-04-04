//! SPL checks and token transfers (mirrors vault patterns, trimmed).

use crate::error::Error;

use pinocchio::{
   account::Ref,
   cpi::Signer,
   error::ProgramError,
   AccountView, Address, ProgramResult,
};
use pinocchio_associated_token_account::check_id as ASSOCIATED_TOKEN_PROGRAM_CHECK_ID;
use pinocchio_log::log;
use pinocchio_system::check_id as SYSTEM_PROGRAM_CHECK_ID;
use pinocchio_token::{check_id as SPL_TOKEN_PROGRAM_CHECK_ID, instructions::TransferChecked, state::{Mint, TokenAccount}};
use pinocchio_token_2022::{check_id as TOKEN_2022_PROGRAM_CHECK_ID, instructions::TransferChecked as TransferChecked2022};

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
   let id = program.address();
   if !SPL_TOKEN_PROGRAM_CHECK_ID(id) && !TOKEN_2022_PROGRAM_CHECK_ID(id) {
      log!("invalid spl/token-2022 token program");
      return Err(Error::InvalidTokenProgram);
   }
   Ok(())
}

pub fn borrow_token_account<'a>(
   account: &'a AccountView,
   token_program: &AccountView,
) -> Result<Ref<'a, TokenAccount>, Error> {
   assert_spl_token_program(token_program)?;
   if !account.owned_by(token_program.address()) {
      log!("token account: owner program mismatch");
      return Err(Error::InvalidAta);
   }
   let data = account.try_borrow().map_err(|_| Error::InvalidAta)?;
   if data.len() < TokenAccount::LEN {
      log!("token account: data too short");
      return Err(Error::InvalidAta);
   }
   Ok(Ref::map(data, |d| unsafe {
      TokenAccount::from_bytes_unchecked(&d[..TokenAccount::LEN])
   }))
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
      log!("invalid associated token program");
      return Err(Error::InvalidAssociatedTokenProgram);
   }
   Ok(())
}

pub fn verify_token_account(
   account: &AccountView,
   token_program: &AccountView,
   expected_owner: &Address,
   expected_mint: &Address,
) -> Result<(), Error> {
   let token_account = borrow_token_account(account, token_program)?;
   if token_account.mint() != expected_mint {
      log!("token account mint mismatch");
      return Err(Error::MintMismatch);
   }
   if token_account.owner() != expected_owner {
      log!("token account owner mismatch");
      return Err(Error::InvalidAta);
   }
   if !token_account.is_initialized() {
      log!("token account not initialized");
      return Err(Error::InvalidAta);
   }
   Ok(())
}

pub fn verify_mint_account(mint: &AccountView, token_program: &AccountView) -> Result<(), Error> {
   assert_spl_token_program(token_program)?;
   if !mint.owned_by(token_program.address()) {
      log!("mint: owner program mismatch");
      return Err(Error::InvalidAta);
   }
   let data = mint.try_borrow().map_err(|_| Error::InvalidAta)?;
   if data.len() < Mint::LEN {
      log!("mint: data too short");
      return Err(Error::InvalidAta);
   }
   let m = unsafe { Mint::from_bytes_unchecked(&data[..Mint::LEN]) };
   if !m.is_initialized() {
      log!("mint: not initialized");
      return Err(Error::InvalidAta);
   }
   Ok(())
}

pub fn mint_decimals(mint: &AccountView, token_program: &AccountView) -> Result<u8, Error> {
   if !mint.owned_by(token_program.address()) {
      return Err(Error::InvalidAta);
   }
   let data = mint.try_borrow().map_err(|_| Error::InvalidAta)?;
   if data.len() < Mint::LEN {
      return Err(Error::InvalidAta);
   }
   let m = unsafe { Mint::from_bytes_unchecked(&data[..Mint::LEN]) };
   if !m.is_initialized() {
      return Err(Error::InvalidAta);
   }
   Ok(m.decimals())
}

pub fn treasury_ata_exists(
   account: &AccountView,
   token_program: &AccountView,
   treasury_wallet: &Address,
   mint: &Address,
) -> Result<bool, Error> {
   assert_spl_token_program(token_program)?;
   if !account.owned_by(token_program.address()) {
      return Ok(false);
   }
   let data = match account.try_borrow() {
      Ok(d) => d,
      Err(_) => return Ok(false),
   };
   if data.len() < TokenAccount::LEN {
      return Ok(false);
   }
   let token_account = unsafe { TokenAccount::from_bytes_unchecked(&data[..TokenAccount::LEN]) };
   if !token_account.is_initialized() {
      return Ok(false);
   }
   if token_account.mint() != mint {
      return Err(Error::MintMismatch);
   }
   if token_account.owner() != treasury_wallet {
      return Err(Error::InvalidAta);
   }
   Ok(true)
}

pub fn invoke_token_transfer_checked(
   token_program: &AccountView,
   mint: &AccountView,
   from: &AccountView,
   to: &AccountView,
   authority: &AccountView,
   amount: u64,
   signers: &[Signer],
) -> ProgramResult {
   let decimals = mint_decimals(mint, token_program).map_err(ProgramError::from)?;
   if SPL_TOKEN_PROGRAM_CHECK_ID(token_program.address()) {
      TransferChecked {
         from,
         mint,
         to,
         authority,
         amount,
         decimals,
      }
      .invoke_signed(signers)
   } else {
      TransferChecked2022 {
         from,
         mint,
         to,
         authority,
         amount,
         decimals,
         token_program: token_program.address(),
      }
      .invoke_signed(signers)
   }
}

#[inline]
pub fn parse_u64_instruction_data(data: &[u8]) -> Result<u64, ProgramError> {
   if data.len() != 8 {
      return Err(ProgramError::InvalidInstructionData);
   }
   Ok(u64::from_le_bytes(data.try_into().map_err(|_| ProgramError::InvalidInstructionData)?))
}
