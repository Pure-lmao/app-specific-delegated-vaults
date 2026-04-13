use crate::common::{
   associated_token_address, bench_delegate, bench_dest_owner, bench_lamports_dest, bench_mint, bench_owner,
   clock_sysvar_pk, custom_vault_err, derive_user_vault, fresh_mollusk, ix_create, ix_cpi_entry, ix_deposit,
   ix_test_bench_noop_top, ix_test_cpi_entry_dual_via_vault, ix_test_deposit_via_cpi, log_cu_bench,
   log_cu_bench_cpi_via_caller_split, merge_accounts, mint_account, rent_sysvar_account, rent_sysvar_pk,
   signer_account, system_program_meta, test_program_id, token_account, token_program_id, vault_program_id,
   with_ix_sysvar_and_loaders, with_ix_sysvar_clock_and_loaders,
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

fn funded_vault_for_cpi() -> (
   mollusk_svm::Mollusk,
   mollusk_svm::result::InstructionResult,
   Pubkey,
   Pubkey,
   Pubkey,
   Pubkey,
   Pubkey,
   Pubkey,
   Pubkey,
) {
   let mollusk = fresh_mollusk();
   let owner = bench_owner();
   let app = test_program_id();
   let delegate = bench_delegate();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = bench_mint();
   let mint_acct = mint_account(owner, 0);
   let source = associated_token_address(&owner, &mint_pk);
   let source_acct = token_account(&mint_pk, &owner, 5_000_000);
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
   let merged = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(600_000),
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
   let r1 = mollusk.process_and_validate_instruction(&dep_ix, &merged, &[Check::success()]);
   (mollusk, r1, owner, app, delegate, pda, vault_ata, mint_pk, tok)
}

#[test]
fn cpi_entry_success_spl_only_via_test_program() {
   let (mollusk, r1, owner, app, delegate, pda, vault_ata, mint_pk, tok) = funded_vault_for_cpi();
   let lamports_dest = bench_lamports_dest();
   let dest_owner = bench_dest_owner();
   let dest_ata = associated_token_address(&dest_owner, &mint_pk);

   let cpi_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_deposit_via_cpi(100_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );

   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&mint_pk, &dest_owner, 0)),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged_cpi = with_ix_sysvar_clock_and_loaders(&cpi_ix, &mollusk, &merge_accounts(&user, &r1));

   let stub_ix = Instruction::new_with_bytes(test_program_id(), &ix_test_bench_noop_top(), vec![]);
   let r_stub = mollusk.process_and_validate_instruction(
      &stub_ix,
      &with_ix_sysvar_and_loaders(&stub_ix, &[]),
      &[Check::success()],
   );

   let lamports_before = merged_cpi
      .iter()
      .find(|(k, _)| k == &lamports_dest)
      .unwrap()
      .1
      .lamports;
   let r2 = mollusk.process_and_validate_instruction(&cpi_ix, &merged_cpi, &[Check::success()]);
   log_cu_bench_cpi_via_caller_split("cpi_entry::cpi_entry_success_spl_only_via_test_program", &r_stub, &r2);
   let lamports_after = r2.get_account(&lamports_dest).unwrap().lamports;
   assert_eq!(lamports_before, lamports_after);
   let d_bal = SplTokenAccount::unpack(&r2.get_account(&dest_ata).unwrap().data)
      .unwrap()
      .amount;
   assert_eq!(d_bal, 100_000);
}

#[test]
fn cpi_entry_success_native_and_spl_via_test_program() {
   let (mollusk, r1, owner, app, delegate, pda, vault_ata, mint_pk, tok) = funded_vault_for_cpi();
   let lamports_dest = bench_lamports_dest();
   let dest_owner = bench_dest_owner();
   let dest_ata = associated_token_address(&dest_owner, &mint_pk);

   // Dual CPI payload (two u64s) via `test_program`; SPL leg only here — Mollusk flags
   // `UnbalancedInstruction` when this vault ix both adjusts PDA lamports and CPIs token in one step.
   let dual_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_cpi_entry_dual_via_vault(0, 40_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );

   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&mint_pk, &dest_owner, 0)),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let mut merged_cpi = with_ix_sysvar_clock_and_loaders(&dual_ix, &mollusk, &merge_accounts(&user, &r1));
   let rent = mollusk.sysvars.rent.minimum_balance(app_specific_delegated_vaults::state::UserVaultAccount::LEN);
   for (k, a) in merged_cpi.iter_mut() {
      if k == &pda {
         a.lamports = rent + 200_000;
      }
   }

   let stub_ix = Instruction::new_with_bytes(test_program_id(), &ix_test_bench_noop_top(), vec![]);
   let r_stub = mollusk.process_and_validate_instruction(
      &stub_ix,
      &with_ix_sysvar_and_loaders(&stub_ix, &[]),
      &[Check::success()],
   );

   let lamports_before = merged_cpi.iter().find(|(k, _)| k == &lamports_dest).unwrap().1.lamports;
   let r2 = mollusk.process_and_validate_instruction(&dual_ix, &merged_cpi, &[Check::success()]);
   log_cu_bench_cpi_via_caller_split(
      "cpi_entry::cpi_entry_success_native_and_spl_via_test_program",
      &r_stub,
      &r2,
   );
   let lamports_after = r2.get_account(&lamports_dest).unwrap().lamports;
   assert_eq!(lamports_after, lamports_before);
   let spl = SplTokenAccount::unpack(&r2.get_account(&dest_ata).unwrap().data)
      .unwrap()
      .amount;
   assert_eq!(spl, 40_000);
}

#[test]
fn cpi_entry_fails_both_amounts_zero() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let tok = token_program_id();
   let vault_ata = Pubkey::new_unique();
   let lamports_dest = Pubkey::new_unique();
   let dest_ata = Pubkey::new_unique();
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_cpi_entry(0, 0),
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, Account::default()),
      (mint_pk, Account::default()),
      (tok, token::keyed_account().1),
   ];
   let accounts = with_ix_sysvar_clock_and_loaders(&ix, &mollusk, &user);
   let r = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu_bench("cpi_entry::cpi_entry_fails_both_amounts_zero", &r);
}

#[test]
fn cpi_entry_fails_direct_vault_top_level() {
   let (mollusk, r1, owner, app, delegate, pda, vault_ata, mint_pk, tok) = funded_vault_for_cpi();
   let lamports_dest = Pubkey::new_unique();
   let dest_owner = Pubkey::new_unique();
   let dest_ata = associated_token_address(&dest_owner, &mint_pk);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_cpi_entry(0, 10_000),
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&mint_pk, &dest_owner, 0)),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged = with_ix_sysvar_clock_and_loaders(&ix, &mollusk, &merge_accounts(&user, &r1));
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(custom_vault_err(Error::UnauthorizedCpiCaller))],
   );
   log_cu_bench("cpi_entry::cpi_entry_fails_direct_vault_top_level", &r);
}

#[test]
fn cpi_entry_fails_delegate_mismatch() {
   let (mollusk, r1, owner, app, _delegate, pda, vault_ata, mint_pk, tok) = funded_vault_for_cpi();
   let wrong = Pubkey::new_unique();
   let lamports_dest = Pubkey::new_unique();
   let dest_owner = Pubkey::new_unique();
   let dest_ata = associated_token_address(&dest_owner, &mint_pk);
   let cpi_ix = Instruction::new_with_bytes(
      test_program_id(),
      &ix_test_deposit_via_cpi(10_000),
      vec![
         AccountMeta::new_readonly(vault_program_id(), false),
         AccountMeta::new(wrong, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(clock_sysvar_pk(), false),
      ],
   );
   let user = vec![
      (wrong, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (lamports_dest, Account::default()),
      (dest_ata, token_account(&mint_pk, &dest_owner, 0)),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged = with_ix_sysvar_clock_and_loaders(&cpi_ix, &mollusk, &merge_accounts(&user, &r1));
   let r = mollusk.process_and_validate_instruction(
      &cpi_ix,
      &merged,
      &[Check::err(custom_vault_err(Error::InvalidUserVaultDelegate))],
   );
   log_cu_bench("cpi_entry::cpi_entry_fails_delegate_mismatch", &r);
}

#[test]
fn cpi_entry_fails_not_enough_accounts() {
   let (mollusk, r1, owner, app, delegate, pda, vault_ata, mint_pk, tok) = funded_vault_for_cpi();
   let x1 = Pubkey::new_unique();
   let x2 = Pubkey::new_unique();
   let x3 = Pubkey::new_unique();
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_cpi_entry(0, 1),
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(x1, false),
         AccountMeta::new(x2, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(x3, false),
      ],
   );
   let user = vec![
      (delegate, signer_account(1_000_000_000)),
      (owner, Account::default()),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (x1, Account::default()),
      (x2, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
      (x3, Account::default()),
   ];
   let merged = with_ix_sysvar_and_loaders(&ix, &merge_accounts(&user, &r1));
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(ProgramError::NotEnoughAccountKeys)],
   );
   log_cu_bench("cpi_entry::cpi_entry_fails_not_enough_accounts", &r);
}
