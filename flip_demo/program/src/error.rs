//! Custom program errors (`ProgramError::Custom`).

use pinocchio::error::ProgramError;

#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
   NotSigner = 1,
   InvalidTokenProgram = 2,
   InvalidAssociatedTokenProgram = 3,
   InvalidSystemProgram = 4,
   ConfigPdaMismatch = 5,
   AlreadyInitialized = 6,
   MintMismatch = 7,
   TokenOwnerMismatch = 8,
   Inactive = 10,
   BadInstructionData = 11,
   InvalidClock = 12,
   NotInitialized = 13,
   VaultProgramMismatch = 14,
   AppAddressMismatch = 15,
}

impl From<Error> for ProgramError {
   fn from(e: Error) -> Self {
      ProgramError::Custom(e as u32)
   }
}
