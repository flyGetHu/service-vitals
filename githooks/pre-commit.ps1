#!/usr/bin/env pwsh
<#!
.SYNOPSIS
    Git pre-commit hook for Windows PowerShell.
.DESCRIPTION
    Executes `cargo fmt` and `cargo clippy` with warnings treated as errors.
    If any check fails, the commit is aborted.
#>

Write-Host "`n[Git Hook] Running cargo fmt --all -- --check" -ForegroundColor Blue
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) {
    Write-Host "[Git Hook] ✗ 代码格式检查未通过，请运行 'cargo fmt --all' 修复格式。" -ForegroundColor Red
    exit 1
}

Write-Host "[Git Hook] Running cargo clippy" -ForegroundColor Blue
cargo clippy --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) {
    Write-Host "[Git Hook] ✗ Clippy 检查未通过，请修复警告后再提交。" -ForegroundColor Red
    exit 1
}

Write-Host "[Git Hook] ✓ 所有检查通过，允许提交。" -ForegroundColor Green
exit 0 