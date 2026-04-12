/**
 * Encoders and decoders for vault instruction payloads and on-chain account data.
 * Types and layout constants: {@link ./types.js}.
 *
 * @see https://www.solanakit.com/docs/concepts/codecs
 */

import {
   getAddressDecoder,
   getAddressEncoder,
   type Decoder,
   type Encoder,
   getBytesDecoder,
   getBytesEncoder,
   getDiscriminatedUnionDecoder,
   getDiscriminatedUnionEncoder,
   getStructDecoder,
   getStructEncoder,
   getU16Decoder,
   getU16Encoder,
   getU32Decoder,
   getU32Encoder,
   getU64Decoder,
   getU64Encoder,
   getU8Decoder,
   getU8Encoder,
   getUnitDecoder,
   getUnitEncoder,
   type ReadonlyUint8Array,
   transformDecoder,
   transformEncoder,
} from '@solana/kit';

import type { DecodedVaultInstruction, UserVaultAccountData } from './types';

const U64_MAX = 2n ** 64n - 1n;
const U32_MAX = 0xffff_ffff;

const getU64BigintEncoder = getU64Encoder;
const getU64BigintDecoder = getU64Decoder;

export function assertU64Amount(amount: bigint, label = 'amount'): void {
   if (typeof amount !== 'bigint') {
      throw new TypeError(`${label} must be a bigint`);
   }
   if (amount <= 0n) {
      throw new RangeError(`${label} must be > 0`);
   }
   if (amount > U64_MAX) {
      throw new RangeError(`${label} must be <= 2**64-1`);
   }
}

export function assertU32DelegateExpires(n: number, label = 'delegateExpires'): void {
   if (!Number.isInteger(n) || n < 0 || n > U32_MAX) {
      throw new RangeError(`${label} must be an integer in [0, 2**32-1]`);
   }
}

/** `CpiEntry`: at least one of native lamports or SPL amount must be &gt; 0; both fit `u64`. */
export function assertCpiEntryAmounts(amountNative: bigint, amount: bigint, label = 'cpiEntry'): void {
   if (typeof amountNative !== 'bigint' || typeof amount !== 'bigint') {
      throw new TypeError(`${label}: amountNative and amount must be bigints`);
   }
   if (amountNative < 0n || amount < 0n) {
      throw new RangeError(`${label}: amounts must be >= 0`);
   }
   if (amountNative === 0n && amount === 0n) {
      throw new RangeError(`${label}: at least one of amountNative and amount must be > 0`);
   }
   if (amountNative > U64_MAX || amount > U64_MAX) {
      throw new RangeError(`${label}: amounts must be <= 2**64-1`);
   }
}

// --- Full program instruction data (discriminator + payload) ---

type VaultInstructionUnion =
   | { kind: 'createUserVault'; delegateExpires: number }
   | { kind: 'depositUserVault'; amount: bigint }
   | { kind: 'updateUserVaultDelegate'; delegateExpires: number }
   | { kind: 'withdrawUserVault'; amount: bigint }
   | { kind: 'withdrawUserVaultNative'; amount: bigint }
   | { kind: 'appIx'; innerInstructionData: ReadonlyUint8Array }
   | { kind: 'cpiEntry'; amountNative: bigint; amount: bigint }
   | { kind: 'cpiEntryNative'; amountNative: bigint }
   | { kind: 'closeVaultAta' }
   | { kind: 'closeUserVault' };

export const getVaultInstructionDataEncoder = (): Encoder<DecodedVaultInstruction> =>
   transformEncoder(
      getDiscriminatedUnionEncoder(
         [
            ['createUserVault', getStructEncoder([['delegateExpires', getU32Encoder()]])],
            ['depositUserVault', getStructEncoder([['amount', getU64BigintEncoder()]])],
            ['updateUserVaultDelegate', getStructEncoder([['delegateExpires', getU32Encoder()]])],
            ['withdrawUserVault', getStructEncoder([['amount', getU64BigintEncoder()]])],
            ['withdrawUserVaultNative', getStructEncoder([['amount', getU64BigintEncoder()]])],
            ['appIx', getStructEncoder([['innerInstructionData', getBytesEncoder()]])],
            [
               'cpiEntry',
               getStructEncoder([
                  ['amountNative', getU64BigintEncoder()],
                  ['amount', getU64BigintEncoder()],
               ]),
            ],
            ['cpiEntryNative', getStructEncoder([['amountNative', getU64BigintEncoder()]])],
            ['closeVaultAta', getUnitEncoder()],
            ['closeUserVault', getUnitEncoder()],
         ],
         { size: getU8Encoder(), discriminator: 'kind' },
      ),
      (ix: DecodedVaultInstruction): VaultInstructionUnion => {
         switch (ix.kind) {
            case 'createUserVault':
               return { kind: 'createUserVault', delegateExpires: ix.delegateExpires };
            case 'depositUserVault':
               return { kind: 'depositUserVault', amount: ix.amount };
            case 'updateUserVaultDelegate':
               return { kind: 'updateUserVaultDelegate', delegateExpires: ix.delegateExpires };
            case 'withdrawUserVault':
               return { kind: 'withdrawUserVault', amount: ix.amount };
            case 'withdrawUserVaultNative':
               return { kind: 'withdrawUserVaultNative', amount: ix.amount };
            case 'appIx':
               return { kind: 'appIx', innerInstructionData: ix.innerInstructionData };
            case 'cpiEntry':
               return { kind: 'cpiEntry', amountNative: ix.amountNative, amount: ix.amount };
            case 'cpiEntryNative':
               return { kind: 'cpiEntryNative', amountNative: ix.amountNative };
            case 'closeVaultAta':
               return { kind: 'closeVaultAta' };
            case 'closeUserVault':
               return { kind: 'closeUserVault' };
         }
      },
   );

export const getVaultInstructionDataDecoder = (): Decoder<DecodedVaultInstruction> =>
   transformDecoder(
      getDiscriminatedUnionDecoder(
         [
            ['createUserVault', getStructDecoder([['delegateExpires', getU32Decoder()]])],
            ['depositUserVault', getStructDecoder([['amount', getU64BigintDecoder()]])],
            ['updateUserVaultDelegate', getStructDecoder([['delegateExpires', getU32Decoder()]])],
            ['withdrawUserVault', getStructDecoder([['amount', getU64BigintDecoder()]])],
            ['withdrawUserVaultNative', getStructDecoder([['amount', getU64BigintDecoder()]])],
            ['appIx', getStructDecoder([['innerInstructionData', getBytesDecoder()]])],
            [
               'cpiEntry',
               getStructDecoder([
                  ['amountNative', getU64BigintDecoder()],
                  ['amount', getU64BigintDecoder()],
               ]),
            ],
            ['cpiEntryNative', getStructDecoder([['amountNative', getU64BigintDecoder()]])],
            ['closeVaultAta', getUnitDecoder()],
            ['closeUserVault', getUnitDecoder()],
         ],
         { size: getU8Decoder(), discriminator: 'kind' },
      ),
      (v: VaultInstructionUnion): DecodedVaultInstruction => {
         switch (v.kind) {
            case 'createUserVault':
               return { kind: 'createUserVault', delegateExpires: v.delegateExpires };
            case 'depositUserVault':
               return { kind: 'depositUserVault', amount: v.amount };
            case 'updateUserVaultDelegate':
               return { kind: 'updateUserVaultDelegate', delegateExpires: v.delegateExpires };
            case 'withdrawUserVault':
               return { kind: 'withdrawUserVault', amount: v.amount };
            case 'withdrawUserVaultNative':
               return { kind: 'withdrawUserVaultNative', amount: v.amount };
            case 'appIx':
               return {
                  kind: 'appIx',
                  innerInstructionData: new Uint8Array(v.innerInstructionData),
               };
            case 'cpiEntry':
               return { kind: 'cpiEntry', amountNative: v.amountNative, amount: v.amount };
            case 'cpiEntryNative':
               return { kind: 'cpiEntryNative', amountNative: v.amountNative };
            case 'closeVaultAta':
               return { kind: 'closeVaultAta' };
            case 'closeUserVault':
               return { kind: 'closeUserVault' };
         }
      },
   );

// --- User vault account ---

export const getUserVaultAccountEncoder = (): Encoder<UserVaultAccountData> =>
   getStructEncoder([
      ['discriminator', getU8Encoder()],
      ['bump', getU8Encoder()],
      ['ataCount', getU16Encoder()],
      ['delegateExpires', getU32Encoder()],
      ['owner', getAddressEncoder()],
      ['appAddress', getAddressEncoder()],
      ['delegate', getAddressEncoder()],
   ]);

export const getUserVaultAccountDecoder = (): Decoder<UserVaultAccountData> =>
   getStructDecoder([
      ['discriminator', getU8Decoder()],
      ['bump', getU8Decoder()],
      ['ataCount', getU16Decoder()],
      ['delegateExpires', getU32Decoder()],
      ['owner', getAddressDecoder()],
      ['appAddress', getAddressDecoder()],
      ['delegate', getAddressDecoder()],
   ]);

export const decodeUserVaultAccountData = (data: ReadonlyUint8Array): UserVaultAccountData =>
   getUserVaultAccountDecoder().decode(data);
