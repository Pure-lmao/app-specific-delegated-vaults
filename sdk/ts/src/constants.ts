import { address } from '@solana/addresses';

/** Deployed vault program id (must match `program/src/constants.rs` `ID`). */
export const VAULT_PROGRAM_ADDRESS = address('ASdvz39AFXXEGcYadGYPYhHprGadw1AeydQPqn4GqrV1');

/** PDA seed segment: `b"vault"` */
export const USER_VAULT_SEED = 'vault' as const;

/** First byte of serialized `UserVaultAccount` */
export const USER_VAULT_DISCRIMINATOR = 1;

/** `UserVaultAccount::LEN` on-chain */
export const USER_VAULT_ACCOUNT_DATA_SIZE = 104;

export const SYSTEM_PROGRAM_ADDRESS = address('11111111111111111111111111111111');
export const SPL_TOKEN_PROGRAM_ADDRESS = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const TOKEN_2022_PROGRAM_ADDRESS = address('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
export const ASSOCIATED_TOKEN_PROGRAM_ADDRESS = address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
/** Sysvar instructions — required for `cpiEntry` / `cpiEntryNative` (delegate expiry + top-level ix checks). */
export const SYSVAR_INSTRUCTIONS_ADDRESS = address('Sysvar1nstructions1111111111111111111111111');
/** Clock sysvar — last account on `cpiEntry` / `cpiEntryNative`. */
export const SYSVAR_CLOCK_ADDRESS = address('SysvarC1ock11111111111111111111111111111111');
/** Rent sysvar — account index 4 on `createUserVault` (read lamports for rent-exempt minimum). */
export const SYSVAR_RENT_ADDRESS = address('SysvarRent111111111111111111111111111111111');
