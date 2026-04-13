use crate::common::{
   associated_token_address, bench_delegate, bench_mint, bench_owner, bench_rent_dest, custom_vault_err,
   decode_user_vault, derive_user_vault, fresh_mollusk, ix_close_vault_ata, ix_create, ix_deposit, ix_withdraw,
   log_cu_bench, log_cu_setup, merge_accounts, mint_account, rent_sysvar_account, rent_sysvar_pk, signer_account,
   system_program_meta, test_program_id, token_account, token_program_id, vault_program_id,
};
use mollusk_svm::result::Check;
use mollusk_svm_programs_token::{associated_token, token};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use app_specific_delegated_vaults::error::Error;

fn vault_with_empty_ata() -> (
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
   let merged = merge_accounts(&deposit_accounts, &r0);
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(100_000),
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
      &ix_withdraw(100_000),
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
   (mollusk, r2, owner, app, pda, vault_ata, mint_pk, tok, sys_pk)
}

#[test]
fn close_vault_ata_success() {
   let (mollusk, r2, owner, app, pda, vault_ata, mint_pk, tok, _sys) = vault_with_empty_ata();
   let rent_dest = bench_rent_dest();
   let close_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (vault_ata, Account::default()),
      (rent_dest, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged = merge_accounts(&close_pre, &r2);
   let ata_before = decode_user_vault(&merged.iter().find(|(k, _)| k == &pda).unwrap().1.data)
      .unwrap()
      .ata_count;
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_close_vault_ata(),
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
   let r3 = mollusk.process_and_validate_instruction(&ix, &merged, &[Check::success()]);
   log_cu_bench("close_vault_ata::close_vault_ata_success", &r3);
   let ata_after = decode_user_vault(&r3.get_account(&pda).unwrap().data)
      .unwrap()
      .ata_count;
   assert_eq!(ata_before, 1);
   assert_eq!(ata_after, 0);
}

#[test]
fn close_vault_ata_fails_nonempty() {
   let (mollusk, r1, owner, app, pda, vault_ata, mint_pk, tok, _sys_pk) = {
      let m = fresh_mollusk();
      let owner = Pubkey::new_unique();
      let app = test_program_id();
      let delegate = Pubkey::new_unique();
      let (pda, _) = derive_user_vault(&owner, &app);
      let mint_pk = Pubkey::new_unique();
      let mint_acct = mint_account(owner, 0);
      let source = associated_token_address(&owner, &mint_pk);
      let source_acct = token_account(&mint_pk, &owner, 5_000_000);
      let vault_ata = associated_token_address(&pda, &mint_pk);
      let (sys_pk, sys_acct) = system_program_meta();
      let tok = token_program_id();
      let ata = associated_token::ID;
      let (rent_pk, rent_acct) = rent_sysvar_account(&m);
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
      let r0 = m.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
      log_cu_setup("close_vault_ata::close_vault_ata_fails_nonempty:create", &r0);
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
      let r1 = m.process_and_validate_instruction(&dep_ix, &merged, &[Check::success()]);
      log_cu_setup("close_vault_ata::close_vault_ata_fails_nonempty:deposit", &r1);
      (m, r1, owner, app, pda, vault_ata, mint_pk, tok, sys_pk)
   };

   let rent_dest = Pubkey::new_unique();
   let close_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (vault_ata, Account::default()),
      (rent_dest, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged = merge_accounts(&close_pre, &r1);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_close_vault_ata(),
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
   let r_close = mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(custom_vault_err(Error::UserVaultAtaNotEmpty))],
   );
   log_cu_bench("close_vault_ata::close_vault_ata_fails_nonempty:close_ata", &r_close);
}

#[test]
fn close_vault_ata_fails_ata_count_zero() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let tok = token_program_id();
   let (sys_pk, sys_acct) = system_program_meta();
   let (rent_pk, rent_acct) = rent_sysvar_account(&mollusk);
   let create_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (app, Account::default()),
      (delegate, Account::default()),
      (rent_pk, rent_acct),
      (sys_pk, sys_acct),
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
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu_setup("close_vault_ata::close_vault_ata_fails_ata_count_zero:create", &r0);
   let rent_dest = Pubkey::new_unique();
   let merged = merge_accounts(
      &[
         (owner, signer_account(2_000_000_000)),
         (pda, Account::default()),
         (app, Account::default()),
         (vault_ata, Account::default()),
         (rent_dest, Account::default()),
         (mint_pk, mint_account(owner, 0)),
         (tok, token::keyed_account().1),
      ],
      &r0,
   );
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_close_vault_ata(),
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
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(custom_vault_err(Error::UserVaultAtaCountZero))],
   );
   log_cu_bench("close_vault_ata::close_vault_ata_fails_ata_count_zero:close_ata", &r);
}
