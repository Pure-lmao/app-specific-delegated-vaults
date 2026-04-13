import { getAddressEncoder, type Address } from '@solana/addresses';
import { getBase64Decoder, getBase64Encoder } from '@solana/kit';
import type { GetAccountInfoApi, GetProgramAccountsApi } from '@solana/rpc-api';
import type { Rpc } from '@solana/rpc-spec';
import type { Base64EncodedBytes, Commitment } from '@solana/rpc-types';

import { USER_VAULT_ACCOUNT_DATA_SIZE, USER_VAULT_DISCRIMINATOR, VAULT_PROGRAM_ADDRESS } from './constants';
import { decodeUserVaultAccountData } from './codex';
import type { UserVaultAccountData } from './types';

const base64Encoder = getBase64Encoder();
const base64Decoder = getBase64Decoder();
const addressEncoder = getAddressEncoder();

/** Byte offset of `owner` in serialized `UserVaultAccount` (`#[repr(C)]`). */
export const USER_VAULT_OWNER_MEMCMP_OFFSET = 8;

/** Byte offset of `app_address` in serialized `UserVaultAccount`. */
export const USER_VAULT_APP_MEMCMP_OFFSET = 40;

export type FetchUserVaultAccountConfig = Readonly<{
   commitment?: Commitment;
}>;

export type UserVaultAccountRecord = Readonly<{
   address: Address;
   data: UserVaultAccountData;
}>;

export type FetchUserVaultAccountsConfig = Readonly<{
   commitment?: Commitment;
   /** Defaults to {@link VAULT_PROGRAM_ADDRESS}. */
   vaultProgramAddress?: Address;
}>;

export type FetchUserVaultAccountsFilter = Readonly<{
   owner?: Address;
   appAddress?: Address;
}>;

/** Decode `getAccountInfo` / `getProgramAccounts` account `data` when `encoding: 'base64'`. */
export function decodeRpcBase64AccountDataTuple(data: readonly [string, string]): Uint8Array {
   const [b64, encoding] = data;
   if (encoding !== 'base64') {
      throw new RangeError(`decodeRpcBase64AccountDataTuple: expected base64 account data, got ${encoding}`);
   }
   return new Uint8Array(base64Encoder.encode(b64));
}

function discriminatorMemcmpBytes(): Base64EncodedBytes {
   const raw = new Uint8Array([USER_VAULT_DISCRIMINATOR]);
   return base64Decoder.decode(raw) as Base64EncodedBytes;
}

function pubkeyMemcmpBytes(addr: Address): Base64EncodedBytes {
   const raw = new Uint8Array(addressEncoder.encode(addr));
   return base64Decoder.decode(raw) as Base64EncodedBytes;
}

export async function fetchUserVaultAccount(
   rpc: Rpc<GetAccountInfoApi>,
   accountAddress: Address,
   config?: FetchUserVaultAccountConfig,
): Promise<UserVaultAccountData> {
   const res = await rpc
      .getAccountInfo(accountAddress, { encoding: 'base64', commitment: config?.commitment })
      .send();
   const info = res.value;
   if (!info) {
      throw new Error(`fetchUserVaultAccount: account not found at ${accountAddress}`);
   }
   if (info.owner !== VAULT_PROGRAM_ADDRESS) {
      throw new Error(
         `fetchUserVaultAccount: wrong owner ${info.owner} (expected vault program ${VAULT_PROGRAM_ADDRESS})`,
      );
   }
   if (!('data' in info) || !Array.isArray(info.data)) {
      throw new Error('fetchUserVaultAccount: account info missing raw data');
   }
   const bytes = decodeRpcBase64AccountDataTuple(info.data as [string, string]);
   return decodeUserVaultAccountData(bytes);
}

/**
 * Returns all `UserVault` accounts for the vault program matching optional `owner` and/or `appAddress`
 * memcmp filters (plus fixed size + discriminator filters). Omit both filters to fetch every vault
 * for the program (can be large on busy clusters).
 */
export async function fetchUserVaultAccountsByFilter(
   rpc: Rpc<GetProgramAccountsApi>,
   filter: FetchUserVaultAccountsFilter,
   config?: FetchUserVaultAccountsConfig,
): Promise<readonly UserVaultAccountRecord[]> {
   const programAddress = config?.vaultProgramAddress ?? VAULT_PROGRAM_ADDRESS;
   const filters = [
      { dataSize: BigInt(USER_VAULT_ACCOUNT_DATA_SIZE) },
      {
         memcmp: {
            offset: 0n,
            bytes: discriminatorMemcmpBytes(),
            encoding: 'base64' as const,
         },
      },
   ] as const;
   const extra: { memcmp: { offset: bigint; bytes: Base64EncodedBytes; encoding: 'base64' } }[] = [];
   if (filter.owner) {
      extra.push({
         memcmp: {
            offset: BigInt(USER_VAULT_OWNER_MEMCMP_OFFSET),
            bytes: pubkeyMemcmpBytes(filter.owner),
            encoding: 'base64',
         },
      });
   }
   if (filter.appAddress) {
      extra.push({
         memcmp: {
            offset: BigInt(USER_VAULT_APP_MEMCMP_OFFSET),
            bytes: pubkeyMemcmpBytes(filter.appAddress),
            encoding: 'base64',
         },
      });
   }
   const res = await rpc
      .getProgramAccounts(programAddress, {
         commitment: config?.commitment,
         encoding: 'base64',
         filters: [...filters, ...extra],
      })
      .send();

   return res.map((row): UserVaultAccountRecord => {
      if (!('data' in row.account) || !Array.isArray(row.account.data)) {
         throw new Error('fetchUserVaultAccountsByFilter: account missing base64 data');
      }
      const bytes = decodeRpcBase64AccountDataTuple(row.account.data as [string, string]);
      return { address: row.pubkey, data: decodeUserVaultAccountData(bytes) };
   });
}
