//! Account checks and derivations.
//!
//! Helpers that take a `token_program` account assume the caller has already validated it with
//! [`assert_spl_token_program`] (classic SPL Token or Token-2022).
//!
//! Inline policy (SBF ~4KiB stack): `#[inline]` on tiny checks and instruction-data parsers;
//! `#[inline(never)]` on sysvar work ([`get_time`], [`require_top_level_instruction_is_app`]).

use crate::{
   constants::USER_VAULT_SEED,
   error::Error,
   state::UserVaultAccount, constants::ID,
};

use pinocchio::{
   account::Ref,
   cpi::{invoke_signed, Signer},
   error::ProgramError,
   instruction::{InstructionAccount, InstructionView},
   AccountView, Address, ProgramResult,
   hint::unlikely,
   sysvars::{clock::Clock, instructions::Instructions, Sysvar},
};
use pinocchio_associated_token_account::check_id as ASSOCIATED_TOKEN_PROGRAM_CHECK_ID;
use pinocchio_log::log;
use pinocchio_system::check_id as SYSTEM_PROGRAM_CHECK_ID;
use pinocchio_token::{check_id as SPL_TOKEN_PROGRAM_CHECK_ID, state::{Mint, TokenAccount}};
use pinocchio_token_2022::{check_id as TOKEN_2022_PROGRAM_CHECK_ID, instructions::TransferChecked as TransferChecked2022};


#[inline]
pub fn require_signer(account: &AccountView) -> Result<(), Error> {
   if unlikely(!account.is_signer()) {
      log!("account is not a signer");
      return Err(Error::NotSigner);
   }
   Ok(())
}

/// Top-level instruction that started this CPI chain must belong to `app_address`.
#[inline(never)]
pub fn require_top_level_instruction_is_app(
   instructions_sysvar: &AccountView,
   app_address: &AccountView,
) -> Result<(), ProgramError> {
   let ixs = Instructions::try_from(instructions_sysvar)?;
   let current_ix = ixs.load_instruction_at(ixs.load_current_index() as usize)?;
   if unlikely(current_ix.get_program_id() != app_address.address()) {
      log!("cpi_entry: must be invoked via CPI from the app program");
      return Err(Error::UnauthorizedCpiCaller.into());
   }
   Ok(())
}

/// Classic SPL Token or Token-2022 program id.
#[inline]
pub fn assert_spl_token_program(program: &AccountView) -> Result<(), Error> {
   let id = program.address();
   if unlikely(!SPL_TOKEN_PROGRAM_CHECK_ID(id) && !TOKEN_2022_PROGRAM_CHECK_ID(id)) {
      log!("invalid spl/token-2022 token program");
      return Err(Error::InvalidTokenProgram);
   }
   Ok(())
}

/// Borrow the base SPL token account layout; account must be owned by `token_program` (supports extension accounts).
pub fn borrow_token_account<'a>(
   account: &'a AccountView,
   token_program: &AccountView,
) -> Result<Ref<'a, TokenAccount>, Error> {
   if unlikely(!account.owned_by(token_program.address())) {
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
   if unlikely(!SYSTEM_PROGRAM_CHECK_ID(program.address())) {
      log!("invalid system program");
      return Err(Error::InvalidSystemProgram);
   }
   Ok(())
}

#[inline]
pub fn assert_associated_token_program(program: &AccountView) -> Result<(), Error> {
   if unlikely(!ASSOCIATED_TOKEN_PROGRAM_CHECK_ID(program.address())) {
      log!("invalid associated token program");
      return Err(Error::InvalidAssociatedTokenProgram);
   }
   Ok(())
}

/// SPL token account: correct program owner, mint, and wallet authority.
/// Returns SPL token `amount` after validation (e.g. check `== 0` before close).
pub fn verify_token_account(
   account: &AccountView,
   token_program: &AccountView,
   expected_owner: &Address,
   expected_mint: &Address,
) -> Result<u64, Error> {
   let token_account = borrow_token_account(account, token_program)?;
   if unlikely(token_account.mint() != expected_mint) {
      log!("token account mint mismatch");
      return Err(Error::MintMismatch);
   }
   if unlikely(token_account.owner() != expected_owner) {
      log!("token account owner mismatch");
      return Err(Error::InvalidAta);
   }
   if unlikely(!token_account.is_initialized()) {
      log!("token account not initialized");
      return Err(Error::InvalidAta);
   }
   Ok(token_account.amount())
}

/// `true` if `account` is an initialized SPL token account for `wallet` + `mint` (e.g. vault ATA already exists).
pub fn vault_ata_exists(
   account: &AccountView,
   token_program: &AccountView,
   wallet: &Address,
   mint: &Address,
) -> Result<bool, Error> {
   if !account.owned_by(token_program.address()) {
      return Ok(false);
   }
   let data = match account.try_borrow() {
      Ok(d) => d,
      Err(_) => return Ok(false),
   };
   if unlikely(data.len() < TokenAccount::LEN) {
      return Ok(false);
   }
   let token_account = unsafe { TokenAccount::from_bytes_unchecked(&data[..TokenAccount::LEN]) };
   if !token_account.is_initialized() {
      return Ok(false);
   }
   if unlikely(token_account.mint() != mint) {
      log!("vault ata: mint mismatch on existing account");
      return Err(Error::MintMismatch);
   }
   if unlikely(token_account.owner() != wallet) {
      log!("vault ata: owner mismatch on existing account");
      return Err(Error::InvalidAta);
   }
   Ok(true)
}

/// SPL token account: correct mint and initialized; owner not checked (e.g. arbitrary CPI destination).
pub fn verify_token_account_mint(
   account: &AccountView,
   token_program: &AccountView,
   expected_mint: &Address,
) -> Result<(), Error> {
   let token_account = borrow_token_account(account, token_program)?;
   if unlikely(token_account.mint() != expected_mint) {
      log!("token account mint mismatch");
      return Err(Error::MintMismatch);
   }
   if unlikely(!token_account.is_initialized()) {
      log!("token account not initialized");
      return Err(Error::InvalidAta);
   }
   Ok(())
}

/// Mint account owned by `token_program` (classic or Token-2022); supports extension mints (base layout only).
/// Returns base-layout `decimals` for use with `TransferChecked` without re-reading the mint.
pub fn verify_mint_account(mint: &AccountView, token_program: &AccountView) -> Result<u8, Error> {
   if unlikely(!mint.owned_by(token_program.address())) {
      log!("mint: owner program mismatch");
      return Err(Error::InvalidAta);
   }
   let data = mint.try_borrow().map_err(|_| Error::InvalidAta)?;
   if unlikely(data.len() < Mint::LEN) {
      log!("mint: data too short");
      return Err(Error::InvalidAta);
   }
   let m = unsafe { Mint::from_bytes_unchecked(&data[..Mint::LEN]) };
   if unlikely(!m.is_initialized()) {
      log!("mint: not initialized");
      return Err(Error::InvalidAta);
   }
   Ok(m.decimals())
}

/// SPL `TransferChecked` (discriminator 12) via classic Token or Token-2022.
pub fn invoke_token_transfer_checked(
   token_program: &AccountView,
   mint: &AccountView,
   from: &AccountView,
   to: &AccountView,
   authority: &AccountView,
   amount: u64,
   signers: &[Signer],
) -> ProgramResult {
   let decimals = verify_mint_account(mint, token_program).map_err(ProgramError::from)?;
   invoke_token_transfer_checked_with_decimals(
      token_program, mint, from, to, authority, amount, signers, decimals,
   )
}

/// Like [`invoke_token_transfer_checked`], but uses known `decimals` (e.g. after [`verify_mint_account`]) to avoid a second mint read.
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

pub fn transfer_lamports_from_user_vault_pda(
   source: &AccountView,
   destination: &AccountView,
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

/// Per-user vault metadata PDA: `["vault", owner, app_address]`.
#[inline]
pub fn derive_user_vault_pda(owner: &Address, app_address: &Address, program_id: &Address) -> (Address, u8) {
   Address::find_program_address(&[USER_VAULT_SEED, owner.as_ref(), app_address.as_ref()], program_id)
}

/// Load program-owned user vault account; verifies PDA address against `owner` + bump.
pub fn load_user_vault(
   program_id: &Address,
   user_vault_pda: &AccountView,
   expected_owner: &Address,
   expected_app_address: &Address,
) -> Result<UserVaultAccount, ProgramError> {
   if unlikely(!user_vault_pda.owned_by(&ID)) {
      log!("user vault: not found");
      return Err(Error::UserVaultNotFound.into());
   }
   let vault = {
      let data = user_vault_pda.try_borrow().map_err(|_| Error::UserVaultNotFound)?;
      if unlikely(data.len() != UserVaultAccount::LEN) {
         log!("user vault: bad account data length");
         return Err(Error::UserVaultNotFound.into());
      }
      UserVaultAccount::unpack(&data).map_err(|e| {
         log!("user vault: unpack failed");
         e
      })?
   };
   if unlikely(vault.owner.as_ref() != expected_owner.as_ref()) {
      log!("user vault: owner mismatch");
      return Err(Error::UserVaultOwnerMismatch.into());
   }
   if unlikely(vault.app_address.as_ref() != expected_app_address.as_ref()) {
      log!("user vault: app address mismatch");
      return Err(Error::UserVaultAppAddressMismatch.into());
   }
   let bump_seed = [vault.bump];
   let expected_addr = Address::create_program_address(
      &[
         USER_VAULT_SEED,
         expected_owner.as_ref(),
         expected_app_address.as_ref(),
         &bump_seed[..],
      ],
      program_id,
   )
   .map_err(|_| ProgramError::InvalidSeeds)?;

   if unlikely(user_vault_pda.address() != &expected_addr) {
      log!("user vault: pda address mismatch");
      return Err(Error::UserVaultPdaMismatch.into());
   }
   Ok(vault)
}

/// Drain `account_to_close` lamports to `recipient` and close (program-owned PDA rent reclaim).
pub fn close_program_account_lamports_to(account_to_close: &AccountView, recipient: &AccountView) -> ProgramResult {
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

/// Empty system-owned account suitable for `CreateAccount` (PDA pre-image).
pub fn assert_pda_uninitialized(account: &AccountView, system_id: &Address) -> Result<(), Error> {
   if unlikely(!account.owned_by(system_id)) {
      log!("pda already exists or wrong owner");
      return Err(Error::UserVaultAlreadyExists);
   }
   if unlikely(!account.is_data_empty() || account.lamports() != 0) {
      log!("pda already exists");
      return Err(Error::UserVaultAlreadyExists);
   }
   Ok(())
}

#[inline]
pub(crate) fn parse_u64_instruction_data(data: &[u8]) -> Result<u64, ProgramError> {
   if unlikely(data.len() != 8) {
      return Err(ProgramError::InvalidInstructionData);
   }
   Ok(u64::from_le_bytes(data.try_into().unwrap()))
}

#[inline]
pub(crate) fn parse_two_u64_instruction_data(data: &[u8]) -> Result<(u64, u64), ProgramError> {
   if unlikely(data.len() != 16) {
      return Err(ProgramError::InvalidInstructionData);
   }
   Ok((u64::from_le_bytes(data[0..8].try_into().unwrap()), u64::from_le_bytes(data[8..16].try_into().unwrap())))
}

#[inline]
pub(crate) fn parse_u32_instruction_data(data: &[u8]) -> Result<u32, ProgramError> {
   if unlikely(data.len() != 4) {
      return Err(ProgramError::InvalidInstructionData);
   }
   Ok(u32::from_le_bytes(data.try_into().unwrap()))
}

#[inline]
pub fn unix_timestamp_to_u32(ts: i64) -> Result<u32, Error> {
   if unlikely(ts <= 0) {
      return Err(Error::InvalidUnixTimestamp);
   }
   if unlikely(ts > u32::MAX as i64) {
      return Err(Error::InvalidUnixTimestamp);
   }
   Ok(ts as u32)
}

#[inline(never)]
pub fn get_time() -> Result<u32, Error> {
   let ts = Clock::get()
      .map_err(|_| Error::InvalidUnixTimestamp)?
      .unix_timestamp;
   unix_timestamp_to_u32(ts)
}

/// `delegate_expires == 0` means no expiry. Otherwise `Clock::unix_timestamp` must be `<= delegate_expires`.
#[inline]
pub fn require_delegate_not_expired(delegate_expires: u32) -> Result<(), Error> {
   if delegate_expires == 0 {
      return Ok(());
   }
   let now = get_time()?;
   if unlikely(now > delegate_expires) {
      log!("delegate authorization expired");
      return Err(Error::ExpiredDelegate);
   }
   Ok(())
}