use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum VaultSdkError {
   #[error("user vault account data wrong length (expected {expected}, got {got})")]
   WrongAccountDataLen { expected: usize, got: usize },
   #[error("invalid user vault discriminator: expected 0, got {0}")]
   InvalidDiscriminator(u8),
   #[error("amount must be greater than zero")]
   InvalidAmount,
}
