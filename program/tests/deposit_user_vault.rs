use crate::common::{
   associated_token_address, bench_delegate, bench_mint, bench_owner, decode_user_vault, derive_user_vault,
   fresh_mollusk, ix_create, ix_deposit, log_cu_bench, log_cu_setup, merge_accounts, mint_account,
   rent_sysvar_account, rent_sysvar_pk, signer_account, system_program_meta, test_program_id, token_account,
   token_program_id, vault_program_id,
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
#[test]
fn deposit_success_first_creates_ata() {
   let mollusk = fresh_mollusk();
   let owner = bench_owner();
   let app = test_program_id();
   let delegate = bench_delegate();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = bench_mint();
   let mint_acct = mint_account(owner, 0);
   let source = associated_token_address(&owner, &mint_pk);
   let source_acct = token_account(&mint_pk, &owner, 1_000_000);
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let (sys_pk, sys_acct) = system_program_meta();
   let tok = token_program_id();
   let ata = associated_token::ID;
   let tok_acct = token::keyed_account().1;
   let ata_acct = associated_token::keyed_account().1;

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
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu_setup("deposit_user_vault::deposit_success_first_creates_ata:create", &r0);

   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, source_acct),
      (mint_pk, mint_acct),
      (sys_pk, sys_acct),
      (tok, tok_acct),
      (ata, ata_acct),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
   let amount = 100_000u64;
   let dep_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(amount),
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
   let r = mollusk.process_and_validate_instruction(&dep_ix, &merged, &[Check::success()]);
   log_cu_bench("deposit_user_vault::deposit_success_first_creates_ata:deposit", &r);
   let vault_data = r.get_account(&pda).expect("pda").data.clone();
   let v = decode_user_vault(&vault_data).expect("decode");
   assert_eq!(v.ata_count, 1);
   let va = r.get_account(&vault_ata).expect("vault ata");
   let ta = SplTokenAccount::unpack(&va.data).expect("unpack token");
   assert_eq!(ta.amount, amount);
}

#[test]
fn deposit_success_second() {
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
   let tok = token_program_id();
   let ata = associated_token::ID;

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
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu_setup("deposit_user_vault::deposit_success_second:create", &r0);

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
   let merged0 = merge_accounts(&deposit_accounts, &r0);

   let dep_ix = |amt: u64| {
      Instruction::new_with_bytes(
         vault_program_id(),
         &ix_deposit(amt),
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
      )
   };

   let r1 = mollusk.process_and_validate_instruction(&dep_ix(50_000), &merged0, &[Check::success()]);
   log_cu_setup("deposit_user_vault::deposit_success_second:deposit_first", &r1);
   let ata_count_1 = decode_user_vault(&r1.get_account(&pda).unwrap().data)
      .unwrap()
      .ata_count;
   let merged1 = merge_accounts(&merged0, &r1);
   let r2 = mollusk.process_and_validate_instruction(&dep_ix(10_000), &merged1, &[Check::success()]);
   log_cu_bench("deposit_user_vault::deposit_success_second:deposit_second", &r2);
   let ata_count_2 = decode_user_vault(&r2.get_account(&pda).unwrap().data)
      .unwrap()
      .ata_count;
   assert_eq!(ata_count_1, 1);
   assert_eq!(ata_count_2, 1);
}

#[test]
fn deposit_fails_zero_amount() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let (sys_pk, sys_acct) = system_program_meta();
   let tok = token_program_id();
   let ata = associated_token::ID;
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
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu_setup("deposit_user_vault::deposit_fails_zero_amount:create", &r0);
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 100)),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(0),
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
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(ProgramError::InvalidInstructionData)],
   );
   log_cu_bench("deposit_user_vault::deposit_fails_zero_amount:deposit", &r);
}

#[test]
fn deposit_fails_owner_not_signer() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let delegate = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let mint_pk = Pubkey::new_unique();
   let (sys_pk, sys_acct) = system_program_meta();
   let tok = token_program_id();
   let ata = associated_token::ID;
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
   let r0 = mollusk.process_and_validate_instruction(&create_ix, &create_accounts, &[Check::success()]);
   log_cu_setup("deposit_user_vault::deposit_fails_owner_not_signer:create", &r0);
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
   let deposit_accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 100)),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let merged = merge_accounts(&deposit_accounts, &r0);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(10),
      vec![
         AccountMeta::new(owner, false),
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
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &merged,
      &[Check::err(ProgramError::Custom(Error::NotSigner as u32))],
   );
   log_cu_bench("deposit_user_vault::deposit_fails_owner_not_signer:deposit", &r);
}

#[test]
fn deposit_fails_vault_not_found() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let mint_pk = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let vault_ata = associated_token_address(&pda, &mint_pk);
   let source = associated_token_address(&owner, &mint_pk);
   let (sys_pk, sys_acct) = system_program_meta();
   let tok = token_program_id();
   let ata = associated_token::ID;
   let accounts = vec![
      (owner, signer_account(2_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, token_account(&mint_pk, &owner, 100)),
      (mint_pk, mint_account(owner, 0)),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_deposit(10),
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
   let r = mollusk.process_and_validate_instruction(
      &ix,
      &accounts,
      &[Check::err(ProgramError::Custom(Error::UserVaultNotFound as u32))],
   );
   log_cu_bench("deposit_user_vault::deposit_fails_vault_not_found", &r);
}
