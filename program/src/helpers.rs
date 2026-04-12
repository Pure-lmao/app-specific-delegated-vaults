use crate::{
   constants::USER_VAULT_SEED,
   error::Error,
   state::UserVaultAccount, constants::ID,
};

use core::mem::size_of;
use core::ptr::read;

use pinocchio::{
   AccountView, Address, ProgramResult, account::Ref, address::address_eq, cpi::{Signer, invoke_signed}, error::ProgramError, hint::unlikely, instruction::{InstructionAccount, InstructionView}, sysvars::{
      clock::CLOCK_ID, instructions::INSTRUCTIONS_ID, rent::RENT_ID
   }
};

use pinocchio_associated_token_account::ID as ASSOCIATED_TOKEN_PROGRAM_ID;
use pinocchio_log::log;
use pinocchio_system::ID as SYSTEM_PROGRAM_ID;
use pinocchio_token::{
   ID as SPL_TOKEN_PROGRAM_ID,
   state::{Account as SplTokenAccountState, Mint},
};
use pinocchio_token_2022::{ID as TOKEN_2022_PROGRAM_ID, instructions::TransferChecked as TransferChecked2022};

#[inline]
pub fn require_signer(account: &AccountView) -> Result<(), Error> {
   if unlikely(!account.is_signer()) {
      log!("account is not a signer");
      return Err(Error::NotSigner);
   }
   Ok(())
}


const IX_ACCOUNT_META_STRIDE: usize = 33;

#[inline(always)]
unsafe fn read_u16_le_bytes(p: *const u8) -> u16 {
   u16::from_le_bytes([*p, *p.add(1)])
}


pub fn require_top_level_instruction_is_app(
   instructions_sysvar: &AccountView,
   app_address: &AccountView,
) -> Result<(), ProgramError> {
   if unlikely(!address_eq(instructions_sysvar.address(), &INSTRUCTIONS_ID)) {
      log!("cpi_entry: bad instructions sysvar (unsafe introspection path)");
      return Err(ProgramError::UnsupportedSysvar);
   }
   let p = instructions_sysvar.data_ptr();
   let len = instructions_sysvar.data_len();
   let current = unsafe { read_u16_le_bytes(p.add(len - size_of::<u16>())) } as usize;
   let ix_start = unsafe {
      read_u16_le_bytes(p.add(size_of::<u16>() + current * size_of::<u16>()))
   } as usize;
   let raw = unsafe { p.add(ix_start) };
   let num_accounts = unsafe { read_u16_le_bytes(raw) } as usize;
   let prog_ptr = unsafe { raw.add(size_of::<u16>() + num_accounts * IX_ACCOUNT_META_STRIDE) } as *const Address;
   let program_id = unsafe { &*prog_ptr };
   if unlikely(!address_eq(program_id, app_address.address())) {
      log!("cpi_entry: must be invoked via CPI from the app program");
      return Err(Error::UnauthorizedCpiCaller.into());
   }
   Ok(())
}

/// Classic SPL Token or Token-2022 program id.
#[inline]
pub fn assert_spl_token_program(program: &AccountView) -> Result<(), Error> {
   let id = program.address();
   if unlikely(
      !address_eq(id, &SPL_TOKEN_PROGRAM_ID) && 
      !address_eq(id, &TOKEN_2022_PROGRAM_ID)
   ) {
      log!("invalid spl/token-2022 token program");
      return Err(Error::InvalidTokenProgram);
   }
   Ok(())
}

/// Borrow the base SPL token account layout; account must be owned by `token_program` (supports extension accounts).
#[inline]
pub fn borrow_token_account<'a>(
   account: &'a AccountView,
   token_program: &AccountView,
) -> Result<Ref<'a, SplTokenAccountState>, Error> {
   if unlikely(!address_eq(account.owner(), token_program.address())) {
      log!("token account: owner program mismatch");
      return Err(Error::InvalidAta);
   }
   let data = account.try_borrow().map_err(|_| Error::InvalidAta)?;
   if data.len() < SplTokenAccountState::LEN {
      log!("token account: data too short");
      return Err(Error::InvalidAta);
   }
   Ok(Ref::map(data, |d| unsafe {
      SplTokenAccountState::from_bytes_unchecked(&d[..SplTokenAccountState::LEN])
   }))
}

#[inline]
pub fn assert_system_program(program: &AccountView) -> Result<(), Error> {
   if unlikely(!address_eq(program.address(), &SYSTEM_PROGRAM_ID)) {
      log!("invalid system program");
      return Err(Error::InvalidSystemProgram);
   }
   Ok(())
}

#[inline]
pub fn assert_associated_token_program(program: &AccountView) -> Result<(), Error> {
   if unlikely(!address_eq(program.address(), &ASSOCIATED_TOKEN_PROGRAM_ID)) {
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
   if unlikely(!address_eq(token_account.mint(), expected_mint)) {
      log!("token account mint mismatch");
      return Err(Error::MintMismatch);
   }
   if unlikely(!address_eq(token_account.owner(), expected_owner)) {
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
   if !address_eq(account.owner(), token_program.address()) {
      return Ok(false);
   }
   let data = match account.try_borrow() {
      Ok(d) => d,
      Err(_) => return Ok(false),
   };
   if unlikely(data.len() < SplTokenAccountState::LEN) {
      return Ok(false);
   }
   let token_account = unsafe { SplTokenAccountState::from_bytes_unchecked(&data[..SplTokenAccountState::LEN]) };
   if !token_account.is_initialized() {
      return Ok(false);
   }
   if unlikely(!address_eq(token_account.mint(), mint)) {
      log!("vault ata: mint mismatch on existing account");
      return Err(Error::MintMismatch);
   }
   if unlikely(!address_eq(token_account.owner(), wallet)) {
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
   if unlikely(!address_eq(token_account.mint(), expected_mint)) {
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
   if unlikely(!address_eq(mint.owner(), token_program.address())) {
      log!("mint: owner program mismatch");
      return Err(Error::InvalidAta);
   }
   let len = mint.data_len();
   if unlikely(len < Mint::LEN) {
      log!("mint: data too short");
      return Err(Error::InvalidAta);
   }
   let ptr = mint.data_ptr();
   let bytes = unsafe { core::slice::from_raw_parts(ptr, Mint::LEN) };
   let m = unsafe { Mint::from_bytes_unchecked(bytes) };
   if unlikely(!m.is_initialized()) {
      log!("mint: not initialized");
      return Err(Error::InvalidAta);
   }
   Ok(m.decimals())
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
   let vault_owner = Address::new_from_array(unsafe {
      read(user_vault_pda.data_ptr().add(8) as *const [u8; 32])
   });
   if unlikely(!address_eq(owner, &vault_owner)) {
      log!("user vault: owner does not match");
      return Err(Error::UserVaultOwnerMismatch.into());
   }
   let vault_app_address = Address::new_from_array(unsafe {
      read(user_vault_pda.data_ptr().add(40) as *const [u8; 32])
   });
   if unlikely(!address_eq(app_address, &vault_app_address)) {
      log!("user vault: app address does not match");
      return Err(Error::UserVaultAppAddressMismatch.into());
   }
   Ok(())
}

/// SPL mint base layout: `decimals` byte at offset 44 (after mint_authority + supply).
#[inline]
pub fn mint_base_decimals(mint: &AccountView) -> Result<u8, ProgramError> {
   if unlikely(mint.data_len() < 45) {
      return Err(ProgramError::InvalidAccountData);
   }
   Ok(unsafe { *mint.data_ptr().add(44) })
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
   let vault_delegate = Address::new_from_array(unsafe {
      read(user_vault_pda.data_ptr().add(72) as *const [u8; 32])
   });
   if unlikely(!address_eq(delegate, &vault_delegate)) {
      log!("user vault: delegate does not match");
      return Err(Error::InvalidUserVaultDelegate);
   }
   Ok(())
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

#[inline]
pub fn require_delegate_not_expired(delegate_expires: u32, clock_sysvar: &AccountView) -> Result<(), Error> {
   verify_clock_account(clock_sysvar)?;
   let now_i64 = unsafe { *(clock_sysvar.data_ptr().add(32) as *const i64) };
   if unlikely(unix_timestamp_to_u32(now_i64)? > delegate_expires) {
      log!("delegate authorization expired");
      return Err(Error::ExpiredDelegate);
   }
   Ok(())
}

#[inline]
pub fn verify_clock_account(clock_account: &AccountView) -> Result<(), Error> {
   if unlikely(!address_eq(clock_account.address(), &CLOCK_ID)) {
      log!("clock account: not found");
      return Err(Error::InvalidClockAccount);
   }
   Ok(())

}

#[inline]
pub fn verify_rent_account(rent_account: &AccountView) -> Result<(), Error> {
   if unlikely(!address_eq(rent_account.address(), &RENT_ID)) {
      log!("rent account: not found");
      return Err(Error::InvalidRentAccount);
   }
   Ok(())
}

/// Rent-exempt minimum lamports from the serialized Rent sysvar (`lamports_per_byte` at LE offset 0).
#[inline]
pub fn rent_minimum_balance_from_sysvar(rent_sysvar: &AccountView, data_len: usize) -> Result<u64, ProgramError> {
   verify_rent_account(rent_sysvar)?;
   let rent = unsafe { *(rent_sysvar.data_ptr() as *const u64) };

   Ok(rent * (data_len as u64 + 128))
}
