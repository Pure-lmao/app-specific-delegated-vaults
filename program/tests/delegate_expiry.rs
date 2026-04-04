use crate::common::{
   associated_token_address, custom_vault_err, derive_user_vault, fresh_mollusk, ix_app_ix, ix_create, ix_deposit,
   ix_test_cpi_entry_native_via_vault, ix_test_deposit_via_cpi, log_cu, merge_accounts, mint_account, set_clock_unix,
   signer_account, system_program_meta, test_program_id, token_account, token_program_id, treasury_pda,
   vault_program_id, with_ix_sysvar_and_loaders, with_loader_accounts,
};
use mollusk_svm::result::Check;
use mollusk_svm_programs_token::{associated_token, token};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use app_specific_delegated_vaults::error::Error;

const T_EXPIRY: u32 = 1_800_000_000;

fn inner_deposit_from_user(amount: u64) -> Vec<u8> {
   let mut v = vec![2u8];
   v.extend_from_slice(&amount.to_le_bytes());
   v
}

/// Shared create + deposit into vault ATA (non-zero delegate expiry) for delegate-clock scenarios.
struct VaultAppTokensFixture {
   mollusk: mollusk_svm::Mollusk,
   after_deposit: mollusk_svm::result::InstructionResult,
   owner: Pubkey,
   app: Pubkey,
   delegate: Pubkey,
   pda: Pubkey,
   vault_ata: Pubkey,
   mint_pk: Pubkey,
   tok: Pubkey,
   sys_pk: Pubkey,
   sys_acct: Account,
}

fn vault_with_vault_tokens_and_app_ix_setup() -> VaultAppTokensFixture {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let mint_acct = mint_account(owner, 0);
   let owner_source = associated_token_address(&owner, &mint_pk);
   let owner_source_acct = token_account(&mint_pk, &owner, 2_000_000);
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let (sys_pk, sys_acct) = system_program_meta();
   let tok = token_program_id();
   let ata = associated_token::ID;

   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(T_EXPIRY),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (owner_source, owner_source_acct),
      (mint_pk, mint_acct),
      (sys_pk, sys_acct.clone()),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged_d = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(500_000),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(owner_source, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let after_deposit = mollusk.process_and_validate_instruction(&dep_ix, &merged_d, &[Check::success()]);
   VaultAppTokensFixture {
      mollusk,
      after_deposit,
      owner,
      app,
      delegate,
      pda,
      vault_ata,
      mint_pk,
      tok,
      sys_pk,
      sys_acct,
   }
}

#[test]
fn app_ix_ok_before_expiry() {
   let mut f = vault_with_vault_tokens_and_app_ix_setup();
   set_clock_unix(&mut f.mollusk, T_EXPIRY as i64);
   let (treasury_pda, _) = treasury_pda();
   let treasury_ata = associated_token_address(&treasury_pda, &f.mint_pk);
   let owner_source = associated_token_address(&f.owner, &f.mint_pk);
   let app_ix_data = ix_app_ix(&inner_deposit_from_user(10_000));
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &app_ix_data,
      vec![
         AccountMeta::new(f.delegate, true),
         AccountMeta::new(f.owner, true),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(f.delegate, true),
         AccountMeta::new(f.owner, true),
         AccountMeta::new(owner_source, false),
         AccountMeta::new_readonly(treasury_pda, false),
         AccountMeta::new(treasury_ata, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(f.sys_pk, false),
         AccountMeta::new_readonly(f.tok, false),
         AccountMeta::new_readonly(associated_token::ID, false),
      ],
   );
   let app_accounts = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, signer_account(1_000_000_000)),
      (f.pda, Account::default()),
      (f.app, Account::default()),
      (owner_source, Account::default()),
      (treasury_pda, Account::default()),
      (treasury_ata, Account::default()),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.sys_pk, f.sys_acct),
      (f.tok, token::keyed_account().1),
      (associated_token::ID, associated_token::keyed_account().1),
   ];
   let merged_app = merge_accounts(&app_accounts, &f.after_deposit);
   let r_app = f.mollusk.process_and_validate_instruction(
      &vault_ix,
      &with_loader_accounts(&merged_app),
      &[Check::success()],
   );
   log_cu("delegate_expiry::app_ix_ok_before_expiry", &r_app);
}

#[test]
fn app_ix_fails_after_expiry() {
   let mut f = vault_with_vault_tokens_and_app_ix_setup();
   set_clock_unix(&mut f.mollusk, T_EXPIRY as i64 + 1);
   let (treasury_pda, _) = treasury_pda();
   let treasury_ata = associated_token_address(&treasury_pda, &f.mint_pk);
   let owner_source = associated_token_address(&f.owner, &f.mint_pk);
   let app_ix_data = ix_app_ix(&inner_deposit_from_user(10_000));
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &app_ix_data,
      vec![
         AccountMeta::new(f.delegate, true),
         AccountMeta::new(f.owner, true),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(f.delegate, true),
         AccountMeta::new(f.owner, true),
         AccountMeta::new(owner_source, false),
         AccountMeta::new_readonly(treasury_pda, false),
         AccountMeta::new(treasury_ata, false),
         AccountMeta::new_readonly(f.mint_pk, false),
         AccountMeta::new_readonly(f.sys_pk, false),
         AccountMeta::new_readonly(f.tok, false),
         AccountMeta::new_readonly(associated_token::ID, false),
      ],
   );
   let app_accounts = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, signer_account(1_000_000_000)),
      (f.pda, Account::default()),
      (f.app, Account::default()),
      (owner_source, Account::default()),
      (treasury_pda, Account::default()),
      (treasury_ata, Account::default()),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.sys_pk, f.sys_acct),
      (f.tok, token::keyed_account().1),
      (associated_token::ID, associated_token::keyed_account().1),
   ];
   let merged_app = merge_accounts(&app_accounts, &f.after_deposit);
   let r = f.mollusk.process_and_validate_instruction(
      &vault_ix,
      &with_loader_accounts(&merged_app),
      &[Check::err(custom_vault_err(Error::ExpiredDelegate))],
   );
   log_cu("delegate_expiry::app_ix_fails_after_expiry", &r);
}

#[test]
fn cpi_entry_fails_after_expiry() {
   let mut f = vault_with_vault_tokens_and_app_ix_setup();
   set_clock_unix(&mut f.mollusk, T_EXPIRY as i64 + 1);
   let lamports_dest = Pubkey::new_unique();
   let dest_owner = Pubkey::new_unique();
   let dest_ata = associated_token_address(&dest_owner, &f.mint_pk);
   let cpi_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_deposit_via_cpi(5_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
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
         AccountMeta::new_readonly(f.sys_pk, false),
      ],
   );
   let cpi_user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.vault_ata, Account::default()),
      (f.app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&f.mint_pk, &dest_owner, 0)),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.tok, token::keyed_account().1),
      (f.sys_pk, system_program_meta().1),
   ];
   let cpi_merged = merge_accounts(&cpi_user, &f.after_deposit);
   let cpi_accounts = with_ix_sysvar_and_loaders(&cpi_ix, &cpi_merged);
   let r = f.mollusk.process_and_validate_instruction(
      &cpi_ix,
      &cpi_accounts,
      &[Check::err(custom_vault_err(Error::ExpiredDelegate))],
   );
   log_cu("delegate_expiry::cpi_entry_fails_after_expiry", &r);
}

#[test]
fn cpi_entry_ok_before_expiry() {
   let mut f = vault_with_vault_tokens_and_app_ix_setup();
   set_clock_unix(&mut f.mollusk, T_EXPIRY as i64);
   let lamports_dest = Pubkey::new_unique();
   let dest_owner = Pubkey::new_unique();
   let dest_ata = associated_token_address(&dest_owner, &f.mint_pk);
   let cpi_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_deposit_via_cpi(5_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
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
         AccountMeta::new_readonly(f.sys_pk, false),
      ],
   );
   let cpi_user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.vault_ata, Account::default()),
      (f.app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&f.mint_pk, &dest_owner, 0)),
      (f.mint_pk, mint_account(f.owner, 0)),
      (f.tok, token::keyed_account().1),
      (f.sys_pk, system_program_meta().1),
   ];
   let cpi_merged = merge_accounts(&cpi_user, &f.after_deposit);
   let cpi_accounts = with_ix_sysvar_and_loaders(&cpi_ix, &cpi_merged);
   let r_cpi = f
      .mollusk
      .process_and_validate_instruction(&cpi_ix, &cpi_accounts, &[Check::success()]);
   log_cu("delegate_expiry::cpi_entry_ok_before_expiry", &r_cpi);
}

#[test]
fn cpi_entry_native_fails_after_expiry() {
   let mut f = vault_with_vault_tokens_and_app_ix_setup();
   set_clock_unix(&mut f.mollusk, T_EXPIRY as i64 + 1);
   let lamports_dest = Pubkey::new_unique();
   let native_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_cpi_entry_native_via_vault(10_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(f.delegate, true),
         AccountMeta::new_readonly(f.owner, false),
         AccountMeta::new_readonly(f.pda, false),
         AccountMeta::new_readonly(f.app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(f.sys_pk, false),
      ],
   );
   let native_user = vec![
      (f.delegate, signer_account(1_000_000_000)),
      (f.owner, Account::default()),
      (f.pda, Account::default()),
      (f.app, Account::default()),
      (lamports_dest, Account::default()),
      (f.sys_pk, f.sys_acct),
   ];
   let mut native_merged = merge_accounts(&native_user, &f.after_deposit);
   let rent = f.mollusk.sysvars.rent.minimum_balance(app_specific_delegated_vaults::state::UserVaultAccount::LEN);
   for (k, a) in native_merged.iter_mut() {
      if k == &f.pda {
         a.lamports = rent + 300_000;
      }
   }
   let native_accounts = with_ix_sysvar_and_loaders(&native_ix, &native_merged);
   let r = f.mollusk.process_and_validate_instruction(
      &native_ix,
      &native_accounts,
      &[Check::err(custom_vault_err(Error::ExpiredDelegate))],
   );
   log_cu("delegate_expiry::cpi_entry_native_fails_after_expiry", &r);
}
