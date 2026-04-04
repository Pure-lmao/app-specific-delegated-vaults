#Requires -Version 5.0
<#
.SYNOPSIS
   Run vault Mollusk + BPF integration tests with CI-friendly defaults.

.DESCRIPTION
   Sets RUST_LOG=error when unset (suppresses Solana runtime DEBUG spam).
   Run from repo root or anywhere; script cds to program/.

   Prerequisites: cargo build-sbf in program/ and ../test_program/ (produces .so under target/deploy).

.EXAMPLE
   ./scripts/run-vault-tests.ps1
   ./scripts/run-vault-tests.ps1 error_paths::deposit_fails_invalid_token_program
#>
param(
   [Parameter(ValueFromRemainingArguments = $true)]
   [string[]]$CargoArgs = @()
)
$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
if (-not $env:RUST_LOG) {
   $env:RUST_LOG = "error"
}
cargo test -p app-specific-delegated-vaults --features test-sbf --test mollusk_vault @CargoArgs
