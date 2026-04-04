//! Off-chain helpers for **the** shared vault program: PDAs, instruction encoding, and `UserVault` account parsing.
//!
//! There is intended to be **one** vault program id per cluster (see [`DEFAULT_VAULT_PROGRAM_ID`]). **Your** on-chain
//! program is passed everywhere as `app_address`; you do not ship a separate vault binary for each product.
//!
//! Pair with `solana-client` or your RPC stack. Discriminators and account order match `program/src/instructions/mod.rs`
//! and `sdk/ts`.
//!
//! # Example
//!
//! ```ignore
//! use solana_keypair::Keypair;
//! use solana_signer::Signer;
//! use vault_sdk::{create_user_vault_ix_default, find_user_vault_pda, DEFAULT_VAULT_PROGRAM_ID};
//!
//! let owner = Keypair::new().pubkey();
//! let app = todo!();
//! let delegate = todo!();
//! let (pda, _bump) = find_user_vault_pda(&DEFAULT_VAULT_PROGRAM_ID, &owner, &app);
//! let ix = create_user_vault_ix_default(&owner, &pda, &app, &delegate, None, 0);
//! ```

#![forbid(unsafe_code)]

mod constants;
mod error;
mod instructions;
mod pda;
mod state;

pub use constants::*;
pub use error::VaultSdkError;
pub use instructions::*;
pub use pda::*;
pub use state::UserVaultAccount;
