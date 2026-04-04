#!/usr/bin/env bash
# Run vault Mollusk + BPF integration tests with CI-friendly defaults.
# Prerequisites: cargo build-sbf in program/ and ../test_program/
set -euo pipefail
cd "$(dirname "$0")/.."
export RUST_LOG="${RUST_LOG:-error}"
exec cargo test -p app-specific-delegated-vaults --features test-sbf --test mollusk_vault "$@"
