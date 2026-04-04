use crate::common::{
   custom_vault_err, decode_user_vault, derive_user_vault, fresh_mollusk, ix_create, log_cu, merge_accounts,
   signer_account, system_program_meta, test_program_id, token_program_id, vault_program_id,
};
use mollusk_svm::result::Check;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use app_specific_delegated_vaults::error::Error;
use app_specific_delegated_vaults::state::UserVaultAccount;

fn base_create_accounts() -> (Pubkey, Pubkey, Pubkey, Pubkey, Pubkey, Vec<(Pubkey, Account)>) {
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let accounts = vec![
      (owner, signer_account(1_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (sys_pk, sys_acct),
   ];
   (owner, app, delegate, pda, sys_pk, accounts)
}

#[test]
fn create_success_with_max_delegate_expires() {
   let mollusk = fresh_mollusk();
   let (owner, app, delegate, pda, sys_pk, accounts) = base_create_accounts();
   let ix = Instruction::new_with_bytes(
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
   let vault_pid = vault_program_id();
   let checks = [
      Check::success(),
      Check::account(&pda)
         .space(UserVaultAccount::LEN)
         .owner(&vault_pid)
         .build(),
   ];
   let r = mollusk.process_and_validate_instruction(&ix, &accounts, &checks);
   log_cu("create_user_vault::create_success_with_max_delegate_expires", &r);
   let data = r.get_account(&pda).expect("pda").data.clone();
   let v = decode_user_vault(&data).expect("decode");
   assert_eq!(v.owner, owner);
   assert_eq!(v.app_address, app);
   assert_eq!(v.delegate, delegate);
   assert_eq!(v.delegate_expires, u32::MAX);
   assert_eq!(v.ata_count, 0);
}

#[test]
fn create_success_with_future_expiry() {
   let mollusk = fresh_mollusk();
   let (owner, app, delegate, pda, sys_pk, accounts) = base_create_accounts();
   let t: u32 = 4_000_000_000;
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(t),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);
   log_cu("create_user_vault::create_success_with_future_expiry", &r);
   let data = r.get_account(&pda).expect("pda").data.clone();
   let v = decode_user_vault(&data).expect("decode");
   assert_eq!(v.delegate_expires, t);
}

#[test]
fn create_fails_pda_mismatch() {
   let mollusk = fresh_mollusk();
   let (owner, app, delegate, _pda, sys_pk, mut accounts) = base_create_accounts();
   let wrong_pda = Pubkey::new_unique();
   accounts[1].0 = wrong_pda;
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(wrong_pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &accounts,
      &[Check::err(custom_vault_err(Error::UserVaultPdaMismatch))],
   );
   log_cu("create_user_vault::create_fails_pda_mismatch", &r);
}

#[test]
fn create_fails_owner_not_signer() {
   let mollusk = fresh_mollusk();
   let (owner, app, delegate, pda, sys_pk, accounts) = base_create_accounts();
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, false),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &accounts,
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
   log_cu("create_user_vault::create_fails_owner_not_signer", &r);
}

#[test]
fn create_fails_already_initialized() {
   let mollusk = fresh_mollusk();
   let (owner, app, delegate, pda, sys_pk, accounts) = base_create_accounts();
   let ix = Instruction::new_with_bytes(
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
   let r_first = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::success()]);
   log_cu("create_user_vault::create_fails_already_initialized:first_create", &r_first);
   let accounts_after = merge_accounts(&accounts, &r_first);
   let r2 = mollusk.process_and_validate_instruction(
      &ix,
      &accounts_after,
      &[Check::err(custom_vault_err(Error::UserVaultAlreadyExists))],
   );
   log_cu("create_user_vault::create_fails_already_initialized:second_create", &r2);
}

#[test]
fn create_fails_wrong_system_program() {
   let mollusk = fresh_mollusk();
   let (owner, app, delegate, pda, _sys_pk, mut accounts) = base_create_accounts();
   let fake_sys = token_program_id();
   accounts[4].0 = fake_sys;
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(fake_sys, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &accounts,
      &[Check::err(custom_vault_err(Error::InvalidSystemProgram))],
   );
   log_cu("create_user_vault::create_fails_wrong_system_program", &r);
}
