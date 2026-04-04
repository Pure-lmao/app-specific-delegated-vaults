use crate::common::{
   custom_vault_err, decode_user_vault, derive_user_vault, fresh_mollusk, ix_create, ix_update_delegate, log_cu,
   merge_accounts, signer_account, system_program_meta, test_program_id, vault_program_id,
};
use mollusk_svm::result::Check;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use app_specific_delegated_vaults::error::Error;

#[test]
fn update_delegate_and_expires_success() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate_old = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate_old, Account::default()),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate_old, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("update_user_vault_delegate::update_delegate_and_expires_success:create", &r0);
   let delegate_new = Pubkey::new_unique();
   let new_exp: u32 = 3_333_333;
   let upd_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate_new, Account::default()),
   ];
   let merged = merge_accounts(&upd_accounts, &r0);
   let upd_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_update_delegate(new_exp),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate_new, false),
      ],
   );
   let r1 = mollusk.process_and_validate_instruction(&upd_ix, &merged, &[Check::success()]);
   log_cu("update_user_vault_delegate::update_delegate_and_expires_success:update", &r1);
   let v = decode_user_vault(&r1.get_account(&pda).unwrap().data).unwrap();
   assert_eq!(v.delegate, delegate_new);
   assert_eq!(v.delegate_expires, new_exp);
}

#[test]
fn update_fails_owner_not_signer() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate_old = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate_old, Account::default()),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate_old, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("update_user_vault_delegate::update_fails_owner_not_signer:create", &r0);
   let merged = merge_accounts(
      &[
         (owner, signer_account(2_000_000_000)),
         (pda, Account::default()),
         (app, Account::default()),
         (delegate_old, Account::default()),
      ],
      &r0,
   );
   let upd_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_update_delegate(1),
      vec![
         AccountMeta::new(owner, false),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate_old, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(
      &upd_ix,
      &merged,
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
   log_cu("update_user_vault_delegate::update_fails_owner_not_signer:update", &r);
}

#[test]
fn update_fails_owner_mismatch() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let other_owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate_old = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate_old, Account::default()),
      (sys_pk, sys_acct.clone()),
   ];
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate_old, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu("update_user_vault_delegate::update_fails_owner_mismatch:create", &r0);
   let merged = merge_accounts(
      &[
         (other_owner, signer_account(2_000_000_000)),
         (pda, Account::default()),
         (app, Account::default()),
         (delegate_old, Account::default()),
      ],
      &r0,
   );
   let upd_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_update_delegate(1),
      vec![
         AccountMeta::new(other_owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate_old, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(
      &upd_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::UserVaultOwnerMismatch))],
   );
   log_cu("update_user_vault_delegate::update_fails_owner_mismatch:update", &r);
}
