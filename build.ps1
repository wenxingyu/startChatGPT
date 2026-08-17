$ErrorActionPreference = "Stop"
$cargo = Join-Path $env:USERPROFILE ".cargo\bin\cargo.exe"

& $cargo test --release
if ($LASTEXITCODE -ne 0) {
    throw "Rust tests failed with exit code $LASTEXITCODE"
}

& $cargo build --release
if ($LASTEXITCODE -ne 0) {
    throw "Rust build failed with exit code $LASTEXITCODE"
}

$source = Join-Path $PSScriptRoot "target\release\startChatGPT.exe"
$destination = Join-Path $PSScriptRoot "startChatGPT.exe"
Copy-Item -LiteralPath $source -Destination $destination -Force

$file = Get-Item -LiteralPath $destination
Write-Host "Built $($file.FullName) ($($file.Length) bytes)"
