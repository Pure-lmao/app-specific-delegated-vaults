# App-Specific Delegated Vaults for Solana

Users deposit SOL and tokens into a **per-app vault**. Your app spends from it with a **delegate key** -- no wallet popup on every action.

One program, deployed once (immutable once on mainnet), shared by every app on the network.

**Deployed program id:** `ASdvz39AFXXEGcYadGYPYhHprGadw1AeydQPqn4GqrV1` (asdv...v1)

---

## The problem

Every time your dApp needs to move funds on behalf of a user, the user has to pull out their wallet and sign. For anything real-time -- games, trading, subscriptions, social tipping -- that kills the experience.

Token `approve` / `delegate` exists, but it is scoped to a single token account with a flat allowance and no program-level gating. There is no expiry, no per-app isolation, and no way to let a delegate key invoke your program with the vault as a signer.

## What this gives you

- **Per-user, per-app vault** -- PDA derived from `["vault", owner, your_program_id]`. Your users' funds are isolated from every other app using the same vault program.
- **Delegate-authorized spending** -- Assign a delegate key (ephemeral keypair, session key, server key) that can spend SOL and SPL tokens from the vault, gated to your program.
- **Built-in expiry** -- `delegate_expires` is a Unix timestamp. Set it to `now + 1 hour` and the chain enforces the session window. Use `u32::MAX` when you do not want expiry within the representable range.
- **Owner always in control** -- The user can withdraw, rotate the delegate, or close the vault at any time without the delegate's involvement.
- **SOL + SPL + Token-2022** -- Native lamports live on the PDA itself. SPL tokens (classic and Token-2022) live in vault ATAs, created on first deposit.

## Quick example: session keys (TypeScript)

User connects, you generate a throwaway keypair, set it as delegate with a 1-hour window, and your frontend/backend uses it to act on their behalf until it expires.

```typescript
import { address, generateKeyPair, getAddressFromPublicKey } from '@solana/kit';
import {
   VAULT_PROGRAM_ADDRESS,
   deriveUserVaultAtaAddress,
   deriveUserVaultPda,
   getCreateUserVaultInstruction,
   getDepositUserVaultInstruction,
   getUpdateUserVaultDelegateInstruction,
   SPL_TOKEN_PROGRAM_ADDRESS,
} from '@vault/sdk';

const APP_ADDRESS = address('YourProgram1111111111111111111111111111111');
// `ownerAddress` / `userTokenAccount` / `mintAddress`: `Address` values from the wallet and mint config.

// 1. Derive the user's vault PDA for your app
const [userVaultPda] = await deriveUserVaultPda(
   VAULT_PROGRAM_ADDRESS, ownerAddress, APP_ADDRESS,
);

// 2. Create the vault with a session delegate (1-hour expiry)
const sessionKey = await generateKeyPair();
const oneHourFromNow = Math.floor(Date.now() / 1000) + 3600;

const createIx = getCreateUserVaultInstruction({
   accounts: {
      owner: ownerAddress,
      userVaultPda,
      appAddress: APP_ADDRESS,
      delegate: await getAddressFromPublicKey(sessionKey.publicKey),
   },
   data: { delegateExpires: oneHourFromNow },
});

// 3. User deposits tokens (owner signs) — classic SPL; use `TOKEN_2022_PROGRAM_ADDRESS` for Token-2022
const vaultAta = await deriveUserVaultAtaAddress(
   userVaultPda, mintAddress, SPL_TOKEN_PROGRAM_ADDRESS,
);
const depositIx = getDepositUserVaultInstruction({
   accounts: {
      owner: ownerAddress,
      userVaultPda,
      userVaultAta: vaultAta,
      appAddress: APP_ADDRESS,
      sourceAta: userTokenAccount,
      mint: mintAddress,
      tokenProgram: SPL_TOKEN_PROGRAM_ADDRESS,
   },
   data: { amount: 1_000_000n },
});

// 4. Later: rotate to a new session key (owner signs once)
const newSessionKey = await generateKeyPair();
const rotateIx = getUpdateUserVaultDelegateInstruction({
   accounts: {
      owner: ownerAddress,
      userVaultPda,
      appAddress: APP_ADDRESS,
      delegate: await getAddressFromPublicKey(newSessionKey.publicKey),
   },
   data: { delegateExpires: Math.floor(Date.now() / 1000) + 3600 },
});
```

From here, the session key signs delegate instructions (`AppIx`, `CpiEntry`, `CpiEntryNative`) without the user's wallet -- until the expiry or until the user rotates.

## Two integration paths

### `AppIx` -- vault wraps your program

Use when your on-chain program **already exists** and you want to add vault spending on top. The delegate signs a vault instruction; the vault validates the delegate, then **CPIs into your program** with the vault PDA as a signer. Your program receives the call and does its thing.

Good for: retrofitting vault support onto an existing program, or when your program needs arbitrary account layouts and custom logic beyond simple transfers.

### `CpiEntry` / `CpiEntryNative` -- your program wraps the vault

Use when you are **writing** the program and want it to be the top-level entry point. Your program is the first instruction in the transaction; it CPIs into the vault to pull SOL/tokens. The vault checks the `instructions_sysvar` to confirm the top-level instruction belongs to your program -- so nobody else can call this path.

Good for: new programs designed around vault integration, where you control the full transaction layout.

## Delegate patterns

### Session key with expiry

Generate an ephemeral keypair, set it as delegate with `delegate_expires = now + N`. The chain rejects delegate actions after that timestamp (`ExpiredDelegate`). When the session ends, the key is worthless. Extend with `UpdateUserVaultDelegate` or let it lapse.

### Frontend-generated hot key

Generate a frontend keypair via with a strong password and something like Argon2id to allow cross-session, cross-device user experience that feels like a web2 app with 0-signing every transaction. Ability to use the delegate key on one program, not withdraw funds reduces, and quickly rotate it reduces the reward for cracking the password.

### Hot vault owned by cold/hardware wallet

Create a hot vault owned by a cold/hardware wallet. The hot vault is used for daily operations on specific programs, while the cold wallet remains unconnected beyond set-up and delegate key rotation.

### Server-side delegate

The delegate is a key controlled by your backend. The user only signs vault creation, deposits, and withdrawals. The server signs all delegated spends. Works well for custodial-lite flows where the user trusts your service but keeps withdrawal rights.

## Instruction reference

Single-byte discriminator as the first byte of instruction data.

| # | Instruction | Signer | What it does |
| ---: | --- | --- | --- |
| 0 | `CreateUserVault` | Owner | Create the vault PDA `["vault", owner, app_address]`; set delegate + expiry. |
| 1 | `DepositUserVault` | Owner | SPL deposit into vault ATA (creates ATA on first use). |
| 2 | `UpdateUserVaultDelegate` | Owner | Swap delegate key and/or change expiry. |
| 3 | `WithdrawUserVault` | Owner | SPL withdrawal to owner's ATA. |
| 4 | `WithdrawUserVaultNative` | Owner | SOL withdrawal to owner. |
| 5 | `AppIx` | Delegate | CPI into `app_address` with arbitrary inner ix; vault PDA signs. |
| 6 | `CpiEntry` | Delegate | Pull SOL + SPL from vault (top-level ix must be `app_address`). |
| 7 | `CpiEntryNative` | Delegate | Pull SOL only from vault (top-level ix must be `app_address`). |
| 8 | `CloseVaultAta` | Owner | Close an empty vault ATA; decrements `ata_count`. |
| 9 | `CloseUserVault` | Owner | Close vault PDA and reclaim rent (requires `ata_count == 0`). |

### Vault account layout

```
UserVaultAccount (104 bytes, #[repr(C)] on-chain):
  discriminator    u8       (always 1 — `USER_VAULT_DISCRIMINATOR`, matches RPC memcmp filters in the SDK)
  bump             u8       (PDA bump seed)
  ata_count        u16      (open vault ATAs)
  delegate_expires u32      (Unix seconds; use u32::MAX for no practical expiry)
  owner            Pubkey   (32 bytes)
  app_address      Pubkey   (32 bytes)
  delegate         Pubkey   (32 bytes)
```

Custom errors: see `program/src/error.rs`.

## Security notes

- **`CpiEntry` / `CpiEntryNative`** verify the top-level instruction is from `app_address` via the instructions sysvar. Don't assume they're safe to call outside that transaction structure.
- **`AppIx`** CPIs into your program -- your program is the callee and must validate what it does with the vault PDA signer.
- **Owner withdrawals** enforce the destination ATA is owned by the owner.
- **`delegate_expires`** uses the on-chain `Clock` sysvar. Delegate actions require `unix_timestamp <= delegate_expires` (as `u32`). Set `delegate_expires` to `u32::MAX` for no practical expiry.

## Demo

The [`demo/`](demo/) app is a small Vite + TypeScript page that connects a wallet, queries vault accounts via the TypeScript SDK, and runs the main owner instructions (create, deposit, withdraw, delegate update, close ATA, close vault). It resolves `@vault/sdk` to `sdk/ts` through the Vite alias, so you can try UI changes against the local SDK without publishing a package.

```bash
cd demo && npm install && npm run dev
```

### Flip demo (`flip_demo/`)

A small **end-to-end app flow** sample: wallet connect, vault create/deposit, delegate key in the page, and a toy flip game that spends from the vault via delegate-signed instructions. It targets **Solana devnet** with devnet mints/RPC baked into the UI; treat it as a **walkthrough only**—not hardened, not audited, and not something to copy for mainnet security.

```bash
cd flip_demo/ui ; npm install ; npm run dev
```

---

## Development

<details>
<summary>Repository layout, building, and testing</summary>

### Repository layout

| Path | What |
| --- | --- |
| `program/` | On-chain vault program (Pinocchio, `no_std`). |
| `test_program/` | Companion program for CPI integration tests. |
| `sdk/rust/` | `vault-sdk` crate -- instruction builders, PDA derivation, account parsing. |
| `sdk/ts/` | `@vault/sdk` package (`@solana/kit`) -- TypeScript mirrors of the Rust SDK. |
| `demo/` | Browser demo (`npm run dev` in that folder) -- wallet + vault flows against devnet or other clusters. |
| `program/tests/` | Mollusk integration tests. |
| `testing/` | Node scripts and experiments. |
| `flip_demo/` | Flip demo app to show how an app can use the smooth flow allowed by the vault program |

### Building

```bash
# vault program
cd program && cargo build-sbf

# test program (needed for integration tests)
cd test_program && cargo build-sbf

# TypeScript SDK
cd sdk/ts && npm run build
```

### Running tests

Integration tests use [Mollusk](https://github.com/buffalojoec/mollusk) against the built BPF artifacts.

```bash
cd program
cargo vault-test
# or: scripts/run-vault-tests.ps1 / run-vault-tests.sh
```

Requires the `test-sbf` feature. Test modules wired in `tests/mollusk_vault.rs`:

`instruction_router`, `create_user_vault`, `deposit_user_vault`, `update_user_vault_delegate`, `withdraw_user_vault`, `withdraw_user_vault_native`, `app_ix`, `cpi_entry`, `cpi_entry_native`, `close_vault_ata`, `close_user_vault`, `delegate_expiry`, `error_paths`

See comments in `tests/mollusk_vault.rs` for CU logging and report generation.

</details>

## License

Licensed under **CC BY-NC 4.0** (Creative Commons Attribution-NonCommercial 4.0 International). You can share and adapt with attribution, but **commercial use of this code is not permitted**. See [`LICENSE`](LICENSE) and the [full legal text](https://creativecommons.org/licenses/by-nc/4.0/legalcode).

Ideas, algorithms, and independent re-implementations inspired by this project are not restricted by this license -- it covers the expression in this repository, not the concepts.
