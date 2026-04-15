import { getAddressEncoder, getProgramDerivedAddress, type Address } from '@solana/addresses';
import { AccountRole, type AccountMeta, type Instruction } from '@solana/instructions';

import {
   ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
   SYSTEM_PROGRAM_ADDRESS,
   SYSVAR_CLOCK_ADDRESS,
   SYSVAR_INSTRUCTIONS_ADDRESS,
   SYSVAR_RENT_ADDRESS,
   USER_VAULT_SEED,
   VAULT_PROGRAM_ADDRESS,
} from './constants';
import {
   assertCpiEntryAmounts,
   assertU32DelegateExpires,
   assertU64Amount,
   getVaultInstructionDataEncoder,
} from './codex';
import type {
   AppIxInput,
   CpiEntryInput,
   CpiEntryNativeInput,
   CreateUserVaultInput,
   DecodedVaultInstruction,
   DepositUserVaultInput,
   UpdateUserVaultDelegateInput,
   WithdrawUserVaultInput,
   WithdrawUserVaultNativeInput,
} from './types';

/**
 * `cpi_entry` / `cpi_entry_native` are invoked via CPI from the registered app program (instructions + clock sysvars).
 * Builders match `cpi_entry.rs` / `cpi_entry_native.rs`.
 * `app_ix` matches `app_ix.rs`: five fixed accounts (ending with clock sysvar), then inner metas.
 */

const textEncoder = new TextEncoder();

function encodeVaultIx(ix: DecodedVaultInstruction): Uint8Array {
   return new Uint8Array(getVaultInstructionDataEncoder().encode(ix));
}

export async function deriveUserVaultPda(
   programAddress: Address,
   owner: Address,
   appAddress: Address,
): Promise<readonly [Address, number]> {
   const addrEnc = getAddressEncoder();
   const [pda, bump] = await getProgramDerivedAddress({
      programAddress,
      seeds: [textEncoder.encode(USER_VAULT_SEED), addrEnc.encode(owner), addrEnc.encode(appAddress)],
   });
   return [pda, bump] as const;
}

/** SPL associated token address for wallet = `userVaultPda`, explicit classic or Token-2022 program. */
export async function deriveUserVaultAtaAddress(
   userVaultPda: Address,
   mint: Address,
   tokenProgram: Address,
): Promise<Address> {
   const addrEnc = getAddressEncoder();
   const [ata] = await getProgramDerivedAddress({
      programAddress: ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
      seeds: [addrEnc.encode(userVaultPda), addrEnc.encode(tokenProgram), addrEnc.encode(mint)],
   });
   return ata;
}

/** SPL ATA for a wallet owner (same PDA layout as the vault ATA, but authority = `owner`). */
export async function deriveAtaAddress(
   owner: Address,
   mint: Address,
   tokenProgram: Address,
): Promise<Address> {
   const addrEnc = getAddressEncoder();
   const [ata] = await getProgramDerivedAddress({
      programAddress: ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
      seeds: [addrEnc.encode(owner), addrEnc.encode(tokenProgram), addrEnc.encode(mint)],
   });
   return ata;
}

export function getCreateUserVaultInstruction(input: {
   accounts: Readonly<{
      owner: Address;
      userVaultPda: Address;
      appAddress: Address;
      delegate: Address;
   }>;
   data: CreateUserVaultInput;
}): Instruction {
   assertU32DelegateExpires(input.data.delegateExpires, 'createUserVault delegateExpires');
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.owner, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.userVaultPda, role: AccountRole.WRITABLE },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
         { address: input.accounts.delegate, role: AccountRole.READONLY },
         { address: SYSVAR_RENT_ADDRESS, role: AccountRole.READONLY },
         { address: SYSTEM_PROGRAM_ADDRESS, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({
         kind: 'createUserVault',
         delegateExpires: input.data.delegateExpires,
      }),
   };
}

export function getDepositUserVaultInstruction(input: {
   accounts: Readonly<{
      owner: Address;
      userVaultPda: Address;
      userVaultAta: Address;
      appAddress: Address;
      sourceAta: Address;
      mint: Address;
      tokenProgram: Address;
   }>;
   data: DepositUserVaultInput;
}): Instruction {
   assertU64Amount(input.data.amount, 'deposit amount');
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.owner, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.userVaultPda, role: AccountRole.WRITABLE },
         { address: input.accounts.userVaultAta, role: AccountRole.WRITABLE },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
         { address: input.accounts.sourceAta, role: AccountRole.WRITABLE },
         { address: input.accounts.mint, role: AccountRole.READONLY },
         { address: SYSTEM_PROGRAM_ADDRESS, role: AccountRole.READONLY },
         { address: input.accounts.tokenProgram, role: AccountRole.READONLY },
         { address: ASSOCIATED_TOKEN_PROGRAM_ADDRESS, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({ kind: 'depositUserVault', amount: input.data.amount }),
   };
}

export function getUpdateUserVaultDelegateInstruction(input: {
   accounts: Readonly<{
      owner: Address;
      userVaultPda: Address;
      appAddress: Address;
      delegate: Address;
   }>;
   data: UpdateUserVaultDelegateInput;
}): Instruction {
   assertU32DelegateExpires(input.data.delegateExpires, 'updateUserVaultDelegate delegateExpires');
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.owner, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.userVaultPda, role: AccountRole.WRITABLE },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
         { address: input.accounts.delegate, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({
         kind: 'updateUserVaultDelegate',
         delegateExpires: input.data.delegateExpires,
      }),
   };
}

/** Matches `withdraw_user_vault.rs` account order (7 accounts). */
export function getWithdrawUserVaultInstruction(input: {
   accounts: Readonly<{
      owner: Address;
      userVaultPda: Address;
      userVaultAta: Address;
      appAddress: Address;
      destAta: Address;
      mint: Address;
      tokenProgram: Address;
   }>;
   data: WithdrawUserVaultInput;
}): Instruction {
   assertU64Amount(input.data.amount, 'withdraw amount');
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.owner, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.userVaultPda, role: AccountRole.READONLY },
         { address: input.accounts.userVaultAta, role: AccountRole.WRITABLE },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
         { address: input.accounts.destAta, role: AccountRole.WRITABLE },
         { address: input.accounts.mint, role: AccountRole.READONLY },
         { address: input.accounts.tokenProgram, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({ kind: 'withdrawUserVault', amount: input.data.amount }),
   };
}

/** Matches `withdraw_user_vault_native.rs`: owner receives lamports from the vault PDA (3 accounts). */
export function getWithdrawUserVaultNativeInstruction(input: {
   accounts: Readonly<{
      owner: Address;
      userVaultPda: Address;
      appAddress: Address;
   }>;
   data: WithdrawUserVaultNativeInput;
}): Instruction {
   assertU64Amount(input.data.amount, 'withdrawUserVaultNative amount');
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.owner, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.userVaultPda, role: AccountRole.WRITABLE },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({ kind: 'withdrawUserVaultNative', amount: input.data.amount }),
   };
}

export function getAppIxInstruction(input: {
   accounts: Readonly<{
      delegate: Address;
      owner: Address;
      userVaultPda: Address;
      appAddress: Address;
   }>;
   innerAccounts: readonly AccountMeta[];
   data: AppIxInput;
}): Instruction {
   if (!(input.data.innerInstructionData instanceof Uint8Array)) {
      throw new TypeError('getAppIxInstruction: innerInstructionData must be a Uint8Array');
   }
   const fixed: AccountMeta[] = [
      { address: input.accounts.delegate, role: AccountRole.WRITABLE_SIGNER },
      { address: input.accounts.owner, role: AccountRole.READONLY },
      { address: input.accounts.userVaultPda, role: AccountRole.WRITABLE },
      { address: input.accounts.appAddress, role: AccountRole.READONLY },
      { address: SYSVAR_CLOCK_ADDRESS, role: AccountRole.READONLY },
   ];
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [...fixed, ...input.innerAccounts],
      data: encodeVaultIx({
         kind: 'appIx',
         innerInstructionData: input.data.innerInstructionData,
      }),
   };
}

/**
 * Vault `CpiEntry` — 11 accounts (`program/src/instructions/cpi_entry.rs`).
 * Ends with instructions sysvar + clock sysvar. Writable flags follow `amountNative` / `amount`.
 */
export function getCpiEntryInstruction(input: {
   accounts: Readonly<{
      delegate: Address;
      owner: Address;
      userVaultPda: Address;
      userVaultAta: Address;
      appAddress: Address;
      lamportsDest: Address;
      destAta: Address;
      mint: Address;
      tokenProgram: Address;
   }>;
   data: CpiEntryInput;
}): Instruction {
   assertCpiEntryAmounts(input.data.amountNative, input.data.amount);
   const wNative = input.data.amountNative > 0n;
   const wSpl = input.data.amount > 0n;
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.delegate, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.owner, role: AccountRole.READONLY },
         {
            address: input.accounts.userVaultPda,
            role: AccountRole.WRITABLE,
         },
         {
            address: input.accounts.userVaultAta,
            role: wSpl ? AccountRole.WRITABLE : AccountRole.READONLY,
         },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
         {
            address: input.accounts.lamportsDest,
            role: wNative ? AccountRole.WRITABLE : AccountRole.READONLY,
         },
         {
            address: input.accounts.destAta,
            role: wSpl ? AccountRole.WRITABLE : AccountRole.READONLY,
         },
         { address: input.accounts.mint, role: AccountRole.READONLY },
         { address: input.accounts.tokenProgram, role: AccountRole.READONLY },
         { address: SYSVAR_INSTRUCTIONS_ADDRESS, role: AccountRole.READONLY },
         { address: SYSVAR_CLOCK_ADDRESS, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({
         kind: 'cpiEntry',
         amountNative: input.data.amountNative,
         amount: input.data.amount,
      }),
   };
}

/** Vault `CpiEntryNative` — 7 accounts (`program/src/instructions/cpi_entry_native.rs`; instructions + clock sysvars). */
export function getCpiEntryNativeInstruction(input: {
   accounts: Readonly<{
      delegate: Address;
      owner: Address;
      userVaultPda: Address;
      appAddress: Address;
      lamportsDest: Address;
   }>;
   data: CpiEntryNativeInput;
}): Instruction {
   assertU64Amount(input.data.amountNative, 'cpiEntryNative amountNative');
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.delegate, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.owner, role: AccountRole.READONLY },
         { address: input.accounts.userVaultPda, role: AccountRole.WRITABLE },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
         { address: input.accounts.lamportsDest, role: AccountRole.WRITABLE },
         { address: SYSVAR_INSTRUCTIONS_ADDRESS, role: AccountRole.READONLY },
         { address: SYSVAR_CLOCK_ADDRESS, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({
         kind: 'cpiEntryNative',
         amountNative: input.data.amountNative,
      }),
   };
}

export function getCloseVaultAtaInstruction(input: {
   accounts: Readonly<{
      owner: Address;
      userVaultPda: Address;
      appAddress: Address;
      userVaultAta: Address;
      destination: Address;
      mint: Address;
      tokenProgram: Address;
   }>;
}): Instruction {
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.owner, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.userVaultPda, role: AccountRole.WRITABLE },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
         { address: input.accounts.userVaultAta, role: AccountRole.WRITABLE },
         { address: input.accounts.destination, role: AccountRole.WRITABLE },
         { address: input.accounts.mint, role: AccountRole.READONLY },
         { address: input.accounts.tokenProgram, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({ kind: 'closeVaultAta' }),
   };
}

export function getCloseUserVaultInstruction(input: {
   accounts: Readonly<{
      owner: Address;
      userVaultPda: Address;
      appAddress: Address;
   }>;
}): Instruction {
   return {
      programAddress: VAULT_PROGRAM_ADDRESS,
      accounts: [
         { address: input.accounts.owner, role: AccountRole.WRITABLE_SIGNER },
         { address: input.accounts.userVaultPda, role: AccountRole.WRITABLE },
         { address: input.accounts.appAddress, role: AccountRole.READONLY },
      ] as const satisfies readonly AccountMeta[],
      data: encodeVaultIx({ kind: 'closeUserVault' }),
   };
}

export { SPL_TOKEN_PROGRAM_ADDRESS, SYSTEM_PROGRAM_ADDRESS, TOKEN_2022_PROGRAM_ADDRESS } from './constants';
