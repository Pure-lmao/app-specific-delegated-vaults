use crate::common::{
   associated_token_address, derive_user_vault, fresh_mollusk, ix_create, ix_deposit, ix_withdraw, log_cu_bench,
   merge_accounts, mint_account, rent_sysvar_account, rent_sysvar_pk, signer_account, system_program_meta,
   test_program_id, token_account, token_program_id, vault_program_id,
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

fn vault_funded_for_withdraw() -> (
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
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
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
   let r1 = mollusk.process_and_validate_instruction(&dep_ix, &merged, &[Check::success()]);
   (
      mollusk,
      r1,
      owner,
      app,
      pda,
      vault_ata,
      source,
      mint_pk,
      tok,
   )
}

#[test]
fn withdraw_success() {
   let (mollusk, r1, owner, app, pda, vault_ata, dest_ata, mint_pk, tok) = vault_funded_for_withdraw();
   let withdraw_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (dest_ata, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged_w = merge_accounts(&withdraw_pre, &r1);
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw(200_000),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
      ],
   );
   let r2 = mollusk.process_and_validate_instruction(&w_ix, &merged_w, &[Check::success()]);
   log_cu_bench("withdraw_user_vault::withdraw_success", &r2);
   let va = r2.get_account(&vault_ata).unwrap();
   let da = r2.get_account(&dest_ata).unwrap();
   let v_bal = SplTokenAccount::unpack(&va.data).unwrap().amount;
   let d_bal = SplTokenAccount::unpack(&da.data).unwrap().amount;
   assert_eq!(v_bal, 300_000);
   assert_eq!(d_bal, 4_700_000);
}

#[test]
fn withdraw_fails_zero_amount() {
   let (mollusk, r1, owner, app, pda, vault_ata, dest_ata, mint_pk, tok) = vault_funded_for_withdraw();
   let withdraw_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (dest_ata, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged_w = merge_accounts(&withdraw_pre, &r1);
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(
      &w_ix,
      &merged_w,
      &[Check::err(ProgramError::InvalidInstructionData)],
   );
   log_cu_bench("withdraw_user_vault::withdraw_fails_zero_amount", &r);
}

#[test]
fn withdraw_fails_owner_not_signer() {
   let (mollusk, r1, owner, app, pda, vault_ata, dest_ata, mint_pk, tok) = vault_funded_for_withdraw();
   let withdraw_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (dest_ata, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged_w = merge_accounts(&withdraw_pre, &r1);
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw(10),
      vec![
         AccountMeta::new(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(
      &w_ix,
      &merged_w,
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
   log_cu_bench("withdraw_user_vault::withdraw_fails_owner_not_signer", &r);
}

#[test]
fn withdraw_fails_insufficient_balance() {
   let (mollusk, r1, owner, app, pda, vault_ata, dest_ata, mint_pk, tok) = vault_funded_for_withdraw();
   let withdraw_pre = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (dest_ata, Account::default()),
      (mint_pk, mint_account(owner, 0)),
      (tok, token::keyed_account().1),
   ];
   let merged_w = merge_accounts(&withdraw_pre, &r1);
   let w_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_withdraw(999_999_999),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint_pk, false),
         AccountMeta::new_readonly(tok, false),
      ],
   );
   let r = mollusk.process_instruction(&w_ix, &merged_w);
   log_cu_bench("withdraw_user_vault::withdraw_fails_insufficient_balance", &r);
   assert!(r.program_result.is_err());
}
