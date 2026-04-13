use crate::common::{
   bench_delegate, bench_lamports_dest, bench_owner, clock_sysvar_pk, custom_vault_err, derive_user_vault,
   fresh_mollusk, ix_cpi_entry_native, ix_create, ix_test_bench_noop_top, ix_test_cpi_entry_native_via_vault,
   log_cu_bench, log_cu_bench_cpi_via_caller_split, log_cu_setup, merge_accounts, rent_sysvar_account, rent_sysvar_pk,
   signer_account, system_program_meta, test_program_id, vault_program_id, with_ix_sysvar_and_loaders,
   with_ix_sysvar_clock_and_loaders,
};
use mollusk_svm::result::Check;
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
use app_specific_delegated_vaults::error::Error;
use app_specific_delegated_vaults::state::UserVaultAccount;

#[test]
fn cpi_entry_native_success() {
   let mollusk = fresh_mollusk();
   let owner = bench_owner();
   let app = test_program_id();
   let delegate = bench_delegate();
   let (pda, _) = derive_user_vault(&owner, &app);
   let lamports_dest = bench_lamports_dest();
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
      &ix_create(u32::MAX),
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
   log_cu_setup("cpi_entry_native::cpi_entry_native_success:create", &r0);

   let native_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_cpi_entry_native_via_vault(80_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );

   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
   ];
   let mut merged = with_ix_sysvar_clock_and_loaders(&native_ix, &mollusk, &merge_accounts(&user, &r0));
   let rent = mollusk.sysvars.rent.minimum_balance(UserVaultAccount::LEN);
   for (k, a) in merged.iter_mut() {
      if k == &pda {
         a.lamports = rent + 500_000;
      }
   }
   let stub_ix = Instruction::new_with_bytes(test_program_id(), &ix_test_bench_noop_top(), vec![]);
   let r_stub = mollusk.process_and_validate_instruction(
      &stub_ix,
      &with_ix_sysvar_and_loaders(&stub_ix, &[]),
      &[Check::success()],
   );

   let pda_lamports_before = merged.iter().find(|(k, _)| k == &pda).unwrap().1.lamports;
   let dest_before = merged.iter().find(|(k, _)| k == &lamports_dest).unwrap().1.lamports;
   let r1 = mollusk.process_and_validate_instruction(&native_ix, &merged, &[Check::success()]);
   log_cu_bench_cpi_via_caller_split("cpi_entry_native::cpi_entry_native_success", &r_stub, &r1);
   let pda_after = r1.get_account(&pda).unwrap().lamports;
   let dest_after = r1.get_account(&lamports_dest).unwrap().lamports;
   assert!(dest_after > dest_before);
   assert!(pda_after < pda_lamports_before);
}

#[test]
fn cpi_entry_native_fails_zero_amount() {
   let mollusk = fresh_mollusk();
   let d = Pubkey::new_unique();
   let o = Pubkey::new_unique();
   let p = Pubkey::new_unique();
   let app = test_program_id();
   let l = Pubkey::new_unique();
   let native_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_cpi_entry_native_via_vault(0),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(d, true),
         AccountMeta::new_readonly(o, false),
         AccountMeta::new_readonly(p, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(l, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (d, signer_account(1_000_000_000)),
      (o, Account::default()),
      (p, Account::default()),
      (app, Account::default()),
      (l, Account::default()),
   ];
   let accounts = with_ix_sysvar_clock_and_loaders(&native_ix, &mollusk, &user);
   let r = mollusk.process_and_validate_instruction(
      &native_ix,
      &accounts,
      &[Check::err(ProgramError::InvalidInstructionData)],
   );
   log_cu_bench("cpi_entry_native::cpi_entry_native_fails_zero_amount", &r);
}

#[test]
fn cpi_entry_native_fails_unauthorized_top_level() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let lamports_dest = Pubkey::new_unique();
   let (sys_pk, sys_acct) = system_program_meta();
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_cpi_entry_native(10_000),
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
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
      &ix_create(u32::MAX),
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
   log_cu_setup("cpi_entry_native::cpi_entry_native_fails_unauthorized_top_level:create", &r0);
   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
   ];
   let mut merged = with_ix_sysvar_clock_and_loaders(&ix, &mollusk, &merge_accounts(&user, &r0));
   let rent = mollusk.sysvars.rent.minimum_balance(UserVaultAccount::LEN);
   for (k, a) in merged.iter_mut() {
      if k == &pda {
         a.lamports = rent + 100_000;
      }
   }
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(custom_vault_err(Error::UnauthorizedCpiCaller))],
   );
   log_cu_bench("cpi_entry_native::cpi_entry_native_fails_unauthorized_top_level:vault_ix", &r);
}

#[test]
fn cpi_entry_native_fails_delegate_mismatch() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let wrong = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let lamports_dest = Pubkey::new_unique();
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
      &ix_create(u32::MAX),
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
   log_cu_setup("cpi_entry_native::cpi_entry_native_fails_delegate_mismatch:create", &r0);
   let native_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_cpi_entry_native_via_vault(10_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(wrong, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (wrong, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
   ];
   let mut merged = with_ix_sysvar_clock_and_loaders(&native_ix, &mollusk, &merge_accounts(&user, &r0));
   let rent = mollusk.sysvars.rent.minimum_balance(UserVaultAccount::LEN);
   for (k, a) in merged.iter_mut() {
      if k == &pda {
         a.lamports = rent + 200_000;
      }
   }
   let r = mollusk.process_and_validate_instruction(
      &native_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::InvalidUserVaultDelegate))],
   );
   log_cu_bench("cpi_entry_native::cpi_entry_native_fails_delegate_mismatch:native_cpi", &r);
}
