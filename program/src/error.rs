//! Custom program errors (`ProgramError::Custom`).

use pinocchio::error::ProgramError;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
   NotSigner = 1,
   InvalidAta = 2,
   MintMismatch = 3,
   ArithmeticOverflow = 4,
   InvalidSystemProgram = 5,
   InvalidTokenProgram = 6,
   InvalidAssociatedTokenProgram = 7,
   UserVaultAlreadyExists = 8,
   UserVaultNotFound = 9,
   UserVaultOwnerMismatch = 10,
   UserVaultPdaMismatch = 11,
   UserVaultAppAddressMismatch = 12,
   InvalidUserVaultDelegate = 13,
   UnauthorizedCpiCaller = 14,
   UserVaultAtaNotEmpty = 15,
   UserVaultAtaCountZero = 16,
   UserVaultHasOpenAtas = 17,
   InvalidUnixTimestamp = 18,
   ExpiredDelegate = 19,
}

impl From<Error> for ProgramError {
   fn from(e: Error) -> Self {
      ProgramError::Custom(e as u32)
   }
}
