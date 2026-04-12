# Regenerate compute-unit reports for all mollusk_vault integration tests.
# Requires: cargo build-sbf in program/ and test_program/, run from program/.
#
#   .\scripts\generate-cu-report.ps1
#
# Writes:
#   tests/cu-success.tsv          — **start here**: happy-path CUs only, one row per major ix (see $PrimaryBench)
#   tests/cu-report.tsv           — full label<TAB>compute_units (every log_cu_bench / split row)
#   tests/cu-report-summary.tsv   — module::test_fn<TAB>max_cu per test

$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $PSScriptRoot
Set-Location $here

$raw = Join-Path $here "tests/cu-report.tsv"
$summaryPath = Join-Path $here "tests/cu-report-summary.tsv"
$tmp = Join-Path $here "tests/cu-report.tmp.tsv"

if (Test-Path $raw) { Remove-Item -Force $raw }
if (Test-Path $tmp) { Remove-Item -Force $tmp }

$env:VAULT_CU_REPORT = $tmp
if (-not $env:RUST_LOG) { $env:RUST_LOG = "error" }

cargo test --features test-sbf --test mollusk_vault
$code = $LASTEXITCODE
Remove-Item Env:VAULT_CU_REPORT -ErrorAction SilentlyContinue
if ($code -ne 0) { exit $code }

function Get-TestKey([string]$label) {
   $idx = $label.LastIndexOf("::")
   if ($idx -lt 0) { return $label }
   $after = $label.Substring($idx + 2)
   $c = $after.IndexOf([char]':')
   if ($c -ge 0) {
      return $label.Substring(0, $idx + 2 + $c)
   }
   return $label
}

$dataLines = @()
if (Test-Path $tmp) {
   $dataLines = Get-Content -LiteralPath $tmp
   Remove-Item -Force $tmp
}

$maxPerTest = @{}
foreach ($line in $dataLines) {
   $parts = $line -split "`t", 2
   if ($parts.Count -lt 2) { continue }
   $label = $parts[0].Trim()
   $cu = [int64]$parts[1].Trim()
   $key = Get-TestKey $label
   if (-not $maxPerTest.ContainsKey($key) -or $cu -gt $maxPerTest[$key]) {
      $maxPerTest[$key] = $cu
   }
}

$stamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss zzz"
@"
# Mollusk compute_units_consumed (simulated instruction + all CPIs in that step). Setup steps use log_cu_setup and are omitted.
# Generated: $stamp
# Regenerate: .\scripts\generate-cu-report.ps1
label	compute_units
"@ | Set-Content -LiteralPath $raw -Encoding utf8

$dataLines | Sort-Object | Add-Content -LiteralPath $raw -Encoding utf8

@"
# Max compute_units_consumed per integration test (max over log_cu_bench / split rows for that test).
# Generated: $stamp
test	max_compute_units
"@ | Set-Content -LiteralPath $summaryPath -Encoding utf8

$maxPerTest.GetEnumerator() | Sort-Object Name | ForEach-Object {
   "{0}`t{1}" -f $_.Key, $_.Value
} | Add-Content -LiteralPath $summaryPath -Encoding utf8

# --- Happy-path file: one CU row per major vault instruction (success tests only) ---
$successPath = Join-Path $here "tests/cu-success.tsv"
$byLabel = @{}
foreach ($line in $dataLines) {
   $parts = $line -split "`t", 2
   if ($parts.Count -lt 2) { continue }
   $byLabel[$parts[0].Trim()] = $parts[1].Trim()
}

# Exact labels from tests/*.rs (change here if test names change)
$PrimaryBench = @(
   @{ op = "create_user_vault"; label = "create_user_vault::create_success_with_max_delegate_expires" },
   @{ op = "deposit_user_vault"; label = "deposit_user_vault::deposit_success_first_creates_ata:deposit" },
   @{ op = "withdraw_user_vault"; label = "withdraw_user_vault::withdraw_success" },
   @{ op = "withdraw_user_vault_native"; label = "withdraw_user_vault_native::withdraw_native_success" },
   @{ op = "app_ix"; label = "app_ix::app_ix_success_deposit_from_user:bench_total" },
   @{ op = "cpi_entry"; label = "cpi_entry::cpi_entry_success_spl_only_via_test_program:bench_total" },
   @{ op = "cpi_entry_native"; label = "cpi_entry_native::cpi_entry_native_success:bench_total" },
   @{ op = "close_vault_ata"; label = "close_vault_ata::close_vault_ata_success" },
   @{ op = "close_user_vault"; label = "close_user_vault::close_user_vault_success:close_vault" }
)

$successHeader = @"
# Happy-path compute units only (Mollusk: one simulated instruction + all CPIs in that step).
# Regenerate: .\scripts\generate-cu-report.ps1
# Generated: $stamp
operation	compute_units
"@
$successHeader | Set-Content -LiteralPath $successPath -Encoding utf8

foreach ($row in $PrimaryBench) {
   $cu = $byLabel[$row.label]
   if ($null -eq $cu) {
      Write-Warning "cu-success: missing label $($row.label); update generate-cu-report.ps1 or tests"
      $cu = ""
   }
   "{0}`t{1}" -f $row.op, $cu | Add-Content -LiteralPath $successPath -Encoding utf8
}

$nPrimary = $PrimaryBench.Count
Write-Host "Wrote $successPath - $nPrimary happy-path rows"
Write-Host "Wrote $raw - $($dataLines.Count) instruction rows"
Write-Host "Wrote $summaryPath - $($maxPerTest.Count) tests"
