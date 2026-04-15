// Security tests: attacker (non-owner, non-delegate) cannot drain lamports/SPL or change delegate.
// Each case starts from a legitimate vault + deposit, then models deployment/spoofing and a hostile tx.

use crate::common::{
   associated_token_address, clock_sysvar_pk, custom_vault_err, decode_user_vault, derive_user_vault, fresh_mollusk,
   ix_app_ix, ix_close_vault_ata, ix_create, ix_cpi_entry, ix_cpi_entry_native, ix_deposit, ix_test_deposit_via_cpi,
   ix_update_delegate, ix_withdraw, merge_accounts, mint_account, overlay_accounts, rent_sysvar_account,
   rent_sysvar_pk, set_clock_unix, signer_account, system_program_meta, test_program_id, token_account, token_program_id,
   vault_program_id, with_clock_and_loaders, with_ix_sysvar_clock_and_loaders, with_loader_accounts,
};
use app_specific_delegated_vaults::constants::USER_VAULT_DISCRIMINATOR;
use app_specific_delegated_vaults::error::Error;
use app_specific_delegated_vaults::state::UserVaultAccount;
use mollusk_svm::result::Check;
use mollusk_svm_programs_token::{associated_token, token};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use spl_token_interface::state::Account as SplTokenAccount;
use test_program::TestProgramInstruction;

const LEGIT_EXPIRY: u32 = u32::MAX;
const DEPOSIT_AMOUNT: u64 = 1_000_000;
const NATIVE_HEADROOM: u64 = 5_000_000_000;

/// Pack vault metadata bytes (same layout as on-chain `UserVaultAccount`).
fn pack_vault_account_bytes(
   discriminator: u8,
   bump: u8,
   ata_count: u16,
   delegate_expires: u32,
   owner: &Pubkey,
   app_address: &Pubkey,
   delegate: &Pubkey,
) -> Vec<u8> {
   let mut v = vec![0u8; UserVaultAccount::LEN];
   v[0] = discriminator;
   v[1] = bump;
   v[2..4].copy_from_slice(&ata_count.to_le_bytes());
   v[4..8].copy_from_slice(&delegate_expires.to_le_bytes());
   v[8..40].copy_from_slice(owner.as_ref());
   v[40..72].copy_from_slice(app_address.as_ref());
   v[72..104].copy_from_slice(delegate.as_ref());
   v
}

/// Program-owned fake vault (harness-only; impossible on-chain without PDA create).
fn fake_vault_account(data: Vec<u8>, lamports: u64) -> Account {
   Account {
      lamports,
      data,
      owner: vault_program_id(),
      executable: false,
      rent_epoch: 0,
   }
}

struct LegitVaultFixture {
   mollusk: mollusk_svm::Mollusk,
   after_deposit: mollusk_svm::result::InstructionResult,
   owner: Pubkey,
   delegate: Pubkey,
   app: Pubkey,
   pda: Pubkey,
   pda_bump: u8,
   vault_ata: Pubkey,
   mint_pk: Pubkey,
   tok: Pubkey,
   sys_pk: Pubkey,
   sys_acct: Account,
}

fn legit_vault_fixture() -> LegitVaultFixture {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let delegate = Pubkey::new_unique();
   let app = test_program_id();
   let (pda, pda_bump) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let mint_acct = mint_account(owner, 0);
   let source = associated_token_address(&owner, &mint_pk);
   let source_acct = token_account(&mint_pk, &owner, 10_000_000);
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let (sys_pk, sys_acct) = system_program_meta();
   let (rent_pk, rent_acct) = rent_sysvar_account(&mollusk);
   let tok = token_program_id();
   let ata = associated_token::ID;

   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (rent_pk, rent_acct),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(LEGIT_EXPIRY),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(rent_sysvar_pk(), false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, source_acct),
      (mint_pk, mint_acct),
      (sys_pk, sys_acct.clone()),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged_d = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(DEPOSIT_AMOUNT),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let after_deposit = mollusk.process_and_validate_instruction(&dep_ix, &merged_d, &[Check::success()]);

   LegitVaultFixture {
      mollusk,
      after_deposit,
      owner,
      delegate,
      app,
      pda,
      pda_bump,
      vault_ata,
      mint_pk,
      tok,
      sys_pk,
      sys_acct,
   }
}

/// Merge `user` with post-deposit state and ensure the vault PDA holds extra native lamports (fixture “SOL on vault”).
fn merge_fixture(f: &LegitVaultFixture, user: &[(Pubkey, Account)]) -> Vec<(Pubkey, Account)> {
   let rent = f.mollusk.sysvars.rent.minimum_balance(UserVaultAccount::LEN);
   let mut m = merge_accounts(user, &f.after_deposit);
   for (k, a) in m.iter_mut() {
      if k == &f.pda {
         a.lamports = rent + NATIVE_HEADROOM;
      }
   }
   m
}

// --- Category A: spoofed / malformed vault accounts ---

#[test]
fn spoofed_vault_app_ix_harness_metadata_does_not_mutate_real_vault() {
   // Harness can inject a program-owned "vault" blob at a non-PDA address. With zero inner accounts,
   // Mollusk may still let the outer `app_ix` succeed (no inner account binds PDA seeds to an address).
   // Security bar: the *real* vault PDA and vault ATA must be unchanged.
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let fake_pk = Pubkey::new_unique();
   let data = pack_vault_account_bytes(
      USER_VAULT_DISCRIMINATOR,
      f.pda_bump,
      1,
      LEGIT_EXPIRY,
      &f.owner,
      &f.app,
      &attacker,
   );
   let fake_acct = fake_vault_account(data.clone(), 1_000_000);

   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(attacker, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(fake_pk, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );

   let user = vec![
      (attacker, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (fake_pk, fake_acct),
      (f.app, Account::default()),
      (f.pda, Account::default()),
      (f.vault_ata, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let victim_vault_before = decode_user_vault(&merged.iter().find(|(k, _)| k == &f.pda).unwrap().1.data).unwrap();
   let spl_before =
      SplTokenAccount::unpack(&merged.iter().find(|(k, _)| k == &f.vault_ata).unwrap().1.data)
         .unwrap()
         .amount;
   let fake_before = merged.iter().find(|(k, _)| k == &fake_pk).unwrap().1.clone();

   let accounts = with_clock_and_loaders(&f.mollusk, &merged);
   let r = f
      .mollusk
      .process_and_validate_instruction(&vault_ix, &accounts, &[Check::success()]);

   let victim_vault_after = decode_user_vault(
      r.get_account(&f.pda)
         .map(|a| a.data.as_slice())
         .unwrap_or_else(|| merged.iter().find(|(k, _)| k == &f.pda).unwrap().1.data.as_slice()),
   )
   .unwrap();
   assert_eq!(victim_vault_after, victim_vault_before);
   let spl_after = SplTokenAccount::unpack(
      r.get_account(&f.vault_ata)
         .map(|a| a.data.as_slice())
         .unwrap_or_else(|| merged.iter().find(|(k, _)| k == &f.vault_ata).unwrap().1.data.as_slice()),
   )
   .unwrap()
   .amount;
   assert_eq!(spl_after, spl_before);
   let fake_after = r.get_account(&fake_pk).cloned().unwrap_or(fake_before.clone());
   assert_eq!(fake_after.lamports, fake_before.lamports);
   assert_eq!(fake_after.data, fake_before.data);
}

#[test]
fn spoofed_vault_cpi_entry_native_blocked_without_app_top_level() {
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let fake_pk = Pubkey::new_unique();
   let rent = f.mollusk.sysvars.rent.minimum_balance(UserVaultAccount::LEN);
   let data = pack_vault_account_bytes(
      USER_VAULT_DISCRIMINATOR,
      255,
      0,
      LEGIT_EXPIRY,
      &f.owner,
      &f.app,
      &attacker,
   );
   let fake_acct = fake_vault_account(data, rent + NATIVE_HEADROOM);
   let lamports_dest = Pubkey::new_unique();

   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_cpi_entry_native(1_000_000),
      vec![
         AccountMeta::new(attacker, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new(fake_pk, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (attacker, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (fake_pk, fake_acct),
      (f.app, Account::default()),
      (lamports_dest, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let accounts = with_ix_sysvar_clock_and_loaders(&ix, &f.mollusk, &merged);
   let fake_lamports_before = accounts.iter().find(|(k, _)| k == &fake_pk).unwrap().1.lamports;
   let dest_lamports_before = accounts.iter().find(|(k, _)| k == &lamports_dest).unwrap().1.lamports;
   let r = f.mollusk.process_and_validate_instruction(
      &ix,
      &accounts,
      &[Check::err(custom_vault_err(Error::UnauthorizedCpiCaller))],
   );
   let fake_after = r.get_account(&fake_pk).unwrap().lamports;
   let dest_after = r.get_account(&lamports_dest).unwrap().lamports;
   assert_eq!(fake_after, fake_lamports_before);
   assert_eq!(dest_after, dest_lamports_before);
}

#[test]
fn spoofed_vault_wrong_discriminator_app_ix_harness_no_victim_drift() {
   // Same as `spoofed_vault_app_ix_harness_metadata_does_not_mutate_real_vault` but metadata uses
   // discriminator `0` (never written by `CreateUserVault`). On-chain only the program can set
   // bytes; here we only assert the real vault + ATA are untouched after `app_ix`.
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let fake_pk = Pubkey::new_unique();
   let data = pack_vault_account_bytes(
      0u8,
      f.pda_bump,
      1,
      LEGIT_EXPIRY,
      &f.owner,
      &f.app,
      &attacker,
   );
   let fake_acct = fake_vault_account(data.clone(), 1_000_000);
   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(attacker, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(fake_pk, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (attacker, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (fake_pk, fake_acct),
      (f.app, Account::default()),
      (f.pda, Account::default()),
      (f.vault_ata, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let victim_vault_before = decode_user_vault(&merged.iter().find(|(k, _)| k == &f.pda).unwrap().1.data).unwrap();
   let spl_before =
      SplTokenAccount::unpack(&merged.iter().find(|(k, _)| k == &f.vault_ata).unwrap().1.data)
         .unwrap()
         .amount;
   let fake_before = merged.iter().find(|(k, _)| k == &fake_pk).unwrap().1.clone();

   let accounts = with_clock_and_loaders(&f.mollusk, &merged);
   let r = f
      .mollusk
      .process_and_validate_instruction(&vault_ix, &accounts, &[Check::success()]);

   let victim_vault_after = decode_user_vault(
      r.get_account(&f.pda)
         .map(|a| a.data.as_slice())
         .unwrap_or_else(|| merged.iter().find(|(k, _)| k == &f.pda).unwrap().1.data.as_slice()),
   )
   .unwrap();
   assert_eq!(victim_vault_after, victim_vault_before);
   let spl_after = SplTokenAccount::unpack(
      r.get_account(&f.vault_ata)
         .map(|a| a.data.as_slice())
         .unwrap_or_else(|| merged.iter().find(|(k, _)| k == &f.vault_ata).unwrap().1.data.as_slice()),
   )
   .unwrap()
   .amount;
   assert_eq!(spl_after, spl_before);
   let fake_after = r.get_account(&fake_pk).cloned().unwrap_or(fake_before.clone());
   assert_eq!(fake_after.lamports, fake_before.lamports);
   assert_eq!(fake_after.data, fake_before.data);
}

#[test]
fn spoofed_vault_wrong_data_length_rejected() {
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let fake_pk = Pubkey::new_unique();
   let mut short = pack_vault_account_bytes(
      USER_VAULT_DISCRIMINATOR,
      1,
      0,
      LEGIT_EXPIRY,
      &f.owner,
      &f.app,
      &attacker,
   );
   short.truncate(UserVaultAccount::LEN - 1);
   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(attacker, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(fake_pk, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (attacker, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (fake_pk, fake_vault_account(short, 1_000_000)),
      (f.app, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let accounts = with_clock_and_loaders(&f.mollusk, &merged);
   let _ = f.mollusk.process_and_validate_instruction(
      &vault_ix,
      &accounts,
      &[Check::err(custom_vault_err(Error::InvalidUserVaultAccountLength))],
   );
}

#[test]
fn spoofed_vault_wrong_program_owner_rejected() {
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let fake_pk = Pubkey::new_unique();
   let data = pack_vault_account_bytes(
      USER_VAULT_DISCRIMINATOR,
      1,
      0,
      LEGIT_EXPIRY,
      &f.owner,
      &f.app,
      &attacker,
   );
   let system_owned = Account {
      lamports: 1_000_000,
      data,
      owner: solana_sdk_ids::system_program::ID,
      executable: false,
      rent_epoch: 0,
   };
   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(attacker, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(fake_pk, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (attacker, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (fake_pk, system_owned),
      (f.app, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let accounts = with_clock_and_loaders(&f.mollusk, &merged);
   let _ = f.mollusk.process_and_validate_instruction(
      &vault_ix,
      &accounts,
      &[Check::err(custom_vault_err(Error::UserVaultNotFound))],
   );
}

// --- Category B: sysvar spoofing ---

#[test]
fn fake_clock_sysvar_rejected() {
   let f = legit_vault_fixture();
   let fake_clock = Pubkey::new_unique();
   let mut clock_like = vec![0u8; 40];
   clock_like[32..40].copy_from_slice(&1i64.to_le_bytes());
   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(f.delegate, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new_readonly(fake_clock, false),
      ],
   );
   let user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.app, Account::default()),
      (fake_clock, Account {
         lamports: 1,
         data: clock_like,
         owner: solana_sdk_ids::sysvar::ID,
         executable: false,
         rent_epoch: 0,
      }),
   ];
   let merged = merge_fixture(&f, &user);
   let _ = f.mollusk.process_and_validate_instruction(
      &vault_ix,
      &with_loader_accounts(&merged),
      &[Check::err(custom_vault_err(Error::InvalidClockAccount))],
   );
}

#[test]
fn fake_instructions_sysvar_rejected() {
   let f = legit_vault_fixture();
   let fake_ix = Pubkey::new_unique();
   let lamports_dest = Pubkey::new_unique();
   let dest_wallet = Pubkey::new_unique();
   let dest_ata = associated_token_address(&dest_wallet, &f.mint_pk);
   let stub = Instruction::new_with_bytes(f.app, &[], vec![]);
   let (_real_ix_pk, real_ix_acct) = crate::common::instructions_sysvar_for(&stub);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_cpi_entry(0, 10_000),
      vec![
         AccountMeta::new(f.delegate, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new(f.vault_ata, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(f.tok, false),
         AccountMeta::new_readonly(fake_ix, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.vault_ata, Account::default()),
      (f.app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&f.mint_pk, &dest_wallet, 0)),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.tok, token::keyed_account().1),
      (fake_ix, real_ix_acct.clone()),
   ];
   let merged = merge_fixture(&f, &user);
   let mut layer = with_ix_sysvar_clock_and_loaders(&ix, &f.mollusk, &merged);
   for (k, a) in layer.iter_mut() {
      if k == &fake_ix {
         *a = real_ix_acct.clone();
      }
   }
   let _ = f.mollusk.process_and_validate_instruction(
      &ix,
      &layer,
      &[Check::err(ProgramError::UnsupportedSysvar)],
   );
}

// --- Category C: cross-vault / substitution ---

#[test]
fn attackers_vault_cannot_access_victims_ata() {
   let f = legit_vault_fixture();
   let attacker_owner = Pubkey::new_unique();
   let attacker_delegate = Pubkey::new_unique();
   let app = f.app;
   let (attacker_pda, _) = derive_user_vault(&attacker_owner, &app);
   let attacker_source = associated_token_address(&attacker_owner, &f.mint_pk);
   let attacker_source_acct = token_account(&f.mint_pk, &attacker_owner, 5_000_000);
   let attacker_vault_ata = associated_token_address(&attacker_pda, &f.mint_pk);
   let (rent_pk, rent_acct) = rent_sysvar_account(&f.mollusk);
   let tok = f.tok;
   let ata = associated_token::ID;

   let create_a = vec![
      (attacker_owner, signer_account(2_000_000_000)),
      (attacker_pda, Account::default()),
      (app, Account::default()),
      (attacker_delegate, Account::default()),
      (rent_pk, rent_acct),
      (f.sys_pk, f.sys_acct.clone()),
   ];
   let create_ix_a = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(LEGIT_EXPIRY),
      vec![
         AccountMeta::new(attacker_owner, true),
         AccountMeta::new(attacker_pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(attacker_delegate, false),
         AccountMeta::new_readonly(rent_sysvar_pk(), false),
         AccountMeta::new_readonly(f.sys_pk, false),
      ],
   );
   let r_a0 = f
      .mollusk
      .process_and_validate_instruction(&create_ix_a, &create_a, &[Check::success()]);

   let dep_a = vec![
      (attacker_owner, signer_account(2_000_000_000)),
      (attacker_pda, Account::default()),
      (attacker_vault_ata, Account::default()),
      (app, Account::default()),
      (attacker_source, attacker_source_acct),
      (f.mint_pk, mint_account(attacker_owner, 0)),
      (f.sys_pk, f.sys_acct.clone()),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged_a = merge_accounts(&dep_a, &r_a0);
   let dep_ix_a = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(100_000),
      vec![
         AccountMeta::new(attacker_owner, true),
         AccountMeta::new(attacker_pda, false),
         AccountMeta::new(attacker_vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(attacker_source, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(f.sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let r_a1 = f
      .mollusk
      .process_and_validate_instruction(&dep_ix_a, &merged_a, &[Check::success()]);

   let lamports_dest = Pubkey::new_unique();
   let dest_owner = Pubkey::new_unique();
   let dest_ata = associated_token_address(&dest_owner, &f.mint_pk);
   let cpi_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_deposit_via_cpi(5_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(attacker_delegate, true),
         AccountMeta::new_readonly(attacker_owner, false),
         AccountMeta::new_readonly(attacker_pda, false),
         AccountMeta::new(f.vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (attacker_delegate, signer_account(1_000_000_000)),
      (attacker_owner, Account::default()),
      (attacker_pda, Account::default()),
      (f.vault_ata, Account::default()),
      (f.pda, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&f.mint_pk, &dest_owner, 0)),
      (f.mint_pk, mint_account(f.owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let mut merged_cpi = merge_accounts(&user, &r_a1);
   for (k, a) in merged_cpi.iter_mut() {
      if let Some(acc) = f.after_deposit.get_account(k) {
         *a = acc.clone();
      }
   }
   let merged_cpi = with_ix_sysvar_clock_and_loaders(&cpi_ix, &f.mollusk, &merged_cpi);
   let victim_vault_before = decode_user_vault(
      &merged_cpi
         .iter()
         .find(|(k, _)| k == &f.pda)
         .unwrap()
         .1
         .data,
   )
   .unwrap();
   let victim_spl_before =
      SplTokenAccount::unpack(&merged_cpi.iter().find(|(k, _)| k == &f.vault_ata).unwrap().1.data)
         .unwrap()
         .amount;
   // SPL `TransferChecked` fails: vault PDA signs for attacker vault, not victim ATA authority.
   let r = f.mollusk.process_and_validate_instruction(
      &cpi_ix,
      &merged_cpi,
      &[Check::err(ProgramError::Custom(4))],
   );
   let victim_vault_after = decode_user_vault(
      r.get_account(&f.pda)
         .as_ref()
         .map(|a| a.data.as_slice())
         .unwrap_or_else(|| {
            merged_cpi
               .iter()
               .find(|(k, _)| k == &f.pda)
               .map(|(_, a)| a.data.as_slice())
               .unwrap()
         }),
   )
   .unwrap();
   let victim_spl_after = r
      .get_account(&f.vault_ata)
      .and_then(|a| SplTokenAccount::unpack(&a.data).ok())
      .map(|t| t.amount)
      .unwrap_or(victim_spl_before);
   assert_eq!(victim_vault_after.ata_count, victim_vault_before.ata_count);
   assert_eq!(victim_spl_after, victim_spl_before);
}

#[test]
fn wrong_owner_account_with_victims_vault() {
   let f = legit_vault_fixture();
   let wrong_owner = Pubkey::new_unique();
   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(f.delegate, true),
         AccountMeta::new_readonly(wrong_owner, false),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (wrong_owner, Account::default()),
      (f.pda, Account::default()),
      (f.app, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let _ = f.mollusk.process_and_validate_instruction(
      &vault_ix,
      &with_clock_and_loaders(&f.mollusk, &merged),
      &[Check::err(custom_vault_err(Error::UserVaultOwnerMismatch))],
   );
}

#[test]
fn wrong_app_address_with_victims_vault() {
   let f = legit_vault_fixture();
   let wrong_app = Pubkey::new_unique();
   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(f.delegate, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new_readonly(wrong_app, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (wrong_app, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let _ = f.mollusk.process_and_validate_instruction(
      &vault_ix,
      &with_clock_and_loaders(&f.mollusk, &merged),
      &[Check::err(custom_vault_err(Error::UserVaultAppAddressMismatch))],
   );
}

// --- Category D: direct vault CPI (no app top-level) ---

#[test]
fn cpi_entry_direct_invocation_rejected() {
   let f = legit_vault_fixture();
   let lamports_dest = Pubkey::new_unique();
   let dest_owner = Pubkey::new_unique();
   let dest_ata = associated_token_address(&dest_owner, &f.mint_pk);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_cpi_entry(0, 10_000),
      vec![
         AccountMeta::new(f.delegate, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new(f.vault_ata, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(f.tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.vault_ata, Account::default()),
      (f.app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&f.mint_pk, &dest_owner, 0)),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.tok, token::keyed_account().1),
   ];
   let merged = with_ix_sysvar_clock_and_loaders(&ix, &f.mollusk, &merge_fixture(&f, &user));
   let _ = f.mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(custom_vault_err(Error::UnauthorizedCpiCaller))],
   );
}

#[test]
fn cpi_entry_native_direct_invocation_rejected() {
   let f = legit_vault_fixture();
   let lamports_dest = Pubkey::new_unique();
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_cpi_entry_native(500_000),
      vec![
         AccountMeta::new(f.delegate, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.app, Account::default()),
      (lamports_dest, Account::default()),
   ];
   let merged = with_ix_sysvar_clock_and_loaders(&ix, &f.mollusk, &merge_fixture(&f, &user));
   let pda_before = merged.iter().find(|(k, _)| k == &f.pda).unwrap().1.lamports;
   let r = f.mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(custom_vault_err(Error::UnauthorizedCpiCaller))],
   );
   assert_eq!(r.get_account(&f.pda).unwrap().lamports, pda_before);
}

// --- Category E: delegate impersonation ---

#[test]
fn attacker_signs_app_ix_as_fake_delegate() {
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(attacker, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (attacker, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.app, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let _ = f.mollusk.process_and_validate_instruction(
      &vault_ix,
      &with_clock_and_loaders(&f.mollusk, &merged),
      &[Check::err(custom_vault_err(Error::InvalidUserVaultDelegate))],
   );
}

#[test]
fn attacker_signs_cpi_entry_as_fake_delegate() {
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let lamports_dest = Pubkey::new_unique();
   let dest_owner = Pubkey::new_unique();
   let dest_ata = associated_token_address(&dest_owner, &f.mint_pk);
   let cpi_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_deposit_via_cpi(1_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(attacker, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new(f.vault_ata, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(f.tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (attacker, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.vault_ata, Account::default()),
      (f.app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&f.mint_pk, &dest_owner, 0)),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.tok, token::keyed_account().1),
   ];
   let merged = with_ix_sysvar_clock_and_loaders(&cpi_ix, &f.mollusk, &merge_fixture(&f, &user));
   let _ = f.mollusk.process_and_validate_instruction(
      &cpi_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::InvalidUserVaultDelegate))],
   );
}

// --- Category F: unauthorized delegate update / create ---

#[test]
fn attacker_cannot_update_delegate_on_victims_vault() {
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let new_delegate = Pubkey::new_unique();
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_update_delegate(LEGIT_EXPIRY),
      vec![
         AccountMeta::new(f.owner, false),
         AccountMeta::new(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new_readonly(new_delegate, false),
      ],
   );
   let user = vec![
      (f.owner, signer_account(2_000_000_000)),
      (attacker, signer_account(1_000_000_000)),
      (f.pda, Account::default()),
      (f.app, Account::default()),
      (new_delegate, Account::default()),
   ];
   let merged = merge_fixture(&f, &user);
   let before = decode_user_vault(&merged.iter().find(|(k, _)| k == &f.pda).unwrap().1.data).unwrap();
   let r = f.mollusk.process_and_validate_instruction(
      &ix,
      &with_loader_accounts(&merged),
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
   let after = decode_user_vault(&r.get_account(&f.pda).unwrap().data).unwrap();
   assert_eq!(after.delegate, before.delegate);
}

#[test]
fn attacker_cannot_create_vault_for_victim() {
   let mollusk = fresh_mollusk();
   let victim = Pubkey::new_unique();
   let attacker = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&victim, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let (rent_pk, rent_acct) = rent_sysvar_account(&mollusk);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(LEGIT_EXPIRY),
      vec![
         AccountMeta::new(victim, false),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(rent_sysvar_pk(), false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let accounts = vec![
      (victim, Account::default()),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (rent_pk, rent_acct),
      (sys_pk, sys_acct),
      (attacker, signer_account(2_000_000_000)),
   ];
   let _ = mollusk.process_and_validate_instruction(
      &ix,
      &with_loader_accounts(&accounts),
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
}

// PDA slot squatting: address is correct for (owner, app) but not an empty system account.

#[test]
fn create_rejected_when_vault_pda_presquatted_with_lamports() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let (rent_pk, rent_acct) = rent_sysvar_account(&mollusk);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(LEGIT_EXPIRY),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(rent_sysvar_pk(), false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let presquatted = Account {
      lamports: 890_880,
      data: vec![],
      owner: solana_sdk_ids::system_program::ID,
      executable: false,
      rent_epoch: 0,
   };
   let accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, presquatted),
      (app, Account::default()),
      (delegate, Account::default()),
      (rent_pk, rent_acct),
      (sys_pk, sys_acct),
   ];
   let _ = mollusk.process_and_validate_instruction(
      &ix,
      &with_loader_accounts(&accounts),
      &[Check::err(custom_vault_err(Error::UserVaultAlreadyExists))],
   );
}

#[test]
fn create_rejected_when_vault_pda_presquatted_wrong_owner_empty_data() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let (rent_pk, rent_acct) = rent_sysvar_account(&mollusk);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(LEGIT_EXPIRY),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(rent_sysvar_pk(), false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let presquatted = Account {
      lamports: 0,
      data: vec![],
      owner: vault_program_id(),
      executable: false,
      rent_epoch: 0,
   };
   let accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, presquatted),
      (app, Account::default()),
      (delegate, Account::default()),
      (rent_pk, rent_acct),
      (sys_pk, sys_acct),
   ];
   let _ = mollusk.process_and_validate_instruction(
      &ix,
      &with_loader_accounts(&accounts),
      &[Check::err(custom_vault_err(Error::UserVaultAlreadyExists))],
   );
}

// --- Category G + H: token / close / expiry ---

#[test]
fn withdraw_to_attackers_ata_rejected() {
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let dest = associated_token_address(&attacker, &f.mint_pk);
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw(10_000),
      vec![
         AccountMeta::new(f.owner, true),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new(f.vault_ata, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(dest, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(f.tok, false),
      ],
   );
   let user = vec![
      (f.owner, signer_account(2_000_000_000)),
      (f.pda, Account::default()),
      (f.vault_ata, Account::default()),
      (f.app, Account::default()),
      (dest, token_account(&f.mint_pk, &attacker, 0)),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.tok, token::keyed_account().1),
   ];
   let merged = merge_fixture(&f, &user);
   let _ = f.mollusk.process_and_validate_instruction(
      &w_ix,
      &with_loader_accounts(&merged),
      &[Check::err(custom_vault_err(Error::InvalidAta))],
   );
}

#[test]
fn close_vault_ata_by_non_owner_rejected() {
   let f = legit_vault_fixture();
   let attacker = Pubkey::new_unique();
   let dest = Pubkey::new_unique();
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_close_vault_ata(),
      vec![
         AccountMeta::new(f.owner, false),
         AccountMeta::new(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(f.vault_ata, false),
         AccountMeta::new(dest, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(f.tok, false),
      ],
   );
   let user = vec![
      (f.owner, signer_account(2_000_000_000)),
      (attacker, signer_account(1_000_000_000)),
      (f.pda, Account::default()),
      (f.app, Account::default()),
      (f.vault_ata, Account::default()),
      (dest, Account::default()),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.tok, token::keyed_account().1),
   ];
   let merged = merge_fixture(&f, &user);
   let _ = f.mollusk.process_and_validate_instruction(
      &ix,
      &with_loader_accounts(&merged),
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
}

#[test]
fn expired_delegate_app_ix_rejected() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let expiry: u32 = 1_800_000_000;
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let (rent_pk, rent_acct) = rent_sysvar_account(&mollusk);
   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (rent_pk, rent_acct),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(expiry),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(rent_sysvar_pk(), false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);

   let mut m2 = mollusk;
   set_clock_unix(&mut m2, expiry as i64 + 1);
   let inner = [TestProgramInstruction::BenchNoopInner as u8];
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner),
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (app, Account::default()),
   ];
   let merged = merge_accounts(&user, &r0);
   let (pk, clock_acct) = m2.sysvars.keyed_account_for_clock_sysvar();
   let accounts = with_loader_accounts(&overlay_accounts(&[merged.as_slice(), &[(pk, clock_acct)]]));
   let _ = m2.process_and_validate_instruction(
      &vault_ix,
      &accounts,
      &[Check::err(custom_vault_err(Error::ExpiredDelegate))],
   );
}
