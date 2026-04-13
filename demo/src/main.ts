import {
   ConnectorClient,
   createKitTransactionSigner,
   createTransactionSigner,
   getDefaultConfig,
   isConnected,
   type SolanaCluster,
   type SolanaClusterId,
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
   mainnet,
   pipe,
   setTransactionMessageFeePayerSigner,
   setTransactionMessageLifetimeUsingBlockhash,
   signTransactionMessageWithSigners,
   testnet,
   type AccountSignerMeta,
   type Address,
   type Instruction,
   type InstructionWithSigners,
   type TransactionSigner,
} from '@solana/kit';

import {
   deriveAtaAddress,
   deriveUserVaultAtaAddress,
   deriveUserVaultPda,
   fetchUserVaultAccountsByFilter,
   getCloseUserVaultInstruction,
   getCloseVaultAtaInstruction,
   getCreateUserVaultInstruction,
   getDepositUserVaultInstruction,
   getUpdateUserVaultDelegateInstruction,
   getWithdrawUserVaultInstruction,
   SPL_TOKEN_PROGRAM_ADDRESS,
   TOKEN_2022_PROGRAM_ADDRESS,
   VAULT_PROGRAM_ADDRESS,
} from '@vault/sdk';

const NO_DELEGATE_EXPIRY = 0xffff_ffff;

/** Rent-exempt lamports locked in the vault PDA; shown balance is total minus this reserve. */
const VAULT_PDA_RENT_LAMPORTS = 1_614_720n;
const LAMPORTS_PER_SOL = 1_000_000_000n;

function solBalanceMinusVaultRentDisplay(lamportsRaw: bigint): string {
   const net = lamportsRaw > VAULT_PDA_RENT_LAMPORTS ? lamportsRaw - VAULT_PDA_RENT_LAMPORTS : 0n;
   const intPart = net / LAMPORTS_PER_SOL;
   const frac = net % LAMPORTS_PER_SOL;
   if (frac === 0n) {
      return `${intPart}`;
   }
   const fracStr = frac.toString().padStart(9, '0').replace(/0+$/, '');
   return `${intPart}.${fracStr}`;
}

let connectorClient: ConnectorClient | null = null;
let unsubscribeConnector: (() => void) | null = null;
/** Tracks connector snapshot so we can run a default “by wallet” read once per connect. */
let wasConnectorWalletConnected = false;
let selectedDelegateExpires: number = NO_DELEGATE_EXPIRY;
let selectedPreset: '1h' | '1d' | 'never' = 'never';

const logEl = document.querySelector('#log')!;
const readResults = document.querySelector('#read-results') as HTMLElement;
const readBatchBar = document.querySelector('#read-batch-bar') as HTMLElement;
const walletStatus = document.querySelector('#wallet-status')!;
const btnWalletAction = document.querySelector('#btn-wallet-action') as HTMLButtonElement;
const walletModalEl = document.querySelector('#wallet-modal') as HTMLElement;
const walletModalListEl = document.querySelector('#wallet-modal-list') as HTMLElement;
const btnWalletModalClose = document.querySelector('#btn-wallet-modal-close') as HTMLButtonElement;
const walletModalScrim = document.querySelector('#wallet-modal-scrim') as HTMLButtonElement;

function log(msg: string): void {
   logEl.textContent = `${new Date().toISOString().slice(11, 23)} ${msg}\n${logEl.textContent ?? ''}`;
}

/** Shorten addresses for log lines (base58-safe middle ellipsis). */
function shortAddr(a: Address | undefined): string {
   if (!a) {
      return '?';
   }
   const s = String(a);
   return s.length <= 14 ? s : `${s.slice(0, 6)}…${s.slice(-4)}`;
}

function formatBatchTxDescription(specs: BatchIxSpec[]): string {
   const nW = specs.filter((s) => s.kind === 'withdraw').length;
   const nC = specs.filter((s) => s.kind === 'closeAta').length;
   const nV = specs.filter((s) => s.kind === 'closeVault').length;
   const bits: string[] = [];
   if (nW) {
      bits.push(`${nW} withdraw`);
   }
   if (nC) {
      bits.push(`${nC} close ATA`);
   }
   if (nV) {
      bits.push(`${nV} close vault`);
   }
   const head = `${specs.length} instruction(s): ${bits.join(', ')}`;
   const withdrawHints = specs
      .filter((s) => s.kind === 'withdraw')
      .slice(0, 3)
      .map((s) => `${shortAddr(s.mint)} +${s.amount}`);
   const extra = withdrawHints.length ? ` — ${withdrawHints.join('; ')}${nW > 3 ? '…' : ''}` : '';
   return head + extra;
}

function escapeHtml(s: string): string {
   return s
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;');
}

function escapeAttr(s: string): string {
   return s.replace(/&/g, '&amp;').replace(/"/g, '&quot;');
}

function unixNowSec(): number {
   return Math.floor(Date.now() / 1000);
}

function setDelegatePreset(preset: '1h' | '1d' | 'never'): void {
   selectedPreset = preset;
   if (preset === 'never') {
      selectedDelegateExpires = NO_DELEGATE_EXPIRY;
   } else {
      const delta = preset === '1h' ? 3600 : 86_400;
      const t = unixNowSec() + delta;
      selectedDelegateExpires = Math.min(t, NO_DELEGATE_EXPIRY);
   }
   syncDelegateExpiryButtons();
   updateDelegateExpiryLabel();
}

function syncDelegateExpiryButtons(): void {
   for (const btn of Array.from(document.querySelectorAll<HTMLButtonElement>('.preset-expiry'))) {
      const p = btn.dataset.preset as '1h' | '1d' | 'never' | undefined;
      btn.classList.toggle('active', p === selectedPreset);
   }
}

function updateDelegateExpiryLabel(): void {
   const text =
      selectedDelegateExpires === NO_DELEGATE_EXPIRY
         ? 'Never'
         : new Date(selectedDelegateExpires * 1000).toLocaleString(undefined, {
              dateStyle: 'medium',
              timeStyle: 'short',
           });
   for (const el of Array.from(document.querySelectorAll('.delegate-expiry-caption'))) {
      el.textContent = text;
   }
}

function parseHumanTokenAmount(ui: string, decimals: number): bigint {
   const s = ui.trim();
   if (!s) {
      throw new RangeError('Amount is required.');
   }
   if (!Number.isInteger(decimals) || decimals < 0 || decimals > 18) {
      throw new RangeError('Decimals must be an integer from 0 to 18.');
   }
   const m = /^(\d+)(?:\.(\d*))?$/.exec(s);
   if (!m) {
      throw new RangeError('Use digits and at most one decimal point (e.g. 1.5).');
   }
   const intPart = m[1];
   let frac = m[2] ?? '';
   if (frac.length > decimals) {
      frac = frac.slice(0, decimals);
   }
   frac = frac.padEnd(decimals, '0');
   const combined = intPart + frac;
   return BigInt(combined);
}

function parseAddr(label: string, s: string): Address {
   const t = s.trim();
   if (!t) {
      throw new Error(`${label} is required.`);
   }
   return address(t);
}

function clusterUrl(): string {
   const cluster = (document.querySelector('#cluster') as HTMLSelectElement).value;
   if (cluster === 'mainnet') {
      return 'https://api.mainnet-beta.solana.com';
   }
   if (cluster === 'testnet') {
      return 'https://api.testnet.solana.com';
   }
   return 'https://api.devnet.solana.com';
}

function clusterIdForUi(): SolanaClusterId {
   const cluster = (document.querySelector('#cluster') as HTMLSelectElement).value;
   if (cluster === 'mainnet') {
      return 'solana:mainnet';
   }
   if (cluster === 'testnet') {
      return 'solana:testnet';
   }
   return 'solana:devnet';
}

function buildConnectorCluster(): SolanaCluster {
   const cluster = (document.querySelector('#cluster') as HTMLSelectElement).value;
   const url = clusterUrl();
   return {
      id: clusterIdForUi(),
      label: `${cluster} — ${url}`,
      url,
   };
}

function buildConnectorConfig() {
   return getDefaultConfig({
      appName: 'App-Specific Delegated Vaults Demo',
      autoConnect: false,
      persistClusterSelection: false,
      clusters: [buildConnectorCluster()],
   });
}

function destroyConnectorClient(): void {
   closeWalletModal();
   if (unsubscribeConnector) {
      unsubscribeConnector();
      unsubscribeConnector = null;
   }
   if (connectorClient) {
      connectorClient.destroy();
      connectorClient = null;
   }
}

function initConnectorClient(): void {
   destroyConnectorClient();
   connectorClient = new ConnectorClient(buildConnectorConfig());
   unsubscribeConnector = connectorClient.subscribe(() => {
      syncWalletStatus();
      renderWalletModalList();
      const client = connectorClient;
      if (!client) {
         return;
      }
      const snap = client.getSnapshot();
      const nowConnected = isConnected(snap.wallet);
      if (nowConnected && !wasConnectorWalletConnected) {
         wasConnectorWalletConnected = true;
         void runVaultQuery('wallet').catch((e) => log(String(e)));
      } else if (!nowConnected) {
         wasConnectorWalletConnected = false;
      }
   });
   renderWalletModalList();
   syncWalletStatus();
}

function requireConnector(): ConnectorClient {
   if (!connectorClient) {
      throw new Error('Connector not initialized');
   }
   return connectorClient;
}

function syncWalletStatus(): void {
   if (!connectorClient) {
      walletStatus.textContent = 'Not connected';
      walletStatus.removeAttribute('title');
      updateWalletHeaderChrome();
      return;
   }
   const s = connectorClient.getSnapshot();
   if (isConnected(s.wallet)) {
      const addr = s.wallet.session.selectedAccount.address;
      walletStatus.textContent = `Connected · ${shortAddr(addr)}`;
      walletStatus.setAttribute('title', String(addr));
      const qw = document.querySelector('#query-wallet') as HTMLInputElement;
      if (qw && !qw.value.trim()) {
         qw.value = String(addr);
      }
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
   if (!connectorClient) {
      return;
   }
   const { connectors } = connectorClient.getSnapshot();
   for (const c of connectors) {
      const btn = document.createElement('button');
      btn.type = 'button';
      btn.textContent = c.ready ? c.name : `${c.name} (unavailable)`;
      btn.disabled = !c.ready;
      btn.addEventListener('click', () => void connectWithConnector(c).catch((e) => log(String(e))));
      walletModalListEl.appendChild(btn);
   }
}

async function connectWithConnector(meta: WalletConnectorMetadata): Promise<void> {
   closeWalletModal();
   const client = requireConnector();
   await client.connectWallet(meta.id);
   log(`Connected via ${meta.name}.`);
}

function createRpc() {
   const url = clusterUrl();
   const cluster = (document.querySelector('#cluster') as HTMLSelectElement).value;
   if (cluster === 'mainnet') {
      return createSolanaRpc(mainnet(url));
   }
   if (cluster === 'testnet') {
      return createSolanaRpc(testnet(url));
   }
   return createSolanaRpc(devnet(url));
}

function tokenProgramForKind(tp: 'spl' | 'token2022'): Address {
   return tp === 'token2022' ? TOKEN_2022_PROGRAM_ADDRESS : SPL_TOKEN_PROGRAM_ADDRESS;
}

function tokenProgramFromSelect(selectId: string): Address {
   const v = (document.querySelector(selectId) as HTMLSelectElement).value;
   return tokenProgramForKind(v === 'token2022' ? 'token2022' : 'spl');
}

function getConnectedAccountAddress(): Address {
   const client = requireConnector();
   const s = client.getSnapshot();
   if (!isConnected(s.wallet)) {
      throw new Error('Connect a wallet first.');
   }
   return s.wallet.session.selectedAccount.address;
}

function tryGetConnectedAccountAddress(): Address | null {
   try {
      return getConnectedAccountAddress();
   } catch {
      return null;
   }
}

function getKitSignerForConnectedWallet(): TransactionSigner {
   const client = requireConnector();
   const s = client.getSnapshot();
   if (!isConnected(s.wallet)) {
      throw new Error('Connect a wallet first.');
   }
   const wallet = client.getConnector(s.wallet.session.connectorId);
   if (!wallet) {
      throw new Error('Wallet missing from registry; try reconnecting.');
   }
   const connectorSigner = createTransactionSigner({
      wallet,
      account: s.wallet.session.selectedAccount.account,
      cluster: s.cluster ?? undefined,
   });
   if (!connectorSigner) {
      throw new Error('This wallet cannot sign transactions.');
   }
   return createKitTransactionSigner(connectorSigner);
}

function vaultIxWithOwnerSigner(ix: Instruction, ownerSigner: TransactionSigner): Instruction & InstructionWithSigners {
   const accounts = ix.accounts ? [...ix.accounts] : [];
   const a0 = accounts[0];
   if (!a0) {
      throw new Error('Instruction has no accounts.');
   }
   if (a0.address !== ownerSigner.address) {
      throw new Error('First account must be the owner (writable signer).');
   }
   if (a0.role !== AccountRole.WRITABLE_SIGNER) {
      throw new Error('Expected owner as WRITABLE_SIGNER.');
   }
   const ownerMeta: AccountSignerMeta = {
      address: a0.address,
      role: AccountRole.WRITABLE_SIGNER,
      signer: ownerSigner,
   };
   accounts[0] = ownerMeta;
   return { ...ix, accounts: accounts as typeof ix.accounts } as Instruction & InstructionWithSigners;
}

async function sendOwnerSignedInstructions(ixs: Instruction[], txDescription: string): Promise<string> {
   if (ixs.length === 0) {
      throw new Error('No instructions to send.');
   }
   log(`[tx] Sending: ${txDescription}`);
   const signer = getKitSignerForConnectedWallet();
   const ixWs = ixs.map((ix) => vaultIxWithOwnerSigner(ix, signer));
   const rpc = createRpc();
   const { value: lb } = await rpc.getLatestBlockhash().send();
   const baseMsg = pipe(
      createTransactionMessage({ version: 0 }),
      (m) => setTransactionMessageFeePayerSigner(signer, m),
      (m) =>
         setTransactionMessageLifetimeUsingBlockhash(
            { blockhash: lb.blockhash, lastValidBlockHeight: lb.lastValidBlockHeight },
            m,
         ),
   );
   let msg: typeof baseMsg = baseMsg;
   for (const ixW of ixWs) {
      msg = appendTransactionMessageInstruction(ixW, msg) as unknown as typeof baseMsg;
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
      console.error('[vault demo] simulateTransaction failed', {
         err: sim.err,
         logs: sim.logs,
         unitsConsumed: sim.unitsConsumed,
         returnData: sim.returnData,
      });
      if (sim.logs?.length) {
         console.error('[vault demo] simulation logs\n' + sim.logs.join('\n'));
      }
      throw new Error(`Transaction simulation failed: ${JSON.stringify(sim.err)}`);
   }

   try {
      const sig = await rpc.sendTransaction(wire, { encoding: 'base64' }).send();
      log(`[tx] Sent — ${sig}`);
      return sig;
   } catch (sendErr) {
      try {
         const { value: postFail } = await rpc
            .simulateTransaction(wire, {
               encoding: 'base64',
               sigVerify: true,
               commitment: 'confirmed',
               innerInstructions: true,
            })
            .send();
         console.error('[vault demo] sendTransaction failed; simulateTransaction snapshot', {
            err: postFail.err,
            logs: postFail.logs,
            unitsConsumed: postFail.unitsConsumed,
            returnData: postFail.returnData,
         });
         if (postFail.logs?.length) {
            console.error('[vault demo] post-send simulation logs\n' + postFail.logs.join('\n'));
         }
      } catch (simErr) {
         console.error('[vault demo] could not re-simulate after send failure', simErr);
      }
      console.error('[vault demo] sendTransaction error', sendErr);
      throw sendErr;
   }
}

async function sendOwnerSignedIx(ix: Instruction, txDescription: string): Promise<string> {
   return sendOwnerSignedInstructions([ix], txDescription);
}

type BatchKind = 'withdraw' | 'closeAta' | 'closeVault';

type BatchIxSpec = {
   kind: BatchKind;
   owner: Address;
   app: Address;
   vault: Address;
   mint?: Address;
   ata?: Address;
   amount?: string;
   tp: 'spl' | 'token2022';
};

function parseBatchIxFromInput(el: HTMLInputElement): BatchIxSpec | null {
   const d = el.dataset;
   const kind = d.kind as BatchKind | undefined;
   if (!kind || (kind !== 'withdraw' && kind !== 'closeAta' && kind !== 'closeVault')) {
      return null;
   }
   if (!d.owner || !d.app || !d.vault) {
      return null;
   }
   const tp = d.tp === 'token2022' ? 'token2022' : 'spl';
   return {
      kind,
      owner: address(d.owner),
      app: address(d.app),
      vault: address(d.vault),
      mint: d.mint ? address(d.mint) : undefined,
      ata: d.ata ? address(d.ata) : undefined,
      amount: d.amount,
      tp,
   };
}

function batchSpecKey(s: BatchIxSpec): string {
   return `${s.kind}|${s.vault}|${s.app}|${s.ata ?? ''}|${s.mint ?? ''}|${s.amount ?? ''}`;
}

function sortBatchSpecs(items: BatchIxSpec[]): BatchIxSpec[] {
   const rank: Record<BatchKind, number> = { withdraw: 0, closeAta: 1, closeVault: 2 };
   return [...items].sort((a, b) => {
      const d = rank[a.kind] - rank[b.kind];
      if (d !== 0) {
         return d;
      }
      return String(a.vault).localeCompare(String(b.vault));
   });
}

async function instructionFromBatchSpec(owner: Address, spec: BatchIxSpec): Promise<Instruction> {
   const tpAddr = tokenProgramForKind(spec.tp);
   if (spec.kind === 'withdraw') {
      if (!spec.mint || !spec.ata || !spec.amount) {
         throw new Error('Withdraw action is missing mint, ATA, or amount.');
      }
      const amt = BigInt(spec.amount);
      if (amt <= 0n) {
         throw new Error('Withdraw amount must be positive.');
      }
      const destAta = await deriveAtaAddress(owner, spec.mint, tpAddr);
      return getWithdrawUserVaultInstruction({
         accounts: {
            owner,
            userVaultPda: spec.vault,
            userVaultAta: spec.ata,
            appAddress: spec.app,
            destAta,
            mint: spec.mint,
            tokenProgram: tpAddr,
         },
         data: { amount: amt },
      });
   }
   if (spec.kind === 'closeAta') {
      if (!spec.mint || !spec.ata) {
         throw new Error('Close ATA action is missing mint or ATA.');
      }
      return getCloseVaultAtaInstruction({
         accounts: {
            owner,
            userVaultPda: spec.vault,
            appAddress: spec.app,
            userVaultAta: spec.ata,
            destination: owner,
            mint: spec.mint,
            tokenProgram: tpAddr,
         },
      });
   }
   return getCloseUserVaultInstruction({
      accounts: { owner, userVaultPda: spec.vault, appAddress: spec.app },
   });
}

async function onRunSelectedReadActions(): Promise<void> {
   const owner = getConnectedAccountAddress();
   const raw: BatchIxSpec[] = [];
   for (const el of Array.from(readResults.querySelectorAll<HTMLInputElement>('input.batch-ix:checked'))) {
      const spec = parseBatchIxFromInput(el);
      if (spec) {
         raw.push(spec);
      }
   }
   if (raw.length === 0) {
      log('No read-list actions selected.');
      return;
   }
   const dedup = new Map<string, BatchIxSpec>();
   for (const s of raw) {
      dedup.set(batchSpecKey(s), s);
   }
   const sorted = sortBatchSpecs([...dedup.values()]);
   for (const s of sorted) {
      if (String(s.owner) !== String(owner)) {
         throw new Error('Selected vault must be owned by the connected wallet.');
      }
   }
   const ixs = await Promise.all(sorted.map((s) => instructionFromBatchSpec(owner, s)));
   await sendOwnerSignedInstructions(ixs, formatBatchTxDescription(sorted));
}

function appAddrFrom(inputId: string): Address {
   return parseAddr('App program id', (document.querySelector(inputId) as HTMLInputElement).value);
}

function mintAddrFrom(inputId: string): Address {
   return parseAddr('Mint', (document.querySelector(inputId) as HTMLInputElement).value);
}

function delegateFrom(inputId: string, ownerFallback: Address): Address {
   const raw = (document.querySelector(inputId) as HTMLInputElement).value.trim();
   return raw ? address(raw) : ownerFallback;
}

async function onDisconnect(): Promise<void> {
   const client = connectorClient;
   if (client) {
      await client.disconnectWallet();
   }
   walletStatus.textContent = 'Not connected';
   walletStatus.removeAttribute('title');
   updateWalletHeaderChrome();
   log('Wallet disconnected.');
}

async function onCreateVault(): Promise<void> {
   const owner = getConnectedAccountAddress();
   const app = appAddrFrom('#create-app');
   const del = delegateFrom('#create-delegate', owner);
   const exp = selectedDelegateExpires;
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, owner, app);
   const ix = getCreateUserVaultInstruction({
      accounts: { owner, userVaultPda: pda, appAddress: app, delegate: del },
      data: { delegateExpires: exp },
   });
   const expLabel = exp === NO_DELEGATE_EXPIRY ? 'never' : String(exp);
   await sendOwnerSignedIx(
      ix,
      `create vault (app ${shortAddr(app)}, delegate ${shortAddr(del)}, delegateExpires ${expLabel})`,
   );
}

async function onDeposit(): Promise<void> {
   const owner = getConnectedAccountAddress();
   const app = appAddrFrom('#deposit-app');
   const mint = mintAddrFrom('#deposit-mint');
   const tp = tokenProgramFromSelect('#deposit-token-program');
   const ui = (document.querySelector('#deposit-amount-ui') as HTMLInputElement).value;
   const dec = Number((document.querySelector('#deposit-decimals') as HTMLInputElement).value);
   const amt = parseHumanTokenAmount(ui, dec);
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, owner, app);
   const userVaultAta = await deriveUserVaultAtaAddress(pda, mint, tp);
   const sourceAta = await deriveAtaAddress(owner, mint, tp);
   const ix = getDepositUserVaultInstruction({
      accounts: {
         owner,
         userVaultPda: pda,
         userVaultAta,
         appAddress: app,
         sourceAta,
         mint,
         tokenProgram: tp,
      },
      data: { amount: amt },
   });
   await sendOwnerSignedIx(
      ix,
      `deposit SPL (UI "${ui.trim()}", ${dec} decimals → ${amt} base units, mint ${shortAddr(mint)}, app ${shortAddr(app)})`,
   );
}

async function onWithdraw(): Promise<void> {
   const owner = getConnectedAccountAddress();
   const app = appAddrFrom('#withdraw-app');
   const mint = mintAddrFrom('#withdraw-mint');
   const tp = tokenProgramFromSelect('#withdraw-token-program');
   const ui = (document.querySelector('#withdraw-amount-ui') as HTMLInputElement).value;
   const dec = Number((document.querySelector('#withdraw-decimals') as HTMLInputElement).value);
   const amt = parseHumanTokenAmount(ui, dec);
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, owner, app);
   const userVaultAta = await deriveUserVaultAtaAddress(pda, mint, tp);
   const destAta = await deriveAtaAddress(owner, mint, tp);
   const ix = getWithdrawUserVaultInstruction({
      accounts: {
         owner,
         userVaultPda: pda,
         userVaultAta,
         appAddress: app,
         destAta,
         mint,
         tokenProgram: tp,
      },
      data: { amount: amt },
   });
   await sendOwnerSignedIx(
      ix,
      `withdraw SPL (UI "${ui.trim()}", ${dec} decimals → ${amt} base units, mint ${shortAddr(mint)}, app ${shortAddr(app)})`,
   );
}

async function onChangeDelegate(): Promise<void> {
   const owner = getConnectedAccountAddress();
   const app = appAddrFrom('#update-app');
   const del = delegateFrom('#update-delegate', owner);
   const exp = selectedDelegateExpires;
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, owner, app);
   const ix = getUpdateUserVaultDelegateInstruction({
      accounts: { owner, userVaultPda: pda, appAddress: app, delegate: del },
      data: { delegateExpires: exp },
   });
   const expLabel = exp === NO_DELEGATE_EXPIRY ? 'never' : String(exp);
   await sendOwnerSignedIx(
      ix,
      `update delegate (app ${shortAddr(app)}, delegate ${shortAddr(del)}, delegateExpires ${expLabel})`,
   );
}

async function onCloseAta(): Promise<void> {
   const owner = getConnectedAccountAddress();
   const app = appAddrFrom('#close-ata-app');
   const mint = mintAddrFrom('#close-ata-mint');
   const tp = tokenProgramFromSelect('#close-ata-token-program');
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, owner, app);
   const userVaultAta = await deriveUserVaultAtaAddress(pda, mint, tp);
   const ix = getCloseVaultAtaInstruction({
      accounts: {
         owner,
         userVaultPda: pda,
         appAddress: app,
         userVaultAta,
         destination: owner,
         mint,
         tokenProgram: tp,
      },
   });
   await sendOwnerSignedIx(
      ix,
      `close vault ATA (mint ${shortAddr(mint)}, app ${shortAddr(app)}, vault PDA ${shortAddr(pda)})`,
   );
}

async function onCloseVault(): Promise<void> {
   const owner = getConnectedAccountAddress();
   const app = appAddrFrom('#close-vault-app');
   const [pda] = await deriveUserVaultPda(VAULT_PROGRAM_ADDRESS, owner, app);
   const ix = getCloseUserVaultInstruction({
      accounts: { owner, userVaultPda: pda, appAddress: app },
   });
   await sendOwnerSignedIx(ix, `close vault (app ${shortAddr(app)}, vault PDA ${shortAddr(pda)})`);
}

type TokenRow = {
   ata: Address;
   mint?: Address;
   amount: string;
   program: string;
   tp: 'spl' | 'token2022';
};

function tokenAmountFlags(amountStr: string): { positive: boolean; zero: boolean } {
   if (!/^\d+$/.test(amountStr)) {
      return { positive: false, zero: false };
   }
   const b = BigInt(amountStr);
   return { positive: b > 0n, zero: b === 0n };
}

function batchIxDataAttrs(p: {
   kind: BatchKind;
   owner: Address;
   app: Address;
   vault: Address;
   mint?: Address;
   ata?: Address;
   amount?: string;
   tp: 'spl' | 'token2022';
   /** Links withdraw + close ATA for the same SPL account row. */
   pair?: string;
   /** When set on close ATA: enabled only after Withdraw for the same pair is checked. */
   requiresWithdrawGate?: boolean;
}): string {
   const parts = [
      `data-kind="${p.kind}"`,
      `data-owner="${escapeAttr(String(p.owner))}"`,
      `data-app="${escapeAttr(String(p.app))}"`,
      `data-vault="${escapeAttr(String(p.vault))}"`,
      `data-tp="${p.tp}"`,
   ];
   if (p.mint) {
      parts.push(`data-mint="${escapeAttr(String(p.mint))}"`);
   }
   if (p.ata) {
      parts.push(`data-ata="${escapeAttr(String(p.ata))}"`);
   }
   if (p.amount !== undefined) {
      parts.push(`data-amount="${escapeAttr(p.amount)}"`);
   }
   if (p.pair) {
      parts.push(`data-pair="${escapeAttr(p.pair)}"`);
   }
   if (p.requiresWithdrawGate) {
      parts.push('data-requires-withdraw="1"');
   }
   return parts.join(' ');
}

function ataPairId(vault: Address, ata: Address): string {
   return `${String(vault)}__${String(ata)}`;
}

function syncWithdrawPartnerCloseAta(withdrawEl: HTMLInputElement): void {
   const pair = withdrawEl.dataset.pair;
   if (!pair) {
      return;
   }
   const closeEl = readResults.querySelector(
      `input.batch-ix[data-kind="closeAta"][data-pair="${CSS.escape(pair)}"]`,
   ) as HTMLInputElement | null;
   if (!closeEl || closeEl.dataset.requiresWithdraw !== '1') {
      return;
   }
   if (!withdrawEl.checked) {
      closeEl.disabled = true;
      closeEl.checked = false;
   } else {
      closeEl.disabled = false;
   }
}

function refreshCloseVaultForVault(vaultAddr: string): void {
   const cv = readResults.querySelector(
      `input.batch-close-vault[data-vault="${CSS.escape(vaultAddr)}"]`,
   ) as HTMLInputElement | null;
   if (!cv) {
      return;
   }
   const onChain = Number(cv.dataset.ataCount ?? '0');
   if (onChain === 0) {
      cv.disabled = false;
      return;
   }
   const closeAtas = Array.from(
      readResults.querySelectorAll<HTMLInputElement>(
         `input.batch-ix[data-kind="closeAta"][data-vault="${CSS.escape(vaultAddr)}"]`,
      ),
   );
   if (closeAtas.length === 0) {
      cv.disabled = true;
      cv.checked = false;
      return;
   }
   const allReady = closeAtas.every((el) => !el.disabled && el.checked);
   cv.disabled = !allReady;
   if (cv.disabled) {
      cv.checked = false;
   }
}

function syncAllReadResultsBatchGates(): void {
   for (const w of Array.from(readResults.querySelectorAll<HTMLInputElement>('input.batch-ix[data-kind="withdraw"]'))) {
      syncWithdrawPartnerCloseAta(w);
   }
   for (const cv of Array.from(readResults.querySelectorAll<HTMLInputElement>('input.batch-close-vault'))) {
      const v = cv.dataset.vault;
      if (v) {
         refreshCloseVaultForVault(v);
      }
   }
}

function onReadResultsBatchChange(ev: Event): void {
   const t = ev.target;
   if (!(t instanceof HTMLInputElement) || !t.classList.contains('batch-ix')) {
      return;
   }
   const kind = t.dataset.kind;
   if (kind === 'withdraw') {
      syncWithdrawPartnerCloseAta(t);
      const v = t.dataset.vault;
      if (v) {
         refreshCloseVaultForVault(v);
      }
   } else if (kind === 'closeAta') {
      const v = t.dataset.vault;
      if (v) {
         refreshCloseVaultForVault(v);
      }
   }
}

async function tokenAccountsForVault(
   rpc: ReturnType<typeof createRpc>,
   vaultPda: Address,
   programLabel: string,
   programId: Address,
): Promise<TokenRow[]> {
   const { value: tokenAccounts } = await rpc
      .getTokenAccountsByOwner(vaultPda, { programId }, { encoding: 'jsonParsed' })
      .send();
   const rows: TokenRow[] = [];
   const tp: 'spl' | 'token2022' =
      String(programId) === String(TOKEN_2022_PROGRAM_ADDRESS) ? 'token2022' : 'spl';
   for (const row of tokenAccounts) {
      const data = row.account.data as unknown;
      if (typeof data === 'object' && data && 'parsed' in data) {
         const parsed = (data as { parsed?: { info?: { mint?: string; tokenAmount?: { amount?: string } } } })
            .parsed;
         const info = parsed?.info;
         rows.push({
            ata: row.pubkey,
            mint: info?.mint ? address(info.mint) : undefined,
            amount: info?.tokenAmount?.amount ?? '?',
            program: programLabel,
            tp,
         });
      }
   }
   return rows;
}

function expiryCellHtml(delegateExpires: number): string {
   if (delegateExpires === NO_DELEGATE_EXPIRY) {
      return 'Never';
   }
   const d = new Date(delegateExpires * 1000);
   const line1 = escapeHtml(String(delegateExpires));
   const line2 = escapeHtml(d.toLocaleString(undefined, { dateStyle: 'short', timeStyle: 'short' }));
   return `${line1}<br><span style="color:#64748b;font-size:0.72rem">${line2}</span>`;
}

async function renderVaultsTable(
   target: HTMLElement,
   rpc: ReturnType<typeof createRpc>,
   vaults: Awaited<ReturnType<typeof fetchUserVaultAccountsByFilter>>,
): Promise<void> {
   if (vaults.length === 0) {
      target.innerHTML = '<p class="empty">No vault accounts found for this query.</p>';
      readBatchBar.hidden = true;
      return;
   }

   const connected = tryGetConnectedAccountAddress();
   const chunks: string[] = [];
   chunks.push('<table class="vaults">');
   chunks.push(
      '<thead><tr><th>Vault PDA</th><th>Owner</th><th>App</th><th>Delegate</th><th>ATAs</th><th>Delegate expires</th><th title="Lamports above vault rent reserve (' +
         String(VAULT_PDA_RENT_LAMPORTS) +
         ' lamports), shown as SOL">SOL balance</th></tr></thead>',
   );
   chunks.push('<tbody>');

   for (const v of vaults) {
      const { value: lamports } = await rpc.getBalance(v.address).send();
      const lamportsBn = BigInt(lamports);
      const solCell = escapeHtml(solBalanceMinusVaultRentDisplay(lamportsBn));
      const classic = await tokenAccountsForVault(rpc, v.address, 'SPL Token', SPL_TOKEN_PROGRAM_ADDRESS);
      const t22 = await tokenAccountsForVault(rpc, v.address, 'Token-2022', TOKEN_2022_PROGRAM_ADDRESS);
      const tokens = [...classic, ...t22];
      const ownerCanSign =
         connected !== null && String(v.data.owner) === String(connected);

      chunks.push('<tr>');
      chunks.push(`<td class="mono">${escapeHtml(String(v.address))}</td>`);
      chunks.push(`<td class="mono">${escapeHtml(String(v.data.owner))}</td>`);
      chunks.push(`<td class="mono">${escapeHtml(String(v.data.appAddress))}</td>`);
      chunks.push(`<td class="mono">${escapeHtml(String(v.data.delegate))}</td>`);
      chunks.push(`<td class="num">${v.data.ataCount}</td>`);
      chunks.push(`<td class="num">${expiryCellHtml(v.data.delegateExpires)}</td>`);
      chunks.push(`<td class="num" title="Raw lamports: ${escapeHtml(String(lamportsBn))}">${solCell} SOL</td>`);
      chunks.push('</tr>');

      if (tokens.length === 0) {
         chunks.push(
            `<tr><td colspan="7" style="background:#fafafa;color:#64748b;font-size:0.78rem">No SPL token accounts on this vault PDA.</td></tr>`,
         );
      } else {
         chunks.push('<tr><td colspan="7" style="padding:0.35rem 0.55rem 0.65rem;background:#fafafa">');
         chunks.push('<table class="tokens"><thead><tr>');
         chunks.push('<th>Program</th><th>ATA</th><th>Mint</th><th>Amount (raw)</th><th>Action</th>');
         chunks.push('</tr></thead><tbody>');
         for (const t of tokens) {
            const { positive, zero } = tokenAmountFlags(t.amount);
            const mintOk = Boolean(t.mint);
            let actionHtml: string;
            if (!ownerCanSign) {
               actionHtml = '<span class="muted">—</span>';
            } else if (!mintOk) {
               actionHtml = '<span class="muted">—</span>';
            } else if (positive) {
               const pairId = ataPairId(v.address, t.ata);
               const withdrawLbl = `<label class="batch-ix-wrap"><input type="checkbox" class="batch-ix" ${batchIxDataAttrs({
                  kind: 'withdraw',
                  owner: v.data.owner,
                  app: v.data.appAddress,
                  vault: v.address,
                  mint: t.mint,
                  ata: t.ata,
                  amount: t.amount,
                  tp: t.tp,
                  pair: pairId,
               })} /> Withdraw</label>`;
               const closeAtaLbl = `<label class="batch-ix-wrap"><input type="checkbox" class="batch-ix" disabled ${batchIxDataAttrs({
                  kind: 'closeAta',
                  owner: v.data.owner,
                  app: v.data.appAddress,
                  vault: v.address,
                  mint: t.mint,
                  ata: t.ata,
                  tp: t.tp,
                  pair: pairId,
                  requiresWithdrawGate: true,
               })} /> Close ATA</label>`;
               actionHtml = `<span class="batch-action-pair">${withdrawLbl}${closeAtaLbl}</span>`;
            } else if (zero) {
               actionHtml = `<label class="batch-ix-wrap"><input type="checkbox" class="batch-ix" ${batchIxDataAttrs({
                  kind: 'closeAta',
                  owner: v.data.owner,
                  app: v.data.appAddress,
                  vault: v.address,
                  mint: t.mint,
                  ata: t.ata,
                  tp: t.tp,
               })} /> Close ATA</label>`;
            } else {
               actionHtml = '<span class="muted">—</span>';
            }
            chunks.push('<tr>');
            chunks.push(`<td>${escapeHtml(t.program)}</td>`);
            chunks.push(`<td class="mono">${escapeHtml(String(t.ata))}</td>`);
            chunks.push(`<td class="mono">${escapeHtml(t.mint ? String(t.mint) : '?')}</td>`);
            chunks.push(`<td class="num">${escapeHtml(t.amount)}</td>`);
            chunks.push(`<td class="batch-cell">${actionHtml}</td>`);
            chunks.push('</tr>');
         }
         chunks.push('</tbody></table></td></tr>');
      }

      if (ownerCanSign) {
         const onChain = v.data.ataCount;
         const cvDisabled = onChain > 0 ? ' disabled' : '';
         const cvAttrs = batchIxDataAttrs({
            kind: 'closeVault',
            owner: v.data.owner,
            app: v.data.appAddress,
            vault: v.address,
            tp: 'spl',
         });
         chunks.push(
            `<tr><td colspan="7" style="background:#f0fdf4;padding:0.4rem 0.55rem;font-size:0.82rem"><label class="batch-ix-wrap"><input type="checkbox" class="batch-ix batch-close-vault"${cvDisabled} data-ata-count="${onChain}" ${cvAttrs} /> Close vault</label></td></tr>`,
         );
      } else if (v.data.ataCount === 0) {
         chunks.push(
            `<tr><td colspan="7" style="font-size:0.8rem;color:#64748b;padding:0.25rem 0.55rem">Close vault: connect as vault owner to sign.</td></tr>`,
         );
      }
   }

   chunks.push('</tbody></table>');
   target.innerHTML = chunks.join('');
   syncAllReadResultsBatchGates();
   readBatchBar.hidden = false;
}

async function runVaultQuery(mode: 'wallet' | 'app' | 'both'): Promise<void> {
   readResults.innerHTML = '<p class="empty">Loading…</p>';
   readBatchBar.hidden = true;
   const rpc = createRpc();
   const qw = (document.querySelector('#query-wallet') as HTMLInputElement).value.trim();
   const qa = (document.querySelector('#query-app') as HTMLInputElement).value.trim();
   const filter: { owner?: Address; appAddress?: Address } = {};
   if (mode === 'wallet' || mode === 'both') {
      if (!qw) {
         const err = 'Set “Owner (wallet) filter” or connect a wallet so the field can default.';
         readResults.innerHTML = `<p class="err">${escapeHtml(err)}</p>`;
         throw new Error(err);
      }
      filter.owner = address(qw);
   }
   if (mode === 'app' || mode === 'both') {
      if (!qa) {
         const err = 'Set “App program” for this query.';
         readResults.innerHTML = `<p class="err">${escapeHtml(err)}</p>`;
         throw new Error(err);
      }
      filter.appAddress = address(qa);
   }
   log(`fetchUserVaultAccountsByFilter(${JSON.stringify({ owner: filter.owner, app: filter.appAddress })})`);
   try {
      const vaults = await fetchUserVaultAccountsByFilter(rpc, filter);
      await renderVaultsTable(readResults, rpc, vaults);
      log(`Found ${vaults.length} vault(s).`);
   } catch (e) {
      const msg = String(e);
      readResults.innerHTML = `<p class="err">${escapeHtml(msg)}</p>`;
      readBatchBar.hidden = true;
      throw e;
   }
}

function wireUi(): void {
   btnWalletAction.addEventListener('click', () => {
      if (!connectorClient) {
         return;
      }
      const s = connectorClient.getSnapshot();
      if (isConnected(s.wallet)) {
         void onDisconnect().catch((e) => log(String(e)));
      } else if (s.wallet.status !== 'connecting') {
         openWalletModal();
      }
   });
   btnWalletModalClose.addEventListener('click', () => closeWalletModal());
   walletModalScrim.addEventListener('click', () => closeWalletModal());
   document.addEventListener('keydown', (ev) => {
      if (ev.key === 'Escape' && walletModalIsOpen()) {
         closeWalletModal();
      }
   });
   document
      .querySelector('#btn-create-vault')!
      .addEventListener('click', () => void onCreateVault().catch((e) => log(String(e))));
   document.querySelector('#btn-deposit')!.addEventListener('click', () => void onDeposit().catch((e) => log(String(e))));
   document
      .querySelector('#btn-withdraw')!
      .addEventListener('click', () => void onWithdraw().catch((e) => log(String(e))));
   document
      .querySelector('#btn-change-delegate')!
      .addEventListener('click', () => void onChangeDelegate().catch((e) => log(String(e))));
   document
      .querySelector('#btn-close-ata')!
      .addEventListener('click', () => void onCloseAta().catch((e) => log(String(e))));
   document
      .querySelector('#btn-close-vault')!
      .addEventListener('click', () => void onCloseVault().catch((e) => log(String(e))));
   document
      .querySelector('#btn-vaults-wallet')!
      .addEventListener('click', () => void runVaultQuery('wallet').catch((e) => log(String(e))));
   document
      .querySelector('#btn-vaults-app')!
      .addEventListener('click', () => void runVaultQuery('app').catch((e) => log(String(e))));
   document
      .querySelector('#btn-vaults-both')!
      .addEventListener('click', () => void runVaultQuery('both').catch((e) => log(String(e))));
   document
      .querySelector('#btn-run-selected')!
      .addEventListener('click', () => void onRunSelectedReadActions().catch((e) => log(String(e))));
   readResults.addEventListener('change', onReadResultsBatchChange);

   for (const btn of Array.from(document.querySelectorAll<HTMLButtonElement>('.preset-expiry'))) {
      btn.addEventListener('click', () => {
         const p = btn.dataset.preset as '1h' | '1d' | 'never' | undefined;
         if (p) {
            setDelegatePreset(p);
         }
      });
   }

   const reinit = () => {
      void onDisconnect().finally(() => {
         initConnectorClient();
         log('Cluster changed — wallet disconnected. Connect again.');
      });
   };
   document.querySelector('#cluster')!.addEventListener('change', reinit);
}

wireUi();
initConnectorClient();
setDelegatePreset('never');
log(`Vault program: ${VAULT_PROGRAM_ADDRESS}`);
