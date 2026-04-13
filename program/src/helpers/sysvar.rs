use crate::error::Error;

use core::mem::size_of;

use pinocchio::{
   AccountView, Address, address::address_eq, error::ProgramError, hint::unlikely,
   sysvars::{
      clock::CLOCK_ID,
      instructions::INSTRUCTIONS_ID,
      rent::RENT_ID,
   },
};
use pinocchio_log::log;

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
