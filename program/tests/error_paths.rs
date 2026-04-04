// Negative tests for custom errors that were not covered elsewhere.

use crate::common::{
   associated_token_address, custom_vault_err, derive_user_vault, fresh_mollusk, ix_app_ix, ix_create, ix_deposit,
   ix_withdraw, log_cu, merge_accounts, mint_account, set_clock_unix, signer_account, system_program_meta,
   test_program_id, token_account, token_program_id, vault_program_id, with_loader_accounts,
};
use mollusk_svm::result::Check;
use mollusk_svm_programs_token::{associated_token, token};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use app_specific_delegated_vaults::error::Error;
use app_specific_delegated_vaults::state::UserVaultAccount;

const NONZERO_EXPIRY: u32 = 9_000_000;

#[test]
fn app_ix_fails_invalid_unix_timestamp_zero_clock() {
   let mut mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();

   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(NONZERO_EXPIRY),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::app_ix_fails_invalid_unix_timestamp_zero_clock:create", &r0);

   set_clock_unix(&mut mollusk, 0);

   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&[]),
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new_readonly(app, false),
      ],
   );
   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, signer_account(1_000_000_000)),
      (pda, r0.get_account(&pda).expect("pda").clone()),
      (app, Account::default()),
   ];
   let r1 = mollusk.process_and_validate_instruction(
      &vault_ix,
      &with_loader_accounts(&user),
      &[Check::err(custom_vault_err(Error::InvalidUnixTimestamp))],
   );
   log_cu("error_paths::app_ix_fails_invalid_unix_timestamp_zero_clock:app_ix", &r1);
}

#[test]
fn app_ix_fails_invalid_unix_timestamp_above_u32() {
   let mut mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();

   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(NONZERO_EXPIRY),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::app_ix_fails_invalid_unix_timestamp_above_u32:create", &r0);

   set_clock_unix(&mut mollusk, (u32::MAX as i64) + 1);

   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&[]),
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new_readonly(app, false),
      ],
   );
   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, signer_account(1_000_000_000)),
      (pda, r0.get_account(&pda).expect("pda").clone()),
      (app, Account::default()),
   ];
   let r1 = mollusk.process_and_validate_instruction(
      &vault_ix,
      &with_loader_accounts(&user),
      &[Check::err(custom_vault_err(Error::InvalidUnixTimestamp))],
   );
   log_cu("error_paths::app_ix_fails_invalid_unix_timestamp_above_u32:app_ix", &r1);
}

#[test]
fn deposit_fails_user_vault_app_address_mismatch() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let wrong_app = Pubkey::new_unique();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
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
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::deposit_fails_user_vault_app_address_mismatch:create", &r0);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (wrong_app, Account::default()),
      (source, token_account(&mint_pk, &owner, 1_000_000)),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(100),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(wrong_app, false),
         AccountMeta::new(source, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let r_dep = mollusk.process_and_validate_instruction(
      &dep_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::UserVaultAppAddressMismatch))],
   );
   log_cu("error_paths::deposit_fails_user_vault_app_address_mismatch:deposit", &r_dep);
}

#[test]
fn deposit_fails_source_mint_mismatch() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let other_mint = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
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
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::deposit_fails_source_mint_mismatch:create", &r0);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 1_000_000)),
      (other_mint, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(100),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source, false),
         AccountMeta::new_readonly(other_mint, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let r_dep = mollusk.process_and_validate_instruction(
      &dep_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::MintMismatch))],
   );
   log_cu("error_paths::deposit_fails_source_mint_mismatch:deposit", &r_dep);
}

#[test]
fn deposit_fails_mint_metadata_too_short() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
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
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::deposit_fails_mint_metadata_too_short:create", &r0);

   let short_mint = Account {
      lamports: 1_000_000,
      data: vec![0u8; 16],
      owner: tok,
      executable: false,
      rent_epoch: 0,
   };

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 1_000_000)),
      (mint_pk, short_mint),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(100),
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
   let r_dep = mollusk.process_and_validate_instruction(
      &dep_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::InvalidAta))],
   );
   log_cu("error_paths::deposit_fails_mint_metadata_too_short:deposit", &r_dep);
}

#[test]
fn deposit_fails_vault_ata_mint_mismatch_after_first_deposit() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let other_mint = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
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
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::deposit_fails_vault_ata_mint_mismatch_after_first_deposit:create", &r0);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 2_000_000)),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct.clone()),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged0 = merge_accounts(&deposit_accounts, &r0);
   let dep_ix0 = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(50_000),
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
   let r1 = mollusk.process_and_validate_instruction(&dep_ix0, &merged0, &[Check::success()]);
   log_cu(
      "error_paths::deposit_fails_vault_ata_mint_mismatch_after_first_deposit:first_deposit",
      &r1,
   );

   let deposit2 = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 1_900_000)),
      (other_mint, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged1 = merge_accounts(&deposit2, &r1);
   let dep_ix1 = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(10_000),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source, false),
         AccountMeta::new_readonly(other_mint, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let r2 = mollusk.process_and_validate_instruction(
      &dep_ix1,
      &merged1,
      &[Check::err(custom_vault_err(Error::MintMismatch))],
   );
   log_cu(
      "error_paths::deposit_fails_vault_ata_mint_mismatch_after_first_deposit:second_deposit",
      &r2,
   );
}

#[test]
fn deposit_fails_invalid_token_program() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
   let (sys_pk, sys_acct) = system_program_meta();
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
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::deposit_fails_invalid_token_program:create", &r0);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 1_000_000)),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(100),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let r_dep = mollusk.process_and_validate_instruction(
      &dep_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::InvalidTokenProgram))],
   );
   log_cu("error_paths::deposit_fails_invalid_token_program:deposit", &r_dep);
}

#[test]
fn deposit_fails_invalid_associated_token_program() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
   let (sys_pk, sys_acct) = system_program_meta();
   let tok = token_program_id();

   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::deposit_fails_invalid_associated_token_program:create", &r0);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 1_000_000)),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(100),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r_dep = mollusk.process_and_validate_instruction(
      &dep_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::InvalidAssociatedTokenProgram))],
   );
   log_cu("error_paths::deposit_fails_invalid_associated_token_program:deposit", &r_dep);
}

#[test]
fn deposit_fails_ata_count_overflow() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_a = Pubkey::new_unique();
   let mint_b = Pubkey::new_unique();
   let vault_ata_b = associated_token_address(&pda, &mint_b);
   let source_b = associated_token_address(&owner, &mint_b);
   let (sys_pk, sys_acct) = system_program_meta();
   let tok = token_program_id();
   let ata = associated_token::ID;

   let create_accounts = vec![
      (owner, signer_account(3_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::deposit_fails_ata_count_overflow:create", &r0);

   let vault_ata_a = associated_token_address(&pda, &mint_a);
   let source_a = associated_token_address(&owner, &mint_a);
   let deposit_a = vec![
      (owner, signer_account(3_000_000_000)),
      (pda, Account::default()),
      (vault_ata_a, Account::default()),
      (app, Account::default()),
      (source_a, token_account(&mint_a, &owner, 2_000_000)),
      (mint_a, mint_account(owner, 0)),
      (sys_pk, sys_acct.clone()),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged_a = merge_accounts(&deposit_a, &r0);
   let dep_a = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(10_000),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata_a, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source_a, false),
         AccountMeta::new_readonly(mint_a, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let r1 = mollusk.process_and_validate_instruction(&dep_a, &merged_a, &[Check::success()]);
   log_cu("error_paths::deposit_fails_ata_count_overflow:first_mint_deposit", &r1);

   let mut pda_data = r1.get_account(&pda).expect("pda").data.clone();
   let mut vault = UserVaultAccount::unpack(&pda_data).expect("unpack vault");
   vault.ata_count = u16::MAX;
   vault.pack(&mut pda_data).expect("pack vault");

   let pda_patched = Account {
      lamports: r1.get_account(&pda).expect("pda").lamports,
      data: pda_data,
      owner: r1.get_account(&pda).expect("pda").owner,
      executable: false,
      rent_epoch: 0,
   };
   let deposit_b = vec![
      (owner, signer_account(3_000_000_000)),
      (pda, pda_patched.clone()),
      (vault_ata_b, Account::default()),
      (app, Account::default()),
      (source_b, token_account(&mint_b, &owner, 2_000_000)),
      (mint_b, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let mut merged_b = merge_accounts(&deposit_b, &r1);
   for entry in &mut merged_b {
      if entry.0 == pda {
         entry.1 = pda_patched.clone();
         break;
      }
   }
   let dep_b = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(10_000),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata_b, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source_b, false),
         AccountMeta::new_readonly(mint_b, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let r_b = mollusk.process_and_validate_instruction(
      &dep_b,
      &merged_b,
      &[Check::err(custom_vault_err(Error::ArithmeticOverflow))],
   );
   log_cu("error_paths::deposit_fails_ata_count_overflow:second_mint_deposit", &r_b);
}

#[test]
fn withdraw_fails_dest_token_owner_mismatch() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let mint_acct = mint_account(owner, 0);
   let source = associated_token_address(&owner, &mint_pk);
   let source_acct = token_account(&mint_pk, &owner, 5_000_000);
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let other = Pubkey::new_unique();
   let dest = associated_token_address(&other, &mint_pk);
   let dest_acct = token_account(&mint_pk, &other, 0);
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
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("error_paths::withdraw_fails_dest_token_owner_mismatch:create", &r0);

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
      &ix_deposit(500_000),
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
   let r1 = mollusk.process_and_validate_instruction(&dep_ix, &merged_d, &[Check::success()]);
   log_cu("error_paths::withdraw_fails_dest_token_owner_mismatch:deposit", &r1);

   let withdraw_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (dest, dest_acct),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged_w = merge_accounts(&withdraw_accounts, &r1);
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw(10_000),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(dest, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
      ],
   );
   let r_w = mollusk.process_and_validate_instruction(
      &w_ix,
      &merged_w,
      &[Check::err(custom_vault_err(Error::InvalidAta))],
   );
   log_cu("error_paths::withdraw_fails_dest_token_owner_mismatch:withdraw", &r_w);
}
