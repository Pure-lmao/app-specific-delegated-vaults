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
   TreasuryPdaMismatch = 8,
   AuthorityMismatch = 9,
}

impl From<Error> for ProgramError {
   fn from(e: Error) -> Self {
      ProgramError::Custom(e as u32)
   }
}
