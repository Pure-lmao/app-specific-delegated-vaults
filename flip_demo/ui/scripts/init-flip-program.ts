/**
 * Sends flip program `init` (discriminator `0`, no extra data): creates config PDA + pool USDC ATA.
 *
 * Usage (from `flip_demo/ui`):
 *   npx tsx scripts/init-flip-program.ts [path-to-payer-keypair.json]
 *
 * Default keypair: `SOLANA_KEYPAIR` env, else `flip_demo/program/deployer_keypair.json`
 * (i.e. `../../program/deployer_keypair.json` from this script).
 */
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { getProgramDerivedAddress } from '@solana/addresses';
import { assertIsSignature } from '@solana/keys';
import {
   AccountRole,
   address,
   appendTransactionMessageInstruction,
   createSolanaRpc,
   createTransactionMessage,
   devnet,
   getBase64EncodedWireTransaction,
   pipe,
   setTransactionMessageFeePayerSigner,
   setTransactionMessageLifetimeUsingBlockhash,
   signTransactionMessageWithSigners,
} from '@solana/kit';
import { createKeyPairSignerFromBytes } from '@solana/signers';
import {
   ASSOCIATED_TOKEN_PROGRAM_ADDRESS,
   deriveAtaAddress,
   SPL_TOKEN_PROGRAM_ADDRESS,
   SYSVAR_RENT_ADDRESS,
   SYSTEM_PROGRAM_ADDRESS,
} from '@ASDV/sdk';

/** Must match `flip_demo/program/src/constants.rs` `ID` for the deployed program. */
const FLIP_PROGRAM_ADDRESS = address('BPppJw92aJEF4PC1jdVWFGSc3SJCCH6BJveDpu786S7V');
const DEVNET_USDC_MINT = address('Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr');
const CONFIG_SEED = new TextEncoder().encode('config');
const INIT_DISCRIMINATOR = 0;

const DEFAULT_RPC = 'https://api.devnet.solana.com';

const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const DEFAULT_DEPLOYER_KEYPAIR = resolve(SCRIPT_DIR, '../../../program/deployer_keypair.json');

function payerKeypairPath(): string {
   const fromArgv = process.argv[2];
   if (fromArgv) {
      return resolve(fromArgv);
   }
   const fromEnv = process.env.SOLANA_KEYPAIR;
   if (fromEnv) {
      return resolve(fromEnv);
   }
   return DEFAULT_DEPLOYER_KEYPAIR;
}

async function main(): Promise<void> {
   const rpcUrl = process.env.FLIP_RPC_URL ?? DEFAULT_RPC;
   const rpc = createSolanaRpc(devnet(rpcUrl));

   const raw = JSON.parse(readFileSync(payerKeypairPath(), 'utf8')) as number[];
   if (!Array.isArray(raw) || raw.length !== 64) {
      throw new Error('Keypair JSON must be a 64-number array (Solana CLI format).');
   }
   const payer = await createKeyPairSignerFromBytes(new Uint8Array(raw));

   const [configPda] = await getProgramDerivedAddress({
      programAddress: FLIP_PROGRAM_ADDRESS,
      seeds: [CONFIG_SEED],
   });
   const poolAta = await deriveAtaAddress(configPda, DEVNET_USDC_MINT, SPL_TOKEN_PROGRAM_ADDRESS);

   const ix = {
      programAddress: FLIP_PROGRAM_ADDRESS,
      accounts: [
         { address: payer.address, role: AccountRole.WRITABLE_SIGNER },
         { address: configPda, role: AccountRole.WRITABLE },
         { address: poolAta, role: AccountRole.WRITABLE },
         { address: DEVNET_USDC_MINT, role: AccountRole.READONLY },
         { address: SYSTEM_PROGRAM_ADDRESS, role: AccountRole.READONLY },
         { address: SPL_TOKEN_PROGRAM_ADDRESS, role: AccountRole.READONLY },
         { address: ASSOCIATED_TOKEN_PROGRAM_ADDRESS, role: AccountRole.READONLY },
         { address: SYSVAR_RENT_ADDRESS, role: AccountRole.READONLY },
      ] as const,
      data: new Uint8Array([INIT_DISCRIMINATOR]),
   };

   const { value: lb } = await rpc.getLatestBlockhash().send();
   const msg = pipe(
      createTransactionMessage({ version: 0 }),
      (m) => setTransactionMessageFeePayerSigner(payer, m),
      (m) =>
         setTransactionMessageLifetimeUsingBlockhash(
            { blockhash: lb.blockhash, lastValidBlockHeight: lb.lastValidBlockHeight },
            m,
         ),
      (m) => appendTransactionMessageInstruction(ix, m),
   );
   const signed = await signTransactionMessageWithSigners(msg);
   const wire = getBase64EncodedWireTransaction(signed);

   const { value: sim } = await rpc
      .simulateTransaction(wire, {
         encoding: 'base64',
         sigVerify: true,
         commitment: 'confirmed',
         innerInstructions: true,
      })
      .send();
   if (sim.err != null) {
      console.error('simulateTransaction failed', {
         err: sim.err,
         logs: sim.logs,
         unitsConsumed: sim.unitsConsumed,
         returnData: sim.returnData,
      });
      if (sim.logs?.length) {
         console.error('simulation logs:\n' + sim.logs.join('\n'));
      }
      throw new Error(`Simulation failed: ${JSON.stringify(sim.err)}`);
   }

   const sigStr = await rpc.sendTransaction(wire, { encoding: 'base64' }).send();
   assertIsSignature(sigStr);
   console.log('init ok', sigStr);
   console.log('config PDA', String(configPda));
   console.log('pool ATA', String(poolAta));
}

main().catch((e) => {
   console.error(e);
   process.exitCode = 1;
});
