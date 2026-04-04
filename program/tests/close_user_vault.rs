use crate::common::{
   custom_vault_err, decode_user_vault, derive_user_vault, fresh_mollusk, ix_close_user_vault, ix_create, ix_deposit,
   log_cu, merge_accounts, mint_account, signer_account, system_program_meta, test_program_id, token_account,
   token_program_id, vault_program_id,
};
use mollusk_svm::result::Check;
use mollusk_svm_programs_token::{associated_token, token};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use app_specific_delegated_vaults::error::Error;

#[test]
fn close_user_vault_success() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let mint_acct = mint_account(owner, 0);
   let source = crate::common::associated_token_address(&owner, &mint_pk);
   let source_acct = token_account(&mint_pk, &owner, 5_000_000);
   let vault_ata = crate::common::associated_token_address(&pda, &mint_pk);
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
   log_cu("close_user_vault::close_user_vault_success:create", &r0);

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
      &ix_deposit(10_000),
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
   log_cu("close_user_vault::close_user_vault_success:deposit", &r1);

   let withdraw_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged_w = merge_accounts(&withdraw_pre, &r1);
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &crate::common::ix_withdraw(10_000),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
      ],
   );
   let r2 = mollusk.process_and_validate_instruction(&w_ix, &merged_w, &[Check::success()]);
   log_cu("close_user_vault::close_user_vault_success:withdraw", &r2);

   let rent_dest = Pubkey::new_unique();
   let close_ata_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (vault_ata, Account::default()),
      (rent_dest, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged_c = merge_accounts(&close_ata_pre, &r2);
   let close_ata_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &crate::common::ix_close_vault_ata(),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new(rent_dest, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
      ],
   );
   let r3 = mollusk.process_and_validate_instruction(&close_ata_ix, &merged_c, &[Check::success()]);
   log_cu("close_user_vault::close_user_vault_success:close_ata", &r3);
   assert_eq!(decode_user_vault(&r3.get_account(&pda).unwrap().data).unwrap().ata_count, 0);

   let close_vault_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
   ];
   let merged_cv = merge_accounts(&close_vault_pre, &r3);
   let close_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_close_user_vault(),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
      ],
   );
   let r4 = mollusk.process_and_validate_instruction(&close_ix, &merged_cv, &[Check::success()]);
   log_cu("close_user_vault::close_user_vault_success:close_vault", &r4);
}

#[test]
fn close_user_vault_fails_open_atas() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let mint_acct = mint_account(owner, 0);
   let source = crate::common::associated_token_address(&owner, &mint_pk);
   let source_acct = token_account(&mint_pk, &owner, 5_000_000);
   let vault_ata = crate::common::associated_token_address(&pda, &mint_pk);
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
   log_cu("close_user_vault::close_user_vault_fails_open_atas:create", &r0);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, source_acct),
      (mint_pk, mint_acct),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged_d = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(10_000),
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
   log_cu("close_user_vault::close_user_vault_fails_open_atas:deposit", &r1);

   let close_vault_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
   ];
   let merged_cv = merge_accounts(&close_vault_pre, &r1);
   let close_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_close_user_vault(),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
      ],
   );
   let r_cv = mollusk.process_and_validate_instruction(
      &close_ix,
      &merged_cv,
      &[Check::err(custom_vault_err(Error::UserVaultHasOpenAtas))],
   );
   log_cu("close_user_vault::close_user_vault_fails_open_atas:close_vault", &r_cv);
}
