//! Bench-only no-ops for integration-test compute attribution (vault tests).
//!
//! * Discriminator **6** (`BenchNoopInner`) — inner ix for vault `app_ix` with **zero** inner accounts.
//! * Discriminator **7** (`BenchNoopTopLevel`) — top-level ix with **no** accounts (minimal caller frame).

use pinocchio::{AccountView, Address, ProgramResult};

#[inline(always)]
pub fn process(_program_id: &Address, _accounts: &mut [AccountView], _data: &[u8]) -> ProgramResult {
   Ok(())
}
