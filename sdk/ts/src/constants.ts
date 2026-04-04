import { address } from '@solana/addresses';

/** Deployed vault program id (must match `program/src/constants.rs` `ID`). */
export const VAULT_PROGRAM_ADDRESS = address('ASdvz39AFXXEGcYadGYPYhHprGadw1AeydQPqn4GqrV1');

/** PDA seed segment: `b"vault"` */
export const USER_VAULT_SEED = 'vault' as const;

/** First byte of serialized `UserVaultAccount` */
export const USER_VAULT_DISCRIMINATOR = 0;

/** `UserVaultAccount::LEN` on-chain */
export const USER_VAULT_ACCOUNT_DATA_SIZE = 104;

export const SYSTEM_PROGRAM_ADDRESS = address('11111111111111111111111111111111');
export const SPL_TOKEN_PROGRAM_ADDRESS = address('TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA');
export const TOKEN_2022_PROGRAM_ADDRESS = address('TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb');
export const ASSOCIATED_TOKEN_PROGRAM_ADDRESS = address('ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL');
/** Sysvar instructions — useful when documenting CPI entry / loader flows; not used by the Bun scripts. */
export const SYSVAR_INSTRUCTIONS_ADDRESS = address('Sysvar1nstructions1111111111111111111111111');
