//! Mollusk integration tests (app specific delegated vaults + `test_program` BPF). Enable with `--features test-sbf`.
//!
//! Build artifacts:
//! - `cargo build-sbf` in `program/` → `target/deploy/app_specific_delegated_vaults.so`
//! - `cargo build-sbf` in `test_program/` → `../test_program/target/deploy/test_program.so`
//!
//! Run (integration tests only, quiet harness): `cargo vault-test` or `scripts/run-vault-tests.ps1` from `program/`.
//!
//! ## Compute units (optimization)
//!
//! - Run a subset: `cargo vault-test create_user_vault::` or a single test name (substring filter).
//! - Use `--nocapture` so `eprintln!` output is visible: `cargo test -p app-specific-delegated-vaults --features test-sbf --test mollusk_vault -- --nocapture FILTER`.
//! - After `process_and_validate_instruction`, the returned `InstructionResult` includes
//!   `compute_units_consumed`. Call `crate::common::log_cu("label", &result)` or read that field.
//! - `compute_units_consumed` includes **every program** invoked in that instruction (vault + SPL CPIs). Pinning
//!   exact CU in `Check::compute_units(...)` is brittle when dependencies change; prefer logging while tuning.
//! - Mollusk’s crate README describes a compute-unit bencher / fixtures for regression tracking.
//! - Full CU table for every `log_cu` step: set `VAULT_CU_REPORT` to a path (see `common::log_cu`) or run
//!   `.\scripts\generate-cu-report.ps1` from `program/` to write `tests/cu-report.tsv` and
//!   `tests/cu-report-summary.tsv`. `log_cu` is only used on instructions inside `#[test]` bodies (not shared fixture helpers).

use std::sync::Once;

static INTEGRATION_TEST_ENV: Once = Once::new();

/// Unless `RUST_LOG` is already set, cap Solana / Mollusk runtime logging (otherwise tests flood stderr with DEBUG lines).
fn ensure_integration_test_env() {
   INTEGRATION_TEST_ENV.call_once(|| {
      if std::env::var_os("RUST_LOG").is_none() {
         std::env::set_var("RUST_LOG", "error");
      }
   });
}

mod common;

/// Each file under `tests/<name>.rs` becomes a submodule without `#[path = "..."]` boilerplate.
macro_rules! test_module {
   ($name:ident) => {
      mod $name {
         include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/", stringify!($name), ".rs"));
      }
   };
}

test_module!(instruction_router);
test_module!(create_user_vault);
test_module!(deposit_user_vault);
test_module!(update_user_vault_delegate);
test_module!(withdraw_user_vault);
test_module!(withdraw_user_vault_native);
test_module!(app_ix);
test_module!(cpi_entry);
test_module!(cpi_entry_native);
test_module!(close_vault_ata);
test_module!(close_user_vault);
test_module!(delegate_expiry);
test_module!(error_paths);
