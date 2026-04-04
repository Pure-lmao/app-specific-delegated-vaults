import type { Address } from '@solana/addresses';
import type { Rpc } from '@solana/rpc-spec';
import type { GetAccountInfoApi } from '@solana/rpc-api';
import type { Commitment } from '@solana/rpc-types';

import { VAULT_PROGRAM_ADDRESS } from './constants';
import { decodeUserVaultAccountData } from './codex';
import type { UserVaultAccountData } from './types';

export type FetchUserVaultAccountConfig = Readonly<{
   commitment?: Commitment;
}>;

function base64DataToBytes(data: readonly [string, string]): Uint8Array {
   const [b64, encoding] = data;
   if (encoding !== 'base64') {
      throw new RangeError(`fetchUserVaultAccount: expected base64 account data, got ${encoding}`);
   }
   const bin = atob(b64);
   const out = new Uint8Array(bin.length);
   for (let i = 0; i < bin.length; i++) {
      out[i] = bin.charCodeAt(i);
   }
   return out;
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
   const bytes = base64DataToBytes(info.data as [string, string]);
   return decodeUserVaultAccountData(bytes);
}
