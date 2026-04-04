use crate::common::{
   all_loader_accounts, derive_user_vault, fresh_mollusk, instructions_sysvar_for, ix_create, log_cu,
   overlay_accounts, signer_account, system_program_meta, test_program_id, token_program_id, vault_program_id,
};
use mollusk_svm::result::Check;
use mollusk_svm_programs_token::{associated_token, token};
use solana_account::Account;
use solana_instruction::{AccountMeta, Instruction};
use solana_program_error::ProgramError;
use solana_pubkey::Pubkey;
fn create_five_account_setup() -> (Pubkey, Pubkey, Pubkey, Pubkey, Vec<(Pubkey, Account)>) {
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
   (owner, app, delegate, pda, accounts)
}

#[test]
fn router_rejects_empty_ix_data() {
   let mollusk = fresh_mollusk();
   let ix = Instruction::new_with_bytes(vault_program_id(), &[], vec![]);
   let r = mollusk.process_and_validate_instruction(&ix, &[], &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu("instruction_router::router_rejects_empty_ix_data", &r);
}

#[test]
fn router_rejects_unknown_discriminator() {
   let mollusk = fresh_mollusk();
   let ix = Instruction::new_with_bytes(vault_program_id(), &[255], vec![]);
   let r = mollusk.process_and_validate_instruction(&ix, &[], &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu("instruction_router::router_rejects_unknown_discriminator", &r);
}

#[test]
fn deposit_rejects_truncated_amount() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let mint = Pubkey::new_unique();
   let vault_ata = Pubkey::new_unique();
   let source = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let (sys_pk, sys_acct) = system_program_meta();
   let tok = token_program_id();
   let ata = associated_token::ID;
   let data = vec![1u8, 1, 2, 3];
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &data,
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(source, false),
         AccountMeta::new_readonly(mint, false),
         AccountMeta::new_readonly(sys_pk, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(ata, false),
      ],
   );
   let accounts = vec![
      (owner, signer_account(1_000_000_000)),
      (pda, Account::default()),
      (vault_ata, Account::default()),
      (app, Account::default()),
      (source, Account::default()),
      (mint, Account::default()),
      (sys_pk, sys_acct),
      (tok, token::keyed_account().1),
      (ata, associated_token::keyed_account().1),
   ];
   let r = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu("instruction_router::deposit_rejects_truncated_amount", &r);
}

#[test]
fn withdraw_rejects_truncated_amount() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let mint = Pubkey::new_unique();
   let vault_ata = Pubkey::new_unique();
   let dest_ata = Pubkey::new_unique();
   let (pda, _) = derive_user_vault(&owner, &app);
   let tok = token_program_id();
   let data = vec![3u8, 0, 0, 0];
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &data,
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint, false),
         AccountMeta::new_readonly(tok, false),
      ],
   );
   let accounts = overlay_accounts(&[
      &[
         (owner, signer_account(1_000_000_000)),
         (pda, Account::default()),
         (vault_ata, Account::default()),
         (app, Account::default()),
         (dest_ata, Account::default()),
         (mint, Account::default()),
         (tok, token::keyed_account().1),
      ],
      &all_loader_accounts(),
   ]);
   let r = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu("instruction_router::withdraw_rejects_truncated_amount", &r);
}

#[test]
fn create_rejects_truncated_delegate_expires() {
   let mollusk = fresh_mollusk();
   let (owner, app, delegate, pda, accounts) = create_five_account_setup();
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &[0u8, 1, 2],
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate, false),
         AccountMeta::new_readonly(accounts[4].0, false),
      ],
   );
   let r = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu("instruction_router::create_rejects_truncated_delegate_expires", &r);
}

#[test]
fn update_delegate_rejects_truncated_expires() {
   let mollusk = fresh_mollusk();
   let (owner, app, delegate_old, pda, base) = create_five_account_setup();
   let create_ix = Instruction::new_with_bytes(
      vault_program_id(),
      &ix_create(0),
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate_old, false),
         AccountMeta::new_readonly(base[4].0, false),
      ],
   );
   let r_create =
      mollusk.process_and_validate_instruction(&create_ix, &base, &[Check::success()]);
   log_cu("instruction_router::update_delegate_rejects_truncated_expires:create", &r_create);
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &[2u8, 9],
      vec![
         AccountMeta::new(owner, true),
         AccountMeta::new(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new_readonly(delegate_old, false),
      ],
   );
   let acc2: Vec<_> = base.iter().take(4).cloned().collect();
   let r_upd = mollusk.process_and_validate_instruction(&ix, &acc2, &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu("instruction_router::update_delegate_rejects_truncated_expires:update", &r_upd);
}

#[test]
fn cpi_entry_rejects_truncated_payload() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let (pda, _) = derive_user_vault(&owner, &app);
   let tok = token_program_id();
   let delegate = Pubkey::new_unique();
   let vault_ata = Pubkey::new_unique();
   let lamports_dest = Pubkey::new_unique();
   let dest_ata = Pubkey::new_unique();
   let mint = Pubkey::new_unique();
   let (sys_pk, sys_acct) = system_program_meta();
   let data = vec![6u8, 1, 2, 3, 4, 5, 6, 7];
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &data,
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new(vault_ata, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new(dest_ata, false),
         AccountMeta::new_readonly(mint, false),
         AccountMeta::new_readonly(tok, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let (ixs_pk, ixs_acct) = instructions_sysvar_for(&ix);
   let accounts = overlay_accounts(&[
      &[
         (delegate, signer_account(1_000_000_000)),
         (owner, Account::default()),
         (pda, Account::default()),
         (vault_ata, Account::default()),
         (app, Account::default()),
         (lamports_dest, Account::default()),
         (dest_ata, Account::default()),
         (mint, Account::default()),
         (tok, token::keyed_account().1),
         (ixs_pk, ixs_acct),
         (sys_pk, sys_acct),
      ],
      &all_loader_accounts(),
   ]);
   let r = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu("instruction_router::cpi_entry_rejects_truncated_payload", &r);
}

#[test]
fn cpi_entry_native_rejects_truncated_payload() {
   let mollusk = fresh_mollusk();
   let owner = Pubkey::new_unique();
   let app = test_program_id();
   let (pda, _) = derive_user_vault(&owner, &app);
   let delegate = Pubkey::new_unique();
   let lamports_dest = Pubkey::new_unique();
   let (sys_pk, sys_acct) = system_program_meta();
   let data = vec![7u8, 1, 2, 3];
   let ix = Instruction::new_with_bytes(
      vault_program_id(),
      &data,
      vec![
         AccountMeta::new(delegate, true),
         AccountMeta::new_readonly(owner, false),
         AccountMeta::new_readonly(pda, false),
         AccountMeta::new_readonly(app, false),
         AccountMeta::new(lamports_dest, false),
         AccountMeta::new_readonly(solana_instructions_sysvar::ID, false),
         AccountMeta::new_readonly(sys_pk, false),
      ],
   );
   let (ixs_pk, ixs_acct) = instructions_sysvar_for(&ix);
   let accounts = overlay_accounts(&[
      &[
         (delegate, signer_account(1_000_000_000)),
         (owner, Account::default()),
         (pda, Account::default()),
         (app, Account::default()),
         (lamports_dest, Account::default()),
         (ixs_pk, ixs_acct),
         (sys_pk, sys_acct),
      ],
      &all_loader_accounts(),
   ]);
   let r = mollusk.process_and_validate_instruction(&ix, &accounts, &[Check::err(ProgramError::InvalidInstructionData)]);
   log_cu("instruction_router::cpi_entry_native_rejects_truncated_payload", &r);
}
