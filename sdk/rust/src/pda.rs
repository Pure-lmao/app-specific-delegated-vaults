//! Program-derived addresses for user vault state and vault ATA.

use solana_pubkey::Pubkey;

use crate::constants::{ASSOCIATED_TOKEN_PROGRAM_ID, USER_VAULT_SEED};

/// `["vault", owner, app_address]` PDA under `program_id`.
pub fn find_user_vault_pda(program_id: &Pubkey, owner: &Pubkey, app_address: &Pubkey) -> (Pubkey, u8) {
   Pubkey::find_program_address(&[USER_VAULT_SEED, owner.as_ref(), app_address.as_ref()], program_id)
}

/// SPL associated token account for `user_vault_pda` + `mint` (classic or Token-2022 via `token_program`).
///
/// Same PDA as `get_associated_token_address_with_program_id`: seeds `[owner, token_program_id, mint]`
/// under the associated token program.
pub fn find_user_vault_ata(user_vault_pda: &Pubkey, mint: &Pubkey, token_program: &Pubkey) -> Pubkey {
   let (ata, _) = Pubkey::find_program_address(
      &[user_vault_pda.as_ref(), token_program.as_ref(), mint.as_ref()],
      &ASSOCIATED_TOKEN_PROGRAM_ID,
   );
   ata
}
