# Regenerate compute-unit reports for all mollusk_vault integration tests.
# Requires: cargo build-sbf in program/ and test_program/, run from program/.
#
#   .\scripts\generate-cu-report.ps1
#
# Writes:
#   tests/cu-report.tsv           — label<TAB>compute_units (one row per log_cu)
#   tests/cu-report-summary.tsv — module::test_fn<TAB>max_cu (only rows from log_cu inside #[test], not fixture helpers)

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
# Mollusk compute_units_consumed (simulated instruction + all CPIs in that step).
# Generated: $stamp
# Regenerate: .\scripts\generate-cu-report.ps1
label	compute_units
"@ | Set-Content -LiteralPath $raw -Encoding utf8

$dataLines | Sort-Object | Add-Content -LiteralPath $raw -Encoding utf8

@"
# Max compute_units_consumed per integration test (max over log_cu steps in that test).
# Generated: $stamp
test	max_compute_units
"@ | Set-Content -LiteralPath $summaryPath -Encoding utf8

$maxPerTest.GetEnumerator() | Sort-Object Name | ForEach-Object {
   "{0}`t{1}" -f $_.Key, $_.Value
} | Add-Content -LiteralPath $summaryPath -Encoding utf8

Write-Host "Wrote $raw ($($dataLines.Count) instruction rows)"
Write-Host "Wrote $summaryPath ($($maxPerTest.Count) tests)"
