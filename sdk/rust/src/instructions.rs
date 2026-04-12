//! Vault program `Instruction` builders (account order matches `program/src/instructions/*` and the TS SDK).

use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::constants::{
   ASSOCIATED_TOKEN_PROGRAM_ID, DEFAULT_VAULT_PROGRAM_ID, SPL_TOKEN_PROGRAM_ID, SYSTEM_PROGRAM_ID,
   SYSVAR_CLOCK_ID, SYSVAR_INSTRUCTIONS_ID,
};
use crate::error::VaultSdkError;

/// On-chain instruction indices (`program/src/instructions/mod.rs`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VaultInstructionKind {
   CreateUserVault = 0,
   DepositUserVault = 1,
   UpdateUserVaultDelegate = 2,
   WithdrawUserVault = 3,
   WithdrawUserVaultNative = 4,
   AppIx = 5,
   CpiEntry = 6,
   CpiEntryNative = 7,
   CloseVaultAta = 8,
   CloseUserVault = 9,
}

fn require_positive_amount(amount: u64) -> Result<(), VaultSdkError> {
   if amount == 0 {
      Err(VaultSdkError::InvalidAmount)
   } else {
      Ok(())
   }
}

fn require_cpi_entry_amounts(amount_native: u64, amount: u64) -> Result<(), VaultSdkError> {
   if amount_native == 0 && amount == 0 {
      Err(VaultSdkError::InvalidAmount)
   } else {
      Ok(())
   }
}

/// `CreateUserVault` — 5 accounts: owner, user_vault_pda, app, delegate, system. Data: discriminator + `delegate_expires` u32 LE (Unix seconds; `u32::MAX` = no practical expiry).
pub fn create_user_vault_ix(
   program_id: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   app_address: &Pubkey,
   delegate: &Pubkey,
   system_program: Option<&Pubkey>,
   delegate_expires: u32,
) -> Instruction {
   let system = system_program.copied().unwrap_or(SYSTEM_PROGRAM_ID);
   let mut data = Vec::with_capacity(5);
   data.push(VaultInstructionKind::CreateUserVault as u8);
   data.extend_from_slice(&delegate_expires.to_le_bytes());
   Instruction {
      program_id: *program_id,
      accounts: vec![
         AccountMeta::new(*owner, true),
         AccountMeta::new(*user_vault_pda, false),
         AccountMeta::new_readonly(*app_address, false),
         AccountMeta::new_readonly(*delegate, false),
         AccountMeta::new_readonly(system, false),
      ],
      data,
   }
}

/// `DepositUserVault` — 9 accounts; `amount` is little-endian `u64` after discriminator.
pub fn deposit_user_vault_ix(
   program_id: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   user_vault_ata: &Pubkey,
   app_address: &Pubkey,
   source_ata: &Pubkey,
   mint: &Pubkey,
   token_program: &Pubkey,
   amount: u64,
   system_program: Option<&Pubkey>,
   associated_token_program: Option<&Pubkey>,
) -> Result<Instruction, VaultSdkError> {
   require_positive_amount(amount)?;
   let system = system_program.copied().unwrap_or(SYSTEM_PROGRAM_ID);
   let ata_program = associated_token_program.copied().unwrap_or(ASSOCIATED_TOKEN_PROGRAM_ID);
   let mut data = Vec::with_capacity(9);
   data.push(VaultInstructionKind::DepositUserVault as u8);
   data.extend_from_slice(&amount.to_le_bytes());
   Ok(Instruction {
      program_id: *program_id,
      accounts: vec![
         AccountMeta::new(*owner, true),
         AccountMeta::new(*user_vault_pda, false),
         AccountMeta::new(*user_vault_ata, false),
         AccountMeta::new_readonly(*app_address, false),
         AccountMeta::new(*source_ata, false),
         AccountMeta::new_readonly(*mint, false),
         AccountMeta::new_readonly(system, false),
         AccountMeta::new_readonly(*token_program, false),
         AccountMeta::new_readonly(ata_program, false),
      ],
      data,
   })
}

/// `UpdateUserVaultDelegate` — 4 accounts. Data: discriminator + `delegate_expires` u32 LE (Unix seconds; `u32::MAX` = no practical expiry).
pub fn update_user_vault_delegate_ix(
   program_id: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   app_address: &Pubkey,
   delegate: &Pubkey,
   delegate_expires: u32,
) -> Instruction {
   let mut data = Vec::with_capacity(5);
   data.push(VaultInstructionKind::UpdateUserVaultDelegate as u8);
   data.extend_from_slice(&delegate_expires.to_le_bytes());
   Instruction {
      program_id: *program_id,
      accounts: vec![
         AccountMeta::new(*owner, true),
         AccountMeta::new(*user_vault_pda, false),
         AccountMeta::new_readonly(*app_address, false),
         AccountMeta::new_readonly(*delegate, false),
      ],
      data,
   }
}

/// `WithdrawUserVault` — 7 accounts.
pub fn withdraw_user_vault_ix(
   program_id: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   user_vault_ata: &Pubkey,
   app_address: &Pubkey,
   dest_ata: &Pubkey,
   mint: &Pubkey,
   token_program: &Pubkey,
   amount: u64,
) -> Result<Instruction, VaultSdkError> {
   require_positive_amount(amount)?;
   let mut data = Vec::with_capacity(9);
   data.push(VaultInstructionKind::WithdrawUserVault as u8);
   data.extend_from_slice(&amount.to_le_bytes());
   Ok(Instruction {
      program_id: *program_id,
      accounts: vec![
         AccountMeta::new(*owner, true),
         AccountMeta::new_readonly(*user_vault_pda, false),
         AccountMeta::new(*user_vault_ata, false),
         AccountMeta::new_readonly(*app_address, false),
         AccountMeta::new(*dest_ata, false),
         AccountMeta::new_readonly(*mint, false),
         AccountMeta::new_readonly(*token_program, false),
      ],
      data,
   })
}

/// `WithdrawUserVaultNative` — 3 accounts: owner (signer, writable), user_vault_pda (writable), app. Data: discriminator + `amount` u64 LE.
pub fn withdraw_user_vault_native_ix(
   program_id: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   app_address: &Pubkey,
   amount: u64,
) -> Result<Instruction, VaultSdkError> {
   require_positive_amount(amount)?;
   let mut data = Vec::with_capacity(9);
   data.push(VaultInstructionKind::WithdrawUserVaultNative as u8);
   data.extend_from_slice(&amount.to_le_bytes());
   Ok(Instruction {
      program_id: *program_id,
      accounts: vec![
         AccountMeta::new(*owner, true),
         AccountMeta::new(*user_vault_pda, false),
         AccountMeta::new_readonly(*app_address, false),
      ],
      data,
   })
}

/// Fixed accounts for `AppIx` (delegate signer + vault context). Append inner CPI metas after these.
pub struct AppIxFixedAccounts<'a> {
   pub delegate: &'a Pubkey,
   pub owner: &'a Pubkey,
   pub user_vault_pda: &'a Pubkey,
   pub app_address: &'a Pubkey,
}

/// `AppIx` — 4 fixed accounts, then accounts for the inner app instruction. `inner_instruction_data` is passed through to the CPI as-is (no length prefix).
pub fn app_ix_ix(
   program_id: &Pubkey,
   fixed: AppIxFixedAccounts,
   inner_accounts: Vec<AccountMeta>,
   inner_instruction_data: &[u8],
) -> Instruction {
   let mut data = Vec::with_capacity(1 + inner_instruction_data.len());
   data.push(VaultInstructionKind::AppIx as u8);
   data.extend_from_slice(inner_instruction_data);
   let mut accounts = vec![
      AccountMeta::new(*fixed.delegate, true),
      AccountMeta::new_readonly(*fixed.owner, false),
      AccountMeta::new_readonly(*fixed.user_vault_pda, false),
      AccountMeta::new_readonly(*fixed.app_address, false),
   ];
   accounts.extend(inner_accounts);
   Instruction {
      program_id: *program_id,
      accounts,
      data,
   }
}

/// Builds the 11 `AccountMeta` entries for `CpiEntry` (`program/src/instructions/cpi_entry.rs`).
/// Writable flags for vault PDA, vault ATA, lamports destination, and destination ATA follow the amounts you pass in
/// (`amount_native` / `amount` must match the serialized instruction data).
pub fn cpi_entry_account_metas(
   delegate: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   user_vault_ata: &Pubkey,
   app_address: &Pubkey,
   lamports_dest: &Pubkey,
   dest_ata: &Pubkey,
   mint: &Pubkey,
   token_program: &Pubkey,
   amount_native: u64,
   amount: u64,
   instructions_sysvar: Option<&Pubkey>,
   clock_sysvar: Option<&Pubkey>,
) -> Vec<AccountMeta> {
   let w_native = amount_native > 0;
   let w_spl = amount > 0;
   let ixs = instructions_sysvar.copied().unwrap_or(SYSVAR_INSTRUCTIONS_ID);
   let clock = clock_sysvar.copied().unwrap_or(SYSVAR_CLOCK_ID);
   vec![
      AccountMeta::new(*delegate, true),
      AccountMeta::new_readonly(*owner, false),
      if w_native {
         AccountMeta::new(*user_vault_pda, false)
      } else {
         AccountMeta::new_readonly(*user_vault_pda, false)
      },
      if w_spl {
         AccountMeta::new(*user_vault_ata, false)
      } else {
         AccountMeta::new_readonly(*user_vault_ata, false)
      },
      AccountMeta::new_readonly(*app_address, false),
      if w_native {
         AccountMeta::new(*lamports_dest, false)
      } else {
         AccountMeta::new_readonly(*lamports_dest, false)
      },
      if w_spl {
         AccountMeta::new(*dest_ata, false)
      } else {
         AccountMeta::new_readonly(*dest_ata, false)
      },
      AccountMeta::new_readonly(*mint, false),
      AccountMeta::new_readonly(*token_program, false),
      AccountMeta::new_readonly(ixs, false),
      AccountMeta::new_readonly(clock, false),
   ]
}

/// `CpiEntry` (vault CPI path) — data is discriminator + `amount_native` u64 LE + `amount` u64 LE (SPL token amount).
pub fn cpi_entry_ix(
   program_id: &Pubkey,
   delegate: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   user_vault_ata: &Pubkey,
   app_address: &Pubkey,
   lamports_dest: &Pubkey,
   dest_ata: &Pubkey,
   mint: &Pubkey,
   token_program: &Pubkey,
   amount_native: u64,
   amount: u64,
   instructions_sysvar: Option<&Pubkey>,
   clock_sysvar: Option<&Pubkey>,
) -> Result<Instruction, VaultSdkError> {
   require_cpi_entry_amounts(amount_native, amount)?;
   let accounts = cpi_entry_account_metas(
      delegate,
      owner,
      user_vault_pda,
      user_vault_ata,
      app_address,
      lamports_dest,
      dest_ata,
      mint,
      token_program,
      amount_native,
      amount,
      instructions_sysvar,
      clock_sysvar,
   );
   let mut data = Vec::with_capacity(17);
   data.push(VaultInstructionKind::CpiEntry as u8);
   data.extend_from_slice(&amount_native.to_le_bytes());
   data.extend_from_slice(&amount.to_le_bytes());
   Ok(Instruction {
      program_id: *program_id,
      accounts,
      data,
   })
}

/// Seven accounts for `CpiEntryNative` (`program/src/instructions/cpi_entry_native.rs`).
pub fn cpi_entry_native_account_metas(
   delegate: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   app_address: &Pubkey,
   lamports_dest: &Pubkey,
   instructions_sysvar: Option<&Pubkey>,
   clock_sysvar: Option<&Pubkey>,
) -> Vec<AccountMeta> {
   let ixs = instructions_sysvar.copied().unwrap_or(SYSVAR_INSTRUCTIONS_ID);
   let clock = clock_sysvar.copied().unwrap_or(SYSVAR_CLOCK_ID);
   vec![
      AccountMeta::new(*delegate, true),
      AccountMeta::new_readonly(*owner, false),
      AccountMeta::new(*user_vault_pda, false),
      AccountMeta::new_readonly(*app_address, false),
      AccountMeta::new(*lamports_dest, false),
      AccountMeta::new_readonly(ixs, false),
      AccountMeta::new_readonly(clock, false),
   ]
}

/// `CpiEntryNative` (vault CPI path) — data is discriminator + `amount_native` u64 LE.
pub fn cpi_entry_native_ix(
   program_id: &Pubkey,
   delegate: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   app_address: &Pubkey,
   lamports_dest: &Pubkey,
   amount_native: u64,
   instructions_sysvar: Option<&Pubkey>,
   clock_sysvar: Option<&Pubkey>,
) -> Result<Instruction, VaultSdkError> {
   require_positive_amount(amount_native)?;
   let accounts = cpi_entry_native_account_metas(
      delegate,
      owner,
      user_vault_pda,
      app_address,
      lamports_dest,
      instructions_sysvar,
      clock_sysvar,
   );
   let mut data = Vec::with_capacity(9);
   data.push(VaultInstructionKind::CpiEntryNative as u8);
   data.extend_from_slice(&amount_native.to_le_bytes());
   Ok(Instruction {
      program_id: *program_id,
      accounts,
      data,
   })
}

/// `CloseVaultAta` — 7 accounts.
pub fn close_vault_ata_ix(
   program_id: &Pubkey,
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   app_address: &Pubkey,
   user_vault_ata: &Pubkey,
   destination: &Pubkey,
   mint: &Pubkey,
   token_program: &Pubkey,
) -> Instruction {
   Instruction {
      program_id: *program_id,
      accounts: vec![
         AccountMeta::new(*owner, true),
         AccountMeta::new(*user_vault_pda, false),
         AccountMeta::new_readonly(*app_address, false),
         AccountMeta::new(*user_vault_ata, false),
         AccountMeta::new(*destination, false),
         AccountMeta::new_readonly(*mint, false),
         AccountMeta::new_readonly(*token_program, false),
      ],
      data: vec![VaultInstructionKind::CloseVaultAta as u8],
   }
}

/// `CloseUserVault` — 3 accounts.
pub fn close_user_vault_ix(program_id: &Pubkey, owner: &Pubkey, user_vault_pda: &Pubkey, app_address: &Pubkey) -> Instruction {
   Instruction {
      program_id: *program_id,
      accounts: vec![
         AccountMeta::new(*owner, true),
         AccountMeta::new(*user_vault_pda, false),
         AccountMeta::new_readonly(*app_address, false),
      ],
      data: vec![VaultInstructionKind::CloseUserVault as u8],
   }
}

// --- Convenience wrappers using [`DEFAULT_VAULT_PROGRAM_ID`] ---

/// Same as [`create_user_vault_ix`] with `program_id` = [`DEFAULT_VAULT_PROGRAM_ID`].
pub fn create_user_vault_ix_default(
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   app_address: &Pubkey,
   delegate: &Pubkey,
   system_program: Option<&Pubkey>,
   delegate_expires: u32,
) -> Instruction {
   create_user_vault_ix(
      &DEFAULT_VAULT_PROGRAM_ID,
      owner,
      user_vault_pda,
      app_address,
      delegate,
      system_program,
      delegate_expires,
   )
}

/// Same as [`deposit_user_vault_ix`] with `program_id` = [`DEFAULT_VAULT_PROGRAM_ID`].
pub fn deposit_user_vault_ix_default(
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   user_vault_ata: &Pubkey,
   app_address: &Pubkey,
   source_ata: &Pubkey,
   mint: &Pubkey,
   token_program: &Pubkey,
   amount: u64,
   system_program: Option<&Pubkey>,
   associated_token_program: Option<&Pubkey>,
) -> Result<Instruction, VaultSdkError> {
   deposit_user_vault_ix(
      &DEFAULT_VAULT_PROGRAM_ID,
      owner,
      user_vault_pda,
      user_vault_ata,
      app_address,
      source_ata,
      mint,
      token_program,
      amount,
      system_program,
      associated_token_program,
   )
}

/// Uses classic SPL Token as `token_program` for [`deposit_user_vault_ix_default`].
pub fn deposit_user_vault_ix_default_spl_token(
   owner: &Pubkey,
   user_vault_pda: &Pubkey,
   user_vault_ata: &Pubkey,
   app_address: &Pubkey,
   source_ata: &Pubkey,
   mint: &Pubkey,
   amount: u64,
   system_program: Option<&Pubkey>,
   associated_token_program: Option<&Pubkey>,
) -> Result<Instruction, VaultSdkError> {
   deposit_user_vault_ix_default(
      owner,
      user_vault_pda,
      user_vault_ata,
      app_address,
      source_ata,
      mint,
      &SPL_TOKEN_PROGRAM_ID,
      amount,
      system_program,
      associated_token_program,
   )
}
