import {
   ConnectorClient,
   createKitTransactionSigner,
   createTransactionSigner,
   getDefaultConfig,
   isConnected,
   type WalletConnectorMetadata,
} from '@solana/connector/headless';
import {
   AccountRole,
   address,
   appendTransactionMessageInstruction,
   createSolanaRpc,
   createTransactionMessage,
   devnet,
   getBase64EncodedWireTransaction,
   lamports,
   pipe,
   setTransactionMessageFeePayerSigner,
   setTransactionMessageLifetimeUsingBlockhash,
   signTransactionMessageWithSigners,
   type Address,
   type Instruction,
} from '@solana/kit';
import { getProgramDerivedAddress } from '@solana/addresses';
import { assertIsSignature, type Signature } from '@solana/keys';
import {
   addSignersToInstruction,
   createKeyPairSignerFromBytes,
   createKeyPairSignerFromPrivateKeyBytes,
   type KeyPairSigner,
} from '@solana/signers';

import {
   deriveAtaAddress,
   deriveUserVaultAtaAddress,
   deriveUserVaultPda,
   getAppIxInstruction,
   getCreateUserVaultInstruction,
   getDepositUserVaultInstruction,
   getUpdateUserVaultDelegateInstruction,
   SPL_TOKEN_PROGRAM_ADDRESS,
   SYSVAR_CLOCK_ADDRESS,
   SYSVAR_INSTRUCTIONS_ADDRESS,
   VAULT_PROGRAM_ADDRESS,
} from '@ASDV/sdk';

/** Matches `flip_demo/program/src/constants.rs` `ID` after deploy. */
const FLIP_PROGRAM_ADDRESS = address('BPppJw92aJEF4PC1jdVWFGSc3SJCCH6BJveDpu786S7V');
const DEVNET_USDC_MINT = address('Gh9ZwEmdLJ8DscKNTkTqPbNwLNNBjuSzaG9Vp2KGtKJr');
const USDC_DECIMALS = 6;
const DEVNET_RPC = 'https://api.devnet.solana.com';
const NO_DELEGATE_EXPIRY = 0xffff_ffff;
const IDB_NAME = 'flip-demo-kv';
const IDB_STORE = 'kv';
const IDB_DELEGATE_KEY = 'delegate64';

const logEl = document.querySelector('#log')!;
const walletStatus = document.querySelector('#wallet-status')!;
const btnWalletAction = document.querySelector('#btn-wallet-action') as HTMLButtonElement;
const walletModalEl = document.querySelector('#wallet-modal') as HTMLElement;
const walletModalListEl = document.querySelector('#wallet-modal-list') as HTMLElement;
const btnWalletModalClose = document.querySelector('#btn-wallet-modal-close') as HTMLButtonElement;
const walletModalScrim = document.querySelector('#wallet-modal-scrim') as HTMLButtonElement;
const delegateLine = document.querySelector('#delegate-line')!;
const btnGenDelegate = document.querySelector('#btn-gen-delegate') as HTMLButtonElement;
const btnCreateVault = document.querySelector('#btn-create-vault') as HTMLButtonElement;
const btnUpdateDelegate = document.querySelector('#btn-update-delegate') as HTMLButtonElement;
const btnDeposit = document.querySelector('#btn-deposit') as HTMLButtonElement;
const btnFlip = document.querySelector('#btn-flip') as HTMLButtonElement;
const depositUi = document.querySelector('#deposit-ui') as HTMLInputElement;
const expirySec = document.querySelector('#expiry-sec') as HTMLInputElement;
const flipAmountUi = document.querySelector('#flip-amount-ui') as HTMLInputElement;
const poolHint = document.querySelector('#pool-hint')!;
const flipResult = document.querySelector('#flip-result')!;
const flipSpinner = document.querySelector('#flip-spinner') as HTMLElement;
const flipHistoryEl = document.querySelector('#flip-history') as HTMLElement;
const betSide = document.querySelector('#bet-side') as HTMLSelectElement;

const FLIP_HISTORY_KEY = 'flip-demo-outcome-history';
const FLIP_HISTORY_MAX = 10;

type FlipOutcomeRecord = {
   outcomeOdd: boolean;
   win: boolean;
};

function log(msg: string): void {
   logEl.textContent = `${new Date().toISOString().slice(11, 23)} ${msg}\n${logEl.textContent ?? ''}`;
}

/** `JSON.stringify` for RPC-shaped values; `bigint` becomes a decimal string. */
function jsonStringifySafe(value: unknown): string {
   return JSON.stringify(value, (_key, v) => (typeof v === 'bigint' ? v.toString() : v));
}

/** Matches `program/src/error.rs` `Error` repr(u32). */
const VAULT_CUSTOM_ERROR_NAMES: Record<number, string> = {
   1: 'NotSigner',
   2: 'InvalidAta',
   3: 'MintMismatch',
   4: 'ArithmeticOverflow',
   8: 'UserVaultAlreadyExists',
   9: 'UserVaultNotFound',
   10: 'UserVaultOwnerMismatch',
   11: 'UserVaultPdaMismatch',
   12: 'UserVaultAppAddressMismatch',
   13: 'InvalidUserVaultDelegate',
   14: 'UnauthorizedCpiCaller',
   15: 'UserVaultAtaNotEmpty',
   16: 'UserVaultAtaCountZero',
   17: 'UserVaultHasOpenAtas',
   18: 'InvalidUnixTimestamp',
   19: 'ExpiredDelegate (session delegate expired — set new delegate expiry)',
   20: 'InvalidClockAccount',
   21: 'InvalidRentAccount',
   22: 'InvalidUserVaultAccountLength',
};

/** Matches `flip_demo/program/src/error.rs` `Error` repr(u32). */
const FLIP_CUSTOM_ERROR_NAMES: Record<number, string> = {
   1: 'NotSigner',
   2: 'InvalidTokenProgram',
   3: 'InvalidAssociatedTokenProgram',
   4: 'InvalidSystemProgram',
   5: 'ConfigPdaMismatch',
   6: 'AlreadyInitialized',
   7: 'MintMismatch',
   8: 'TokenOwnerMismatch',
   10: 'Inactive',
   11: 'BadInstructionData',
   12: 'InvalidClock',
   13: 'NotInitialized',
   14: 'VaultProgramMismatch',
   15: 'AppAddressMismatch',
};

const PROGRAM_FAILED_LINE_RE = /^Program ([1-9A-HJ-NP-Za-km-z]+) failed: (.*)$/;

function getLastCustomFailureProgramId(logs: readonly string[]): string | null {
   let last: string | null = null;
   for (const line of logs) {
      if (!line.startsWith('Program ') || !line.includes(' failed: ')) {
         continue;
      }
      if (!/custom program error/i.test(line)) {
         continue;
      }
      const m = PROGRAM_FAILED_LINE_RE.exec(line);
      if (m) {
         last = m[1]!;
      }
   }
   return last;
}

function decodeCustomForProgram(programId: string, code: number, vaultId: string, flipId: string): string {
   if (programId === vaultId) {
      return VAULT_CUSTOM_ERROR_NAMES[code] ?? `unknown vault custom ${code} (0x${code.toString(16)})`;
   }
   if (programId === flipId) {
      return FLIP_CUSTOM_ERROR_NAMES[code] ?? `unknown flip custom ${code} (0x${code.toString(16)})`;
   }
   const short = programId.length <= 14 ? programId : `${programId.slice(0, 6)}…${programId.slice(-4)}`;
   return `unknown program ${short} custom ${code} (0x${code.toString(16)})`;
}

/**
 * RPC / `@solana/rpc` BigInt upcast: `simulateTransaction` allowlist does not cover `err`, so
 * `InstructionError` → `Custom` is often `bigint` (e.g. `19n`), not `number`.
 */
function coerceCustomErrorCode(raw: unknown): number | null {
   if (typeof raw === 'bigint') {
      const n = Number(raw);
      return Number.isSafeInteger(n) ? n : null;
   }
   if (typeof raw === 'number' && Number.isFinite(raw)) {
      return raw;
   }
   if (typeof raw === 'string') {
      const trimmed = raw.trim();
      if (/^0x[0-9a-fA-F]+$/i.test(trimmed)) {
         return parseInt(trimmed.slice(2), 16);
      }
      const n = parseInt(trimmed, 10);
      return Number.isFinite(n) ? n : null;
   }
   return null;
}

/** One line for the UI log; full detail stays in `console.error`. */
function summarizeSimulationFailure(
   err: unknown,
   logs: readonly string[] | null | undefined,
   vaultId: string,
   flipId: string,
): string {
   const hint = logs?.length ? getLastCustomFailureProgramId(logs) : null;
   if (err && typeof err === 'object' && 'InstructionError' in err) {
      const ie = (err as { InstructionError: [unknown, unknown] }).InstructionError;
      if (Array.isArray(ie) && ie.length === 2) {
         const ixErr = ie[1];
         if (typeof ixErr === 'string') {
            return ixErr;
         }
         if (ixErr && typeof ixErr === 'object' && 'Custom' in ixErr) {
            const code = coerceCustomErrorCode((ixErr as { Custom: unknown }).Custom);
            if (code != null) {
               if (hint) {
                  return decodeCustomForProgram(hint, code, vaultId, flipId);
               }
               const v = VAULT_CUSTOM_ERROR_NAMES[code];
               const f = FLIP_CUSTOM_ERROR_NAMES[code];
               if (v && f && v !== f) {
                  return `${v} (vault) / ${f} (flip)`;
               }
               return v ?? f ?? `Custom(${code})`;
            }
         }
      }
   }
   return jsonStringifySafe(err);
}

function shortAddr(a: Address): string {
   const s = String(a);
   return s.length <= 14 ? s : `${s.slice(0, 6)}…${s.slice(-4)}`;
}

function createRpc() {
   return createSolanaRpc(devnet(DEVNET_RPC));
}

let connectorClient: ConnectorClient | null = null;
let delegateSigner: KeyPairSigner | null = null;
let wasWalletConnected = false;

function buildConnectorCluster() {
   const url = DEVNET_RPC;
   return {
      id: 'solana:devnet' as const,
      label: `devnet — ${url}`,
      url,
   };
}

function initConnector(): void {
   closeWalletModal();
   wasWalletConnected = false;
   if (connectorClient) {
      connectorClient.destroy();
   }
   connectorClient = new ConnectorClient(
      getDefaultConfig({
         appName: 'Flip demo',
         autoConnect: false,
         persistClusterSelection: false,
         clusters: [buildConnectorCluster()],
      }),
   );
   connectorClient.subscribe(() => {
      syncWalletStatus();
      renderWalletModalList();
      const client = connectorClient;
      if (!client) {
         return;
      }
      const snap = client.getSnapshot();
      const nowConnected = isConnected(snap.wallet);
      if (nowConnected && !wasWalletConnected) {
         wasWalletConnected = true;
         updatePoolHint();
      } else if (!nowConnected) {
         wasWalletConnected = false;
      }
   });
   renderWalletModalList();
   syncWalletStatus();
}

function walletModalIsOpen(): boolean {
   return !walletModalEl.hasAttribute('hidden');
}

function openWalletModal(): void {
   if (!connectorClient) {
      return;
   }
   renderWalletModalList();
   walletModalEl.removeAttribute('hidden');
   btnWalletAction.setAttribute('aria-expanded', 'true');
   queueMicrotask(() => {
      const first = walletModalListEl.querySelector<HTMLButtonElement>('button:not(:disabled)');
      if (first) {
         first.focus();
      } else {
         btnWalletModalClose.focus();
      }
   });
}

function closeWalletModal(): void {
   walletModalEl.setAttribute('hidden', '');
   btnWalletAction.setAttribute('aria-expanded', 'false');
   const ae = document.activeElement;
   if (ae instanceof Node && walletModalEl.contains(ae)) {
      btnWalletAction.focus();
   }
}

function updateWalletHeaderChrome(): void {
   if (!connectorClient) {
      btnWalletAction.hidden = true;
      btnWalletAction.disabled = false;
      btnWalletAction.textContent = 'Connect';
      btnWalletAction.removeAttribute('data-mode');
      btnWalletAction.setAttribute('aria-haspopup', 'dialog');
      btnWalletAction.setAttribute('aria-expanded', 'false');
      btnWalletAction.setAttribute('aria-controls', 'wallet-modal');
      return;
   }
   const s = connectorClient.getSnapshot();
   const connected = isConnected(s.wallet);
   const connecting = s.wallet.status === 'connecting';
   btnWalletAction.hidden = false;
   if (connected) {
      btnWalletAction.textContent = 'Disconnect';
      btnWalletAction.disabled = false;
      btnWalletAction.setAttribute('data-mode', 'disconnect');
      btnWalletAction.removeAttribute('aria-haspopup');
      btnWalletAction.removeAttribute('aria-expanded');
      btnWalletAction.removeAttribute('aria-controls');
   } else if (connecting) {
      btnWalletAction.textContent = 'Connecting…';
      btnWalletAction.disabled = true;
      btnWalletAction.removeAttribute('data-mode');
      btnWalletAction.removeAttribute('aria-haspopup');
      btnWalletAction.removeAttribute('aria-expanded');
      btnWalletAction.removeAttribute('aria-controls');
   } else {
      btnWalletAction.textContent = 'Connect';
      btnWalletAction.disabled = false;
      btnWalletAction.removeAttribute('data-mode');
      btnWalletAction.setAttribute('aria-haspopup', 'dialog');
      btnWalletAction.setAttribute('aria-expanded', walletModalIsOpen() ? 'true' : 'false');
      btnWalletAction.setAttribute('aria-controls', 'wallet-modal');
   }
}

function renderWalletModalList(): void {
   walletModalListEl.innerHTML = '';
   const c = connectorClient;
   if (!c) {
      return;
   }
   for (const w of c.getSnapshot().connectors) {
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = w.ready ? w.name : `${w.name} (unavailable)`;
      b.disabled = !w.ready;
      b.addEventListener('click', () => void connectWithConnector(w).catch((e) => log(String(e))));
      walletModalListEl.appendChild(b);
   }
}

async function connectWithConnector(meta: WalletConnectorMetadata): Promise<void> {
   closeWalletModal();
   const client = requireConnector();
   await client.connectWallet(meta.id);
   log(`Connected via ${meta.name}.`);
}

function requireConnector(): ConnectorClient {
   if (!connectorClient) {
      throw new Error('Connector not ready');
   }
   return connectorClient;
}

function syncWalletStatus(): void {
   const c = connectorClient;
   if (!c) {
      walletStatus.textContent = 'Not connected';
      walletStatus.removeAttribute('title');
      updateWalletHeaderChrome();
      return;
   }
   const s = c.getSnapshot();
   if (isConnected(s.wallet)) {
      const addr = s.wallet.session.selectedAccount.address;
      walletStatus.textContent = `Connected · ${shortAddr(addr)}`;
      walletStatus.setAttribute('title', String(addr));
   } else if (s.wallet.status === 'connecting') {
      walletStatus.textContent = 'Connecting…';
      walletStatus.removeAttribute('title');
   } else if (s.wallet.status === 'error') {
      walletStatus.textContent = `Error: ${s.wallet.error.message}`;
      walletStatus.setAttribute('title', s.wallet.error.message);
   } else {
      walletStatus.textContent = 'Not connected';
      walletStatus.removeAttribute('title');
   }
   updateWalletHeaderChrome();
}

function getOwnerSigner() {
   const c = requireConnector();
   const s = c.getSnapshot();
   if (!isConnected(s.wallet)) {
      throw new Error('Connect wallet first');
   }
   const w = c.getConnector(s.wallet.session.connectorId);
   if (!w) {
      throw new Error('Wallet connector missing');
   }
   const ts = createTransactionSigner({
      wallet: w,
      account: s.wallet.session.selectedAccount.account,
      cluster: s.cluster ?? undefined,
   });
   if (!ts) {
      throw new Error('Wallet cannot sign');
   }
   return createKitTransactionSigner(ts);
}

function requireDelegate(): KeyPairSigner {
   if (!delegateSigner) {
      throw new Error('Generate a delegate key in this browser first');
   }
   return delegateSigner;
}

function flipMode(): 'appIx' | 'cpi' {
   const el = document.querySelector<HTMLInputElement>('input[name="flip-mode"]:checked');
   return el?.value === 'cpi' ? 'cpi' : 'appIx';
}

function setFlipPending(pending: boolean): void {
   btnFlip.disabled = pending;
   flipSpinner.hidden = !pending;
}

function loadFlipHistory(): FlipOutcomeRecord[] {
   try {
      const raw = sessionStorage.getItem(FLIP_HISTORY_KEY);
      if (!raw) {
         return [];
      }
      const parsed = JSON.parse(raw) as unknown;
      if (!Array.isArray(parsed)) {
         return [];
      }
      const out: FlipOutcomeRecord[] = [];
      for (const x of parsed) {
         if (
            x &&
            typeof x === 'object' &&
            'outcomeOdd' in x &&
            'win' in x &&
            typeof (x as FlipOutcomeRecord).outcomeOdd === 'boolean' &&
            typeof (x as FlipOutcomeRecord).win === 'boolean'
         ) {
            out.push(x as FlipOutcomeRecord);
         }
      }
      return out.slice(0, FLIP_HISTORY_MAX);
   } catch {
      return [];
   }
}

function saveFlipHistory(items: readonly FlipOutcomeRecord[]): void {
   sessionStorage.setItem(FLIP_HISTORY_KEY, JSON.stringify([...items].slice(0, FLIP_HISTORY_MAX)));
}

function pushFlipOutcome(outcomeOdd: boolean, win: boolean): void {
   const next = [{ outcomeOdd, win }, ...loadFlipHistory()].slice(0, FLIP_HISTORY_MAX);
   saveFlipHistory(next);
   renderFlipHistory();
}

function renderFlipHistory(): void {
   const items = loadFlipHistory();
   flipHistoryEl.replaceChildren();
   for (const rec of [...items].reverse()) {
      const span = document.createElement('span');
      span.className = `flip-history-chip ${rec.win ? 'flip-history-win' : 'flip-history-lose'}`;
      span.textContent = rec.outcomeOdd ? 'ODD' : 'EVEN';
      flipHistoryEl.appendChild(span);
   }
}

function parseUiAmount(s: string, decimals: number): bigint {
   const t = s.trim();
   const m = /^(\d+)(?:\.(\d*))?$/.exec(t);
   if (!m) {
      throw new RangeError('Invalid amount');
   }
   let frac = m[2] ?? '';
   if (frac.length > decimals) {
      frac = frac.slice(0, decimals);
   }
   frac = frac.padEnd(decimals, '0');
   return BigInt(m[1] + frac);
}

function delegateExpiresFromInput(): number {
   const sec = Number(expirySec.value);
   if (!Number.isFinite(sec) || sec < 1) {
      throw new RangeError('Expiry seconds must be >= 1');
   }
   const now = Math.floor(Date.now() / 1000);
   return Math.min(now + Math.floor(sec), NO_DELEGATE_EXPIRY);
}

async function idbGet(key: string): Promise<Uint8Array | null> {
   return new Promise((resolve, reject) => {
      const req = indexedDB.open(IDB_NAME, 1);
      req.onupgradeneeded = () => {
         req.result.createObjectStore(IDB_STORE);
      };
      req.onerror = () => reject(req.error);
      req.onsuccess = () => {
         const db = req.result;
         const tx = db.transaction(IDB_STORE, 'readonly');
         const g = tx.objectStore(IDB_STORE).get(key);
         g.onsuccess = () => {
            const v = g.result;
            resolve(v instanceof Uint8Array ? v : null);
         };
         g.onerror = () => reject(g.error);
      };
   });
}

async function idbSet(key: string, val: Uint8Array): Promise<void> {
   return new Promise((resolve, reject) => {
      const req = indexedDB.open(IDB_NAME, 1);
      req.onupgradeneeded = () => {
         req.result.createObjectStore(IDB_STORE);
      };
      req.onerror = () => reject(req.error);
      req.onsuccess = () => {
         const db = req.result;
         const tx = db.transaction(IDB_STORE, 'readwrite');
         tx.objectStore(IDB_STORE).put(val, key);
         tx.oncomplete = () => resolve();
         tx.onerror = () => reject(tx.error);
      };
   });
}

async function loadDelegateFromIdb(): Promise<void> {
   const raw = await idbGet(IDB_DELEGATE_KEY);
   if (!raw || (raw.length !== 32 && raw.length !== 64)) {
      delegateSigner = null;
      delegateLine.textContent = 'No delegate in this browser.';
      return;
   }
   delegateSigner =
      raw.length === 64
         ? await createKeyPairSignerFromBytes(raw)
         : await createKeyPairSignerFromPrivateKeyBytes(raw);
   delegateLine.textContent = `Delegate: ${String(delegateSigner.address)}`;
}

async function ensureDelegateAirdrop(): Promise<void> {
   const rpc = createRpc();
   const d = requireDelegate();
   const { value: bal } = await rpc.getBalance(d.address).send();
   if (bal >= lamports(50_000_000n)) {
      return;
   }
   log('Airdropping delegate (devnet)…');
   const sig = await rpc.requestAirdrop(d.address, lamports(1_000_000_000n), { commitment: 'confirmed' }).send();
   log(`Airdrop sig ${sig}`);
}

function encodeFlipData(disc: 1 | 2, amount: bigint, betOdd: boolean): Uint8Array {
   const out = new Uint8Array(10);
   out[0] = disc;
   const le = new DataView(out.buffer, 1, 8);
   le.setBigUint64(0, amount, true);
   out[9] = betOdd ? 1 : 0;
   return out;
}

async function deriveConfigPda(): Promise<Address> {
   const [pda] = await getProgramDerivedAddress({
      programAddress: FLIP_PROGRAM_ADDRESS,
      seeds: [new TextEncoder().encode('config')],
   });
   return pda;
}

async function derivePoolAta(configPda: Address): Promise<Address> {
   return deriveAtaAddress(configPda, DEVNET_USDC_MINT, SPL_TOKEN_PROGRAM_ADDRESS);
}

async function fetchPoolTokenBalance(): Promise<bigint> {
   const config = await deriveConfigPda();
   const pool = await derivePoolAta(config);
   const rpc = createRpc();
   const { value: acct } = await rpc.getAccountInfo(pool, { encoding: 'base64' }).send();
   if (!acct?.data) {
      return 0n;
   }
   const [b64] = acct.data;
   const bin = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
   if (bin.length < 72) {
      return 0n;
   }
   const v = new DataView(bin.buffer, bin.byteOffset + 64, 8);
   return v.getBigUint64(0, true);
}

function updatePoolHint(): void {
   void fetchPoolTokenBalance().then((b) => {
      const max = b / 100n;
      poolHint.textContent = `Pool balance ${b/(10n**BigInt(USDC_DECIMALS))} · max stake (1%) ${max/(10n**BigInt(USDC_DECIMALS))}`;
      if (!flipAmountUi.value.trim() && max > 0n) {
         const whole = max / 10n ** BigInt(USDC_DECIMALS);
         const frac = max % 10n ** BigInt(USDC_DECIMALS);
         flipAmountUi.value = frac === 0n ? `${whole}` : `${whole}.${frac.toString().padStart(USDC_DECIMALS, '0').replace(/0+$/, '')}`;
      }
   });
}

async function sendTx(
   ixs: Instruction[],
   signers: Parameters<typeof addSignersToInstruction>[0],
   feePayer: (typeof signers)[0],
   label: string,
): Promise<Signature> {
   const rpc = createRpc();
   const { value: lb } = await rpc.getLatestBlockhash().send();
   const baseMsg = pipe(
      createTransactionMessage({ version: 0 }),
      (m) => setTransactionMessageFeePayerSigner(feePayer, m),
      (m) =>
         setTransactionMessageLifetimeUsingBlockhash(
            { blockhash: lb.blockhash, lastValidBlockHeight: lb.lastValidBlockHeight },
            m,
         ),
   );
   let msg: typeof baseMsg = baseMsg;
   for (const ix of ixs) {
      const withS = addSignersToInstruction(signers, ix);
      msg = appendTransactionMessageInstruction(withS, msg) as unknown as typeof baseMsg;
   }
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
      console.error('[flip demo] simulateTransaction failed', {
         label,
         err: sim.err,
         logs: sim.logs,
         unitsConsumed: sim.unitsConsumed,
         returnData: sim.returnData,
      });
      if (sim.logs?.length) {
         console.error('[flip demo] simulation logs\n' + sim.logs.join('\n'));
      }
      const vaultId = String(VAULT_PROGRAM_ADDRESS);
      const flipId = String(FLIP_PROGRAM_ADDRESS);
      const summary = summarizeSimulationFailure(sim.err, sim.logs ?? undefined, vaultId, flipId);
      throw new Error(summary);
   }
   const sigStr = await rpc.sendTransaction(wire, { encoding: 'base64' }).send();
   assertIsSignature(sigStr);
   log(`[tx] ${label} ${sigStr}`);
   return sigStr;
}

async function onCreateVault(): Promise<void> {
   const owner = getOwnerSigner();
   const del = requireDelegate();
   await ensureDelegateAirdrop();
   const ownerAddr = owner.address;
   const exp = delegateExpiresFromInput();
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, ownerAddr, FLIP_PROGRAM_ADDRESS);
   const ix = getCreateUserVaultInstruction({
      accounts: {
         owner: ownerAddr,
         userVaultPda: pda,
         appAddress: FLIP_PROGRAM_ADDRESS,
         delegate: del.address,
      },
      data: { delegateExpires: exp },
   });
   await sendTx([ix], [owner], owner, 'create vault');
}

async function onUpdateDelegate(): Promise<void> {
   const owner = getOwnerSigner();
   const del = requireDelegate();
   const ownerAddr = owner.address;
   const exp = delegateExpiresFromInput();
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, ownerAddr, FLIP_PROGRAM_ADDRESS);
   const ix = getUpdateUserVaultDelegateInstruction({
      accounts: {
         owner: ownerAddr,
         userVaultPda: pda,
         appAddress: FLIP_PROGRAM_ADDRESS,
         delegate: del.address,
      },
      data: { delegateExpires: exp },
   });
   await sendTx([ix], [owner], owner, 'update delegate');
}

async function onDeposit(): Promise<void> {
   const owner = getOwnerSigner();
   const del = requireDelegate();
   await ensureDelegateAirdrop();
   const ownerAddr = owner.address;
   const amt = parseUiAmount(depositUi.value, USDC_DECIMALS);
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, ownerAddr, FLIP_PROGRAM_ADDRESS);
   const vaultAta = await deriveUserVaultAtaAddress(pda, DEVNET_USDC_MINT, SPL_TOKEN_PROGRAM_ADDRESS);
   const sourceAta = await deriveAtaAddress(ownerAddr, DEVNET_USDC_MINT, SPL_TOKEN_PROGRAM_ADDRESS);
   const ix = getDepositUserVaultInstruction({
      accounts: {
         owner: ownerAddr,
         userVaultPda: pda,
         userVaultAta: vaultAta,
         appAddress: FLIP_PROGRAM_ADDRESS,
         sourceAta,
         mint: DEVNET_USDC_MINT,
         tokenProgram: SPL_TOKEN_PROGRAM_ADDRESS,
      },
      data: { amount: amt },
   });
   await sendTx([ix], [owner], owner, 'deposit');
   updatePoolHint();
}

function buildFlipViaCpiIx(input: {
   delegate: Address;
   amount: bigint;
   betOdd: boolean;
   owner: Address;
   vaultPda: Address;
   vaultAta: Address;
   poolAta: Address;
   configPda: Address;
}): Instruction {
   const data = encodeFlipData(2, input.amount, input.betOdd);
   return {
      programAddress: FLIP_PROGRAM_ADDRESS,
      accounts: [
         { address: VAULT_PROGRAM_ADDRESS, role: AccountRole.READONLY },
         { address: input.delegate, role: AccountRole.WRITABLE_SIGNER },
         { address: input.owner, role: AccountRole.READONLY },
         { address: input.vaultPda, role: AccountRole.READONLY },
         { address: input.vaultAta, role: AccountRole.WRITABLE },
         { address: FLIP_PROGRAM_ADDRESS, role: AccountRole.READONLY },
         { address: input.owner, role: AccountRole.READONLY },
         { address: input.poolAta, role: AccountRole.WRITABLE },
         { address: DEVNET_USDC_MINT, role: AccountRole.READONLY },
         { address: SPL_TOKEN_PROGRAM_ADDRESS, role: AccountRole.READONLY },
         { address: SYSVAR_INSTRUCTIONS_ADDRESS, role: AccountRole.READONLY },
         { address: SYSVAR_CLOCK_ADDRESS, role: AccountRole.READONLY },
         { address: input.configPda, role: AccountRole.WRITABLE },
      ],
      data: new Uint8Array(data),
   };
}

async function onFlip(): Promise<void> {
   flipResult.textContent = '';
   const owner = getOwnerSigner();
   const del = requireDelegate();
   await ensureDelegateAirdrop();
   const ownerAddr = owner.address;
   const poolBal = await fetchPoolTokenBalance();
   const maxStake = poolBal / 100n;
   const amt = parseUiAmount(flipAmountUi.value, USDC_DECIMALS);
   if (amt > maxStake) {
      throw new RangeError(`Stake exceeds 1% of pool (max ${maxStake} base units)`);
   }
   if (amt <= 0n) {
      throw new RangeError('Stake must be > 0');
   }
   const betOdd = betSide.value === 'odd';
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, ownerAddr, FLIP_PROGRAM_ADDRESS);
   const vaultAta = await deriveUserVaultAtaAddress(pda, DEVNET_USDC_MINT, SPL_TOKEN_PROGRAM_ADDRESS);
   const configPda = await deriveConfigPda();
   const poolAta = await derivePoolAta(configPda);

   const mode = flipMode();
   setFlipPending(true);
   try {
      let sig: Signature;
      if (mode === 'appIx') {
         const innerData = encodeFlipData(1, amt, betOdd);
         const innerAccounts = [
            { address: vaultAta, role: AccountRole.WRITABLE },
            { address: poolAta, role: AccountRole.WRITABLE },
            { address: configPda, role: AccountRole.WRITABLE },
            { address: pda, role: AccountRole.WRITABLE },
            { address: SPL_TOKEN_PROGRAM_ADDRESS, role: AccountRole.READONLY },
            { address: SYSVAR_CLOCK_ADDRESS, role: AccountRole.READONLY },
         ] as const;
         const ix = getAppIxInstruction({
            accounts: {
               delegate: del.address,
               owner: ownerAddr,
               userVaultPda: pda,
               appAddress: FLIP_PROGRAM_ADDRESS,
            },
            innerAccounts: [...innerAccounts],
            data: { innerInstructionData: innerData },
         });
         sig = await sendTx([ix], [del], del, 'flip AppIx');
      } else {
         const ix = buildFlipViaCpiIx({
            delegate: del.address,
            amount: amt,
            betOdd,
            owner: ownerAddr,
            vaultPda: pda,
            vaultAta,
            poolAta,
            configPda,
         });
         sig = await sendTx([ix], [del], del, 'flip via cpi_entry');
      }

      const rpc = createRpc();
      let isFetched = false;
      let tx;
      let count = 0;
      while (!isFetched && count < 10) {
         tx = await rpc
            .getTransaction(sig, { encoding: 'json', maxSupportedTransactionVersion: 0, commitment: 'confirmed' })
            .send();
         isFetched = tx != null;
         count++;
         await new Promise((resolve) => setTimeout(resolve, 1000));
      }
      const slot = tx?.slot;
      if (slot == null) {
         flipResult.textContent = 'Confirmed but no slot in response';
         return;
      }
      const slotBn = typeof slot === 'bigint' ? slot : BigInt(slot);
      const outcomeOdd = slotBn % 2n === 1n;
      const win = (outcomeOdd && betOdd) || (!outcomeOdd && !betOdd);
      flipResult.textContent = win ? 'You won' : 'You lost';
      log(`slot=${String(slot)} outcomeOdd=${outcomeOdd} betOdd=${betOdd} → ${win ? 'win' : 'lose'}`);
      pushFlipOutcome(outcomeOdd, win);
      updatePoolHint();
   } finally {
      setFlipPending(false);
   }
}

btnWalletAction.addEventListener('click', () => {
   const c = connectorClient;
   if (!c) {
      return;
   }
   const s = c.getSnapshot();
   if (isConnected(s.wallet)) {
      void c.disconnectWallet();
      log('Disconnected');
      return;
   }
   openWalletModal();
});

btnWalletModalClose.addEventListener('click', () => closeWalletModal());
walletModalScrim.addEventListener('click', () => closeWalletModal());
document.addEventListener('keydown', (ev) => {
   if (ev.key === 'Escape' && walletModalIsOpen()) {
      closeWalletModal();
   }
});

btnGenDelegate.addEventListener('click', () => {
   void (async () => {
      const secret = crypto.getRandomValues(new Uint8Array(32));
      await idbSet(IDB_DELEGATE_KEY, secret);
      delegateSigner = await createKeyPairSignerFromPrivateKeyBytes(secret);
      delegateLine.textContent = `Delegate: ${String(delegateSigner.address)}`;
      log('New delegate (32-byte seed) stored in IndexedDB');
   })();
});

btnCreateVault.addEventListener('click', () => {
   void onCreateVault().catch((e) => log(String(e)));
});
btnUpdateDelegate.addEventListener('click', () => {
   void onUpdateDelegate().catch((e) => log(String(e)));
});
btnDeposit.addEventListener('click', () => {
   void onDeposit().catch((e) => log(String(e)));
});
btnFlip.addEventListener('click', () => {
   void onFlip().catch((e) => log(String(e)));
});

initConnector();
renderFlipHistory();
void loadDelegateFromIdb().then(() => {
   updatePoolHint();
});
