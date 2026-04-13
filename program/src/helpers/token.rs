use crate::error::Error;

use core::ptr::read;

use pinocchio::{
   AccountView, Address, ProgramResult, address::address_eq, cpi::{Signer, invoke_signed},
   hint::unlikely, instruction::{InstructionAccount, InstructionView},
};
use pinocchio_log::log;
use pinocchio_token_2022::instructions::TransferChecked as TransferChecked2022;

pub(crate) const TOKEN_ACCT_MINT_OFFSET: usize = 0;
pub(crate) const TOKEN_ACCT_OWNER_OFFSET: usize = 32;
pub(crate) const TOKEN_ACCT_AMOUNT_OFFSET: usize = 64;
pub(crate) const TOKEN_ACCT_STATE_OFFSET: usize = 108;
pub(crate) const TOKEN_ACCT_LEN: usize = 165;

const MINT_BASE_DECIMALS_OFFSET: usize = 44;
/// SPL Token mint account base size (layout through `freeze_authority`); extension mints are larger.
const SPL_MINT_BASE_LEN: usize = 82;

#[inline(always)]
fn assert_token_account_len(account: &AccountView, token_program: &AccountView) -> Result<*const u8, Error> {
   if unlikely(!address_eq(account.owner(), token_program.address())) {
      log!("token account: owner program mismatch");
      return Err(Error::InvalidAta);
   }
   if unlikely(account.data_len() < TOKEN_ACCT_LEN) {
      log!("token account: data too short");
      return Err(Error::InvalidAta);
   }
   Ok(account.data_ptr())
}

/// Token account owned by `token_program` with at least base SPL layout bytes; otherwise `None`.
#[inline]
pub(crate) fn token_account_base_ptr(
   account: &AccountView,
   token_program: &AccountView,
) -> Option<*const u8> {
   if !address_eq(account.owner(), token_program.address()) {
      return None;
   }
   if account.data_len() < TOKEN_ACCT_LEN {
      return None;
   }
   Some(account.data_ptr())
}

/// SPL token account: program owner, mint, wallet authority, initialized — no balance read (withdraw dest check).
pub fn verify_token_account_owner_mint(
   account: &AccountView,
   token_program: &AccountView,
   expected_owner: &Address,
   expected_mint: &Address,
) -> Result<(), Error> {
   let ptr = assert_token_account_len(account, token_program)?;
   unsafe {
      if unlikely(!address_eq(&*(ptr.add(TOKEN_ACCT_MINT_OFFSET) as *const Address), expected_mint)) {
         log!("token account mint mismatch");
         return Err(Error::MintMismatch);
      }
      if unlikely(!address_eq(&*(ptr.add(TOKEN_ACCT_OWNER_OFFSET) as *const Address), expected_owner)) {
         log!("token account owner mismatch");
         return Err(Error::InvalidAta);
      }
      if unlikely(*ptr.add(TOKEN_ACCT_STATE_OFFSET) == 0) {
         log!("token account not initialized");
         return Err(Error::InvalidAta);
      }
   }
   Ok(())
}

/// Base SPL token account balance after owner + length + initialized checks (for pre-close zero check).
#[inline]
pub fn read_token_account_balance_for_close(
   account: &AccountView,
   token_program: &AccountView,
) -> Result<u64, Error> {
   let ptr = assert_token_account_len(account, token_program)?;
   unsafe {
      if unlikely(*ptr.add(TOKEN_ACCT_STATE_OFFSET) == 0) {
         log!("token account not initialized");
         return Err(Error::InvalidAta);
      }
      Ok(read(ptr.add(TOKEN_ACCT_AMOUNT_OFFSET) as *const u64))
   }
}

/// Mint `decimals` from base SPL layout (`SPL_MINT_BASE_LEN`); CPI enforces mint validity.
#[inline(always)]
pub fn read_mint_decimals(mint: &AccountView) -> Result<u8, Error> {
   if unlikely(mint.data_len() < SPL_MINT_BASE_LEN) {
      log!("mint: data too short");
      return Err(Error::InvalidAta);
   }
   let ptr = mint.data_ptr();
   Ok(unsafe { *ptr.add(MINT_BASE_DECIMALS_OFFSET) })
}

/// SPL `TransferChecked` via classic Token or Token-2022, with caller-supplied `decimals`.
pub fn invoke_token_transfer_checked_with_decimals(
   token_program: &AccountView,
   mint: &AccountView,
   from: &AccountView,
   to: &AccountView,
   authority: &AccountView,
   amount: u64,
   signers: &[Signer],
   decimals: u8,
) -> ProgramResult {
   // Token-2022 `TransferChecked` works for both classic and Token-2022 if the correct program id is provided.
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

/// SPL `CloseAccount` (discriminator 9) via the given token program.
pub fn invoke_token_close_account(
   token_program: &AccountView,
   account: &AccountView,
   destination: &AccountView,
   authority: &AccountView,
   signers: &[Signer],
) -> ProgramResult {
   let instruction_accounts = [
      InstructionAccount::writable(account.address()),
      InstructionAccount::writable(destination.address()),
      InstructionAccount::readonly_signer(authority.address()),
   ];
   let ix = InstructionView {
      program_id: token_program.address(),
      accounts: &instruction_accounts,
      data: &[9],
   };
   invoke_signed(&ix, &[account, destination, authority], signers)
}
