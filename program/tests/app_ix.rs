use crate::common::{
   associated_token_address, custom_vault_err, derive_user_vault, fresh_mollusk, ix_app_ix, ix_create, ix_deposit,
   log_cu, merge_accounts, mint_account, signer_account, system_program_meta, test_program_id, token_account,
   token_program_id, treasury_pda, vault_program_id, with_loader_accounts,
};
use mollusk_svm::result::Check;
use mollusk_svm_programs_token::{associated_token, token};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_program_pack::Pack;
use solana_pubkey::Pubkey;
use spl_token_interface::state::Account as SplTokenAccount;
use app_specific_delegated_vaults::error::Error;

fn inner_deposit_from_user(amount: u64) -> Vec<u8> {
   let mut v = vec![2u8];
   v.extend_from_slice(&amount.to_le_bytes());
   v
}

#[test]
fn app_ix_success_deposit_from_user() {
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
   let (treasury_pda, _) = treasury_pda();
   let treasury_ata = associated_token_address(&treasury_pda, &mint_pk);
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
      &ix_create(u32::MAX),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("app_ix::app_ix_success_deposit_from_user:create", &r0);

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
      &ix_deposit(800_000),
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
   let r1 = mollusk.process_and_validate_instruction(&dep_ix, &merged_d, &[Check::success()]);
   log_cu("app_ix::app_ix_success_deposit_from_user:deposit", &r1);

   let app_ix_data = ix_app_ix(&inner_deposit_from_user(50_000));
   // `app_ix`: 4 fixed + inner `deposit_from_user` (9): signer, authority, source_ata, treasury_pda, treasury_ata, mint, system, token, ata.
   let vault_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &app_ix_data,
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new(owner, true),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(delegate, true),
         AccountMeta::new(owner, true),
         AccountMeta::new(owner_source, false),
         AccountMeta::new_readonly(treasury_pda, false),
         AccountMeta::new(treasury_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );

   let app_accounts = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, signer_account(1_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (owner_source, Account::default()),
      (treasury_pda, Account::default()),
      (treasury_ata, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged_app = merge_accounts(&app_accounts, &r1);
   let for_app = with_loader_accounts(&merged_app);

   let r2 = mollusk.process_and_validate_instruction(&vault_ix, &for_app, &[Check::success()]);
   log_cu("app_ix::app_ix_success_deposit_from_user:app_ix", &r2);
   let t_after = SplTokenAccount::unpack(&r2.get_account(&treasury_ata).unwrap().data)
      .unwrap()
      .amount;
   assert_eq!(t_after, 50_000);
}

#[test]
fn app_ix_fails_delegate_not_signer() {
   let mollusk = fresh_mollusk();
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
      &ix_create(u32::MAX),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("app_ix::app_ix_fails_delegate_not_signer:create", &r0);
   let mint_pk = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let (treasury_pda, _) = treasury_pda();
   let treasury_ata = associated_token_address(&treasury_pda, &mint_pk);
   let tok = token_program_id();
   let ata = associated_token::ID;

   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner_deposit_from_user(1)),
      vec![
         AccountMeta::new(delegate, false),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(treasury_pda, false),
         AccountMeta::new(treasury_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let app_accounts = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (app, Account::default()),
      (vault_ata, Account::default()),
      (treasury_pda, Account::default()),
      (treasury_ata, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&app_accounts, &r0);
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
   log_cu("app_ix::app_ix_fails_delegate_not_signer:app_ix", &r);
}

#[test]
fn app_ix_fails_wrong_delegate() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let wrong_del = Pubkey::new_unique();
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
      &ix_create(u32::MAX),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("app_ix::app_ix_fails_wrong_delegate:create", &r0);
   let mint_pk = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let (treasury_pda, _) = treasury_pda();
   let treasury_ata = associated_token_address(&treasury_pda, &mint_pk);
   let tok = token_program_id();
   let ata = associated_token::ID;

   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_app_ix(&inner_deposit_from_user(1)),
      vec![
         AccountMeta::new(wrong_del, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(treasury_pda, false),
         AccountMeta::new(treasury_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let app_accounts = vec![
      (wrong_del, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (app, Account::default()),
      (vault_ata, Account::default()),
      (treasury_pda, Account::default()),
      (treasury_ata, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&app_accounts, &r0);
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &with_loader_accounts(&merged),
      &[Check::err(custom_vault_err(Error::InvalidUserVaultDelegate))],
   );
   log_cu("app_ix::app_ix_fails_wrong_delegate:app_ix", &r);
}
