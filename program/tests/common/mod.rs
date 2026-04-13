//! Shared Mollusk helpers: program ids, PDAs, instruction payloads, SPL fixtures, dual BPF load.

use mollusk_svm::{
   instructions_sysvar,
   program::{
      create_program_account_pair_loader_v3, keyed_account_for_system_program, loader_keys,
   },
   result::InstructionResult,
   Mollusk,
};
use solana_instruction::Instruction;
use mollusk_svm_programs_token::{associated_token, token};
use solana_account::Account;
use solana_program_error::ProgramError;
use solana_program_option::COption;
use solana_pubkey::Pubkey;
use solana_sdk_ids::sysvar::clock as clock_sysvar;
use solana_sdk_ids::sysvar::rent as rent_sysvar;
use spl_associated_token_account_interface::address::get_associated_token_address_with_program_id;
use spl_token_interface::state::{Account as SplTokenAccount, AccountState, Mint};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub fn vault_program_id() -> Pubkey {
   Pubkey::new_from_array(app_specific_delegated_vaults::constants::ID.to_bytes())
}

pub fn test_program_id() -> Pubkey {
   Pubkey::new_from_array(test_program::constants::ID.to_bytes())
}

pub fn custom_vault_err(e: app_specific_delegated_vaults::error::Error) -> ProgramError {
   ProgramError::Custom(e as u32)
}

/// `program/target/deploy` (for `app_specific_delegated_vaults.so` via `SBF_OUT_DIR`).
fn set_sbf_out_dir_vault() {
   let deploy = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/deploy");
   std::env::set_var("SBF_OUT_DIR", deploy.to_string_lossy().as_ref());
}

fn test_program_elf_path() -> PathBuf {
   PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../test_program/target/deploy/test_program.so")
}

/// BPF loader v3 program + programdata (required in the tx when upgradeable programs appear in the message).
fn vault_loader_accounts() -> Vec<(Pubkey, Account)> {
   let elf = std::fs::read(
      PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/deploy/app_specific_delegated_vaults.so"),
   )
   .expect("app_specific_delegated_vaults.so missing; run `cargo build-sbf` in program/");
   let pid = vault_program_id();
   let (prog, data) = create_program_account_pair_loader_v3(&pid, &elf);
   let programdata = Pubkey::find_program_address(&[pid.as_ref()], &loader_keys::LOADER_V3).0;
   vec![(pid, prog), (programdata, data)]
}

fn test_program_loader_accounts() -> Vec<(Pubkey, Account)> {
   let elf = std::fs::read(test_program_elf_path()).expect("test_program.so missing");
   let pid = test_program_id();
   let (prog, data) = create_program_account_pair_loader_v3(&pid, &elf);
   let programdata = Pubkey::find_program_address(&[pid.as_ref()], &loader_keys::LOADER_V3).0;
   vec![(pid, prog), (programdata, data)]
}

pub fn all_loader_accounts() -> Vec<(Pubkey, Account)> {
   let mut v = vault_loader_accounts();
   v.extend(test_program_loader_accounts());
   v
}

pub fn instructions_sysvar_for(ix: &Instruction) -> (Pubkey, Account) {
   instructions_sysvar::keyed_account(std::iter::once(ix))
}

/// Insert or replace accounts by pubkey (later entries win).
pub fn with_ix_sysvar_and_loaders(ix: &Instruction, user: &[(Pubkey, Account)]) -> Vec<(Pubkey, Account)> {
   let (pk, a) = instructions_sysvar_for(ix);
   // User layer first so placeholder `(app, default)` is overwritten by executable program stubs and real sysvar.
   overlay_accounts(&[user, &all_loader_accounts(), &[(pk, a)]])
}

/// [`clock_sysvar::id`] (for `AccountMeta`); account data must match [`Mollusk::sysvars`] (use [`with_clock_and_loaders`] / [`with_ix_sysvar_clock_and_loaders`]).
pub fn clock_sysvar_pk() -> Pubkey {
   clock_sysvar::id()
}

/// [`rent_sysvar::id`] for `CreateUserVault`; pair with [`rent_sysvar_account`].
pub fn rent_sysvar_pk() -> Pubkey {
   rent_sysvar::id()
}

/// Rent sysvar account serialized from `mollusk.sysvars.rent`.
pub fn rent_sysvar_account(mollusk: &Mollusk) -> (Pubkey, Account) {
   mollusk.sysvars.keyed_account_for_rent_sysvar()
}

/// Loader stubs + Clock sysvar serialized from `mollusk.sysvars.clock` (required for vault `app_ix` etc.).
pub fn with_clock_and_loaders(mollusk: &Mollusk, user: &[(Pubkey, Account)]) -> Vec<(Pubkey, Account)> {
   let (pk, acct) = mollusk.sysvars.keyed_account_for_clock_sysvar();
   with_loader_accounts(&overlay_accounts(&[user, &[(pk, acct)]]))
}

/// Like [`with_ix_sysvar_and_loaders`] but ensures Clock sysvar is present with current `mollusk` sysvar state.
pub fn with_ix_sysvar_clock_and_loaders(
   ix: &Instruction,
   mollusk: &Mollusk,
   user: &[(Pubkey, Account)],
) -> Vec<(Pubkey, Account)> {
   let (pk, acct) = mollusk.sysvars.keyed_account_for_clock_sysvar();
   let user_clock = overlay_accounts(&[user, &[(pk, acct)]]);
   with_ix_sysvar_and_loaders(ix, &user_clock)
}

/// Loader v3 program stubs for vault + test_program when their ids appear as accounts (after user placeholders).
pub fn with_loader_accounts(user: &[(Pubkey, Account)]) -> Vec<(Pubkey, Account)> {
   overlay_accounts(&[user, &all_loader_accounts()])
}

pub fn overlay_accounts(layers: &[&[(Pubkey, Account)]]) -> Vec<(Pubkey, Account)> {
   let mut out: Vec<(Pubkey, Account)> = vec![];
   for layer in layers {
      for (k, a) in *layer {
         if let Some(i) = out.iter().position(|(p, _)| p == k) {
            out[i] = (*k, a.clone());
         } else {
            out.push((*k, a.clone()));
         }
      }
   }
   out
}

/// Mollusk defaults `Clock` to `0`, but the vault rejects `unix_timestamp <= 0` when checking delegate expiry.
const DEFAULT_MOLLUSK_UNIX_TIMESTAMP: i64 = 1_700_000_000;

pub fn fresh_mollusk() -> Mollusk {
   crate::ensure_integration_test_env();
   set_sbf_out_dir_vault();
   let vault_id = vault_program_id();
   let mut mollusk = Mollusk::new(&vault_id, "app_specific_delegated_vaults");
   mollusk.sysvars.clock.unix_timestamp = DEFAULT_MOLLUSK_UNIX_TIMESTAMP;
   token::add_program(&mut mollusk);
   associated_token::add_program(&mut mollusk);
   let elf = std::fs::read(test_program_elf_path()).expect(
      "missing ../test_program/target/deploy/test_program.so — run `cargo build-sbf` in test_program/",
   );
   mollusk.add_program_with_loader_and_elf(&test_program_id(), &loader_keys::LOADER_V3, &elf);
   mollusk
}

/// Fixed keys for bench tests so PDA bumps (and therefore CU) are deterministic across runs.
/// `Pubkey::new_unique()` produces random keys each run → `find_program_address` iterates a
/// different number of SHA-256 hashes → CU jitter.
pub fn bench_owner() -> Pubkey {
   Pubkey::new_from_array([
      1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
   ])
}
pub fn bench_delegate() -> Pubkey {
   Pubkey::new_from_array([
      2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
   ])
}
pub fn bench_mint() -> Pubkey {
   Pubkey::new_from_array([
      3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
   ])
}
pub fn bench_rent_dest() -> Pubkey {
   Pubkey::new_from_array([
      4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
   ])
}
pub fn bench_lamports_dest() -> Pubkey {
   Pubkey::new_from_array([
      5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
   ])
}
pub fn bench_dest_owner() -> Pubkey {
   Pubkey::new_from_array([
      6, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
      0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
   ])
}

pub fn derive_user_vault(owner: &Pubkey, app: &Pubkey) -> (Pubkey, u8) {
   Pubkey::find_program_address(
      &[app_specific_delegated_vaults::constants::USER_VAULT_SEED, owner.as_ref(), app.as_ref()],
      &vault_program_id(),
   )
}

pub fn treasury_pda() -> (Pubkey, u8) {
   Pubkey::find_program_address(&[test_program::constants::TREASURY_SEED], &test_program_id())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedVault {
   pub owner: Pubkey,
   pub app_address: Pubkey,
   pub delegate: Pubkey,
   pub delegate_expires: u32,
   pub ata_count: u16,
   pub bump: u8,
}

pub fn decode_user_vault(data: &[u8]) -> Option<DecodedVault> {
   if data.len() != app_specific_delegated_vaults::state::UserVaultAccount::LEN {
      return None;
   }
   if data[0] != app_specific_delegated_vaults::constants::USER_VAULT_DISCRIMINATOR {
      return None;
   }
   let bump = data[1];
   let ata_count = u16::from_le_bytes(data[2..4].try_into().ok()?);
   let delegate_expires = u32::from_le_bytes(data[4..8].try_into().ok()?);
   let mut o = 8usize;
   let owner = Pubkey::new_from_array(data[o..o + 32].try_into().ok()?);
   o += 32;
   let app_address = Pubkey::new_from_array(data[o..o + 32].try_into().ok()?);
   o += 32;
   let delegate = Pubkey::new_from_array(data[o..o + 32].try_into().ok()?);
   Some(DecodedVault {
      owner,
      app_address,
      delegate,
      delegate_expires,
      ata_count,
      bump,
   })
}

pub fn mint_account(mint_authority: Pubkey, supply: u64) -> Account {
   let mint = Mint {
      mint_authority: COption::Some(mint_authority),
      supply,
      decimals: 6,
      is_initialized: true,
      freeze_authority: COption::None,
   };
   token::create_account_for_mint(mint)
}

pub fn token_account(mint: &Pubkey, owner: &Pubkey, amount: u64) -> Account {
   let t = SplTokenAccount {
      mint: *mint,
      owner: *owner,
      amount,
      delegate: COption::None,
      state: AccountState::Initialized,
      is_native: COption::None,
      delegated_amount: 0,
      close_authority: COption::None,
   };
   token::create_account_for_token_account(t)
}

pub fn signer_account(lamports: u64) -> Account {
   Account {
      lamports,
      ..Account::default()
   }
}

pub fn set_clock_unix(mollusk: &mut Mollusk, unix_timestamp: i64) {
   mollusk.sysvars.clock.unix_timestamp = unix_timestamp;
}

// --- Instruction payloads (first byte = vault discriminator; remainder = handler `data` after strip) ---

pub fn ix_create(delegate_expires: u32) -> Vec<u8> {
   let mut v = vec![0u8];
   v.extend_from_slice(&delegate_expires.to_le_bytes());
   v
}

pub fn ix_deposit(amount: u64) -> Vec<u8> {
   let mut v = vec![1u8];
   v.extend_from_slice(&amount.to_le_bytes());
   v
}

pub fn ix_update_delegate(delegate_expires: u32) -> Vec<u8> {
   let mut v = vec![2u8];
   v.extend_from_slice(&delegate_expires.to_le_bytes());
   v
}

pub fn ix_withdraw(amount: u64) -> Vec<u8> {
   let mut v = vec![3u8];
   v.extend_from_slice(&amount.to_le_bytes());
   v
}

pub fn ix_withdraw_native(amount: u64) -> Vec<u8> {
   let mut v = vec![4u8];
   v.extend_from_slice(&amount.to_le_bytes());
   v
}

pub fn ix_app_ix(inner: &[u8]) -> Vec<u8> {
   let mut v = vec![5u8];
   v.extend_from_slice(inner);
   v
}

pub fn ix_cpi_entry(amount_native: u64, amount_spl: u64) -> Vec<u8> {
   let mut v = vec![6u8];
   v.extend_from_slice(&amount_native.to_le_bytes());
   v.extend_from_slice(&amount_spl.to_le_bytes());
   v
}

pub fn ix_cpi_entry_native(amount_native: u64) -> Vec<u8> {
   let mut v = vec![7u8];
   v.extend_from_slice(&amount_native.to_le_bytes());
   v
}

pub fn ix_close_vault_ata() -> Vec<u8> {
   vec![8u8]
}

pub fn ix_close_user_vault() -> Vec<u8> {
   vec![9u8]
}

pub fn ix_test_deposit_via_cpi(amount: u64) -> Vec<u8> {
   let mut v = vec![0u8];
   v.extend_from_slice(&amount.to_le_bytes());
   v
}

pub fn ix_test_cpi_entry_native_via_vault(amount_native: u64) -> Vec<u8> {
   let mut v = vec![4u8];
   v.extend_from_slice(&amount_native.to_le_bytes());
   v
}

pub fn ix_test_cpi_entry_dual_via_vault(amount_native: u64, amount_spl: u64) -> Vec<u8> {
   let mut v = vec![5u8];
   v.extend_from_slice(&amount_native.to_le_bytes());
   v.extend_from_slice(&amount_spl.to_le_bytes());
   v
}

pub fn system_program_meta() -> (Pubkey, Account) {
   keyed_account_for_system_program()
}

pub fn token_program_id() -> Pubkey {
   token::ID
}

pub fn associated_token_address(wallet: &Pubkey, mint: &Pubkey) -> Pubkey {
   get_associated_token_address_with_program_id(wallet, mint, &token::ID)
}

pub fn merge_accounts(prev: &[(Pubkey, Account)], res: &InstructionResult) -> Vec<(Pubkey, Account)> {
   prev
      .iter()
      .map(|(k, a)| res.get_account(k).map(|u| (*k, u.clone())).unwrap_or((*k, a.clone())))
      .collect()
}

static CU_REPORT_MUTEX: Mutex<()> = Mutex::new(());

fn vault_log_cu_enabled() -> bool {
   match std::env::var("VAULT_LOG_CU") {
      Ok(v) => v == "1" || v.eq_ignore_ascii_case("true"),
      Err(_) => false,
   }
}

fn append_cu_report_line(label: &str, compute_units: u64) {
   if let Ok(path) = std::env::var("VAULT_CU_REPORT") {
      let _lock = CU_REPORT_MUTEX.lock().expect("cu report mutex");
      if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
         let _ = writeln!(f, "{}\t{}", label, compute_units);
      }
   }
}

/// Top-level `test_program` no-op (discriminator 7) for CPI / caller-shell baselines.
pub fn ix_test_bench_noop_top() -> Vec<u8> {
   vec![test_program::TestProgramInstruction::BenchNoopTopLevel as u8]
}

/// Setup / fixture steps: **never** written to `VAULT_CU_REPORT` (keeps bench TSV focused on the ix under test).
/// Still honors `VAULT_LOG_CU` for local debugging.
pub fn log_cu_setup(label: &str, result: &InstructionResult) {
   if vault_log_cu_enabled() {
      eprintln!(
         "vault CU [setup {label}]: {} (instruction + all CPIs in this step)",
         result.compute_units_consumed
      );
   }
}

/// Bench step: append `label<TAB>compute_units` when `VAULT_CU_REPORT` is set (mutex-safe).
///
/// `compute_units_consumed` is the **total CU for the whole simulated step** (all programs + CPIs).
pub fn log_cu_bench(label: &str, result: &InstructionResult) {
   append_cu_report_line(label, result.compute_units_consumed);
   if vault_log_cu_enabled() {
      eprintln!(
         "vault CU [bench {label}]: {} (instruction + all CPIs in this step)",
         result.compute_units_consumed
      );
   }
}

/// `app_ix` attribution: `inner_noop` = same vault ix with inner data `BenchNoopInner` and **no** inner accounts
/// (vault CPI shell + trivial app); `full` = real inner app work. Writes:
/// - `{label_base}:bench_total` — full path
/// - `{label_base}:cu_vault_invoke_shell` — noop-inner path (vault + CPI into empty app)
/// - `{label_base}:cu_inner_app_delta` — `full − shell` (extra CU from real inner ix, mostly app + nested CPIs)
pub fn log_cu_bench_app_ix_split(label_base: &str, inner_noop: &InstructionResult, full: &InstructionResult) {
   let shell = inner_noop.compute_units_consumed;
   let total = full.compute_units_consumed;
   let delta = total.saturating_sub(shell);
   append_cu_report_line(&format!("{label_base}:bench_total"), total);
   append_cu_report_line(&format!("{label_base}:cu_vault_invoke_shell"), shell);
   append_cu_report_line(&format!("{label_base}:cu_inner_app_delta"), delta);
   if vault_log_cu_enabled() {
      eprintln!(
         "vault CU [bench {label_base}] app_ix split: total={total} vault_invoke_shell={shell} inner_app_delta={delta}"
      );
   }
}

/// CPI through `test_program`: `caller_noop` = top-level [`ix_test_bench_noop_top`] only; `full` = wrapper + vault + CPIs.
/// Writes:
/// - `{label_base}:bench_total`
/// - `{label_base}:cu_caller_shell_min` — minimal `test_program` top-level frame
/// - `{label_base}:cu_below_caller_shell` — `full − shell` (vault subtree, SPL/System CPIs, and extra wrapper work)
pub fn log_cu_bench_cpi_via_caller_split(
   label_base: &str,
   caller_noop: &InstructionResult,
   full: &InstructionResult,
) {
   let shell = caller_noop.compute_units_consumed;
   let total = full.compute_units_consumed;
   let delta = total.saturating_sub(shell);
   append_cu_report_line(&format!("{label_base}:bench_total"), total);
   append_cu_report_line(&format!("{label_base}:cu_caller_shell_min"), shell);
   append_cu_report_line(&format!("{label_base}:cu_below_caller_shell"), delta);
   if vault_log_cu_enabled() {
      eprintln!(
         "vault CU [bench {label_base}] cpi split: total={total} caller_shell_min={shell} below_caller_shell={delta}"
      );
   }
}
