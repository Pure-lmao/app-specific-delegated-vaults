import type { Address } from '@solana/addresses';

/**
 * Decoded on-chain `UserVaultAccount`.
 * `ataCount` is the raw `u16` from account data (use `number`; max 65535).
 */
export type UserVaultAccountData = Readonly<{
   discriminator: number; // 0
   owner: Address;
   appAddress: Address;
   delegate: Address;
   delegateExpires: number;
   ataCount: number;
   bump: number;
}>;

export type CreateUserVaultInput = Readonly<{
   /** Unix timestamp seconds (u32); use `0xffffffff` for no practical expiry. */
   delegateExpires: number;
}>;

export type DepositUserVaultInput = Readonly<{
   amount: bigint;
}>;

export type WithdrawUserVaultInput = Readonly<{
   amount: bigint;
}>;

export type WithdrawUserVaultNativeInput = Readonly<{
   amount: bigint;
}>;

export type UpdateUserVaultDelegateInput = Readonly<{
   /** Unix timestamp seconds (u32); use `0xffffffff` for no practical expiry. */
   delegateExpires: number;
}>;

/**
 * Inner CPI payload only; vault instruction data is built by {@link ./codex.js}.
 * Must be a real `Uint8Array` (not ArrayBuffer, not a plain object).
 */
export type AppIxInput = Readonly<{
   innerInstructionData: Uint8Array;
}>;

/** Vault `CpiEntry` payload: native lamports from vault PDA first, then SPL token amount (either may be `0n`). */
export type CpiEntryInput = Readonly<{
   amountNative: bigint;
   amount: bigint;
}>;

/** Vault `CpiEntryNative` payload (discriminator `7`). */
export type CpiEntryNativeInput = Readonly<{
   amountNative: bigint;
}>;

/**
 * Full vault program instruction `data` (u8 variant index + payload).
 * Variant order matches `program/src/instructions/mod.rs` `Instruction` enum (indices 0..=9 on-chain).
 */
export type DecodedVaultInstruction =
   | { kind: 'createUserVault'; delegateExpires: number }
   | { kind: 'depositUserVault'; amount: bigint }
   | { kind: 'updateUserVaultDelegate'; delegateExpires: number }
   | { kind: 'withdrawUserVault'; amount: bigint }
   | { kind: 'withdrawUserVaultNative'; amount: bigint }
   | { kind: 'appIx'; innerInstructionData: Uint8Array }
   | { kind: 'cpiEntry'; amountNative: bigint; amount: bigint }
   | { kind: 'cpiEntryNative'; amountNative: bigint }
   | { kind: 'closeVaultAta' }
   | { kind: 'closeUserVault' };
