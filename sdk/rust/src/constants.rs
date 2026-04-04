//! Well-known addresses and layout constants (aligned with `program/src/constants.rs` and the TypeScript SDK).

use solana_pubkey::pubkey;
pub use solana_pubkey::Pubkey;

/// PDA seed segment: `b"vault"`.
pub const USER_VAULT_SEED: &[u8] = b"vault";

/// First byte of serialized on-chain `UserVaultAccount`.
pub const USER_VAULT_DISCRIMINATOR: u8 = 0;

/// Default deployed vault program id (matches `sdk/ts/src/constants.ts` `VAULT_PROGRAM_ADDRESS`).
pub const DEFAULT_VAULT_PROGRAM_ID: Pubkey = pubkey!("ASdvz39AFXXEGcYadGYPYhHprGadw1AeydQPqn4GqrV1");

pub const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
/// Instructions sysvar (`Sysvar1nstructions…`) — last-but-one account on `CpiEntry` / `CpiEntryNative`.
pub const SYSVAR_INSTRUCTIONS_ID: Pubkey = pubkey!("Sysvar1nstructions1111111111111111111111111");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey = pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const SPL_TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const SPL_TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
