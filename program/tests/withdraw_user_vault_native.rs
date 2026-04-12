use crate::common::{
   derive_user_vault, fresh_mollusk, ix_create, ix_withdraw_native, log_cu_bench, merge_accounts, rent_sysvar_account,
   rent_sysvar_pk, signer_account, system_program_meta, test_program_id, vault_program_id,
};
use mollusk_svm::result::Check;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use app_specific_delegated_vaults::error::Error;
use app_specific_delegated_vaults::state::UserVaultAccount;

fn create_vault_only() -> (
   mollusk_svm::Mollusk,
   mollusk_svm::result::InstructionResult,
   Pubkey,
   Pubkey,
   Pubkey,
   Pubkey,
) {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
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
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(rent_sysvar_pk(), false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let r0 = mollusk.process_and_validate_instruction(&create_ix, create_accounts.as_slice(), &[Check::success()]);
   (mollusk, r0, owner, app, delegate, pda)
}

#[test]
fn withdraw_native_success() {
   let (mollusk, r0, owner, app, _delegate, pda) = create_vault_only();
   let rent = mollusk.sysvars.rent.minimum_balance(UserVaultAccount::LEN);
   let withdraw_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
   ];
   let mut merged = merge_accounts(&withdraw_pre, &r0);
   for (k, a) in merged.iter_mut() {
      if k == &pda {
         a.lamports = rent + 750_000;
      }
   }
   let owner_lamports_before = merged.iter().find(|(k, _)| k == &owner).unwrap().1.lamports;
   let pda_lamports_before = merged.iter().find(|(k, _)| k == &pda).unwrap().1.lamports;
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw_native(120_000),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
      ],
   );
   let r2 = mollusk.process_and_validate_instruction(&w_ix, &merged, &[Check::success()]);
   log_cu_bench("withdraw_user_vault_native::withdraw_native_success", &r2);
   let owner_after = r2.get_account(&owner).unwrap().lamports;
   let pda_after = r2.get_account(&pda).unwrap().lamports;
   assert_eq!(owner_after, owner_lamports_before + 120_000);
   assert_eq!(pda_after, pda_lamports_before - 120_000);
}

#[test]
fn withdraw_native_fails_zero_amount() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let (pda, _) = derive_user_vault(&owner, &app);
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw_native(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
      ],
   );
   let accounts = vec![
      (owner, signer_account(1_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
   ];
   let r = mollusk.process_and_validate_instruction(
      &w_ix,
      &accounts,
      &[Check::err(ProgramError::InvalidInstructionData)],
   );
   log_cu_bench("withdraw_user_vault_native::withdraw_native_fails_zero_amount", &r);
}

#[test]
fn withdraw_native_fails_owner_not_signer() {
   let (mollusk, r0, owner, app, _delegate, pda) = create_vault_only();
   let rent = mollusk.sysvars.rent.minimum_balance(UserVaultAccount::LEN);
   let withdraw_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
   ];
   let mut merged = merge_accounts(&withdraw_pre, &r0);
   for (k, a) in merged.iter_mut() {
      if k == &pda {
         a.lamports = rent + 400_000;
      }
   }
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw_native(10_000),
      vec![
         AccountMeta::new(owner, false),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(
      &w_ix,
      &merged,
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
   log_cu_bench("withdraw_user_vault_native::withdraw_native_fails_owner_not_signer", &r);
}
