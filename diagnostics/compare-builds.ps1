$ErrorActionPreference = 'Stop'

function Read-RevisionFile([string]$Revision, [string]$Path) {
    $text = & git show "${Revision}:$Path"
    if ($LASTEXITCODE -ne 0) { throw "Cannot read ${Revision}:$Path" }
    return ($text -join "`n") + "`n"
}

function Replace-Required([string]$Text, [string]$Before, [string]$After) {
    if (-not $Text.Contains($Before)) { throw "Missing source fragment: $Before" }
    return $Text.Replace($Before, $After)
}

$original = Read-RevisionFile 'v1.2.5' 'src/usage.rs'
$complete = Read-RevisionFile 'v1.2.6' 'src/usage.rs'
$manifest = Read-RevisionFile 'v1.2.6' 'Cargo.toml'
$lockfile = Read-RevisionFile 'v1.2.6' 'Cargo.lock'

$dropOnly = Replace-Required $original 'self.reader.take()' 'self.reader.take().filter(|reader| reader.is_finished())'

$cleanupOnly = Replace-Required $original "            })();`n            {" "            })();`n            let failed = result.is_err();`n            {"
$cleanupOnly = Replace-Required $cleanupOnly "                        bridge = None;`n" ''
$cleanupOnly = Replace-Required $cleanupOnly '            match rx.recv_timeout(Duration::from_secs(45)) {' "            if failed { bridge = None; }`n            match rx.recv_timeout(Duration::from_secs(45)) {"

$refactorOnly = Replace-Required $complete 'self.reader.take().filter(|reader| reader.is_finished())' 'self.reader.take()'
$refactorOnly = Replace-Required $refactorOnly "            let failed = result.is_err();`n" ''
$refactorOnly = Replace-Required $refactorOnly '                        state.error = Some(error);' "                        state.error = Some(error);`n                        bridge = None;"
$refactorOnly = Replace-Required $refactorOnly "            if failed {`n                bridge = None;`n            }`n" ''

$variants = @(
    @{ Name = 'baseline-1.2.5'; Version = '1.2.5'; Source = $original; Change = 'Original release source' },
    @{ Name = 'version-only'; Version = '1.2.6'; Source = $original; Change = 'Only package version metadata' },
    @{ Name = 'drop-only'; Version = '1.2.6'; Source = $dropOnly; Change = 'Bridge::drop skips waiting for unfinished reader' },
    @{ Name = 'cleanup-only'; Version = '1.2.6'; Source = $cleanupOnly; Change = 'Move failed bridge cleanup outside UI state lock' },
    @{ Name = 'refactor-only'; Version = '1.2.6'; Source = $refactorOnly; Change = 'Extract start_with_connector without either cleanup fix' },
    @{ Name = 'complete-1.2.6'; Version = '1.2.6'; Source = $complete; Change = 'All release changes' }
)

$outputRoot = Join-Path $PSScriptRoot 'output'
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
@{
    Rust = (& rustc --version --verbose) -join "`n"
    RunnerImage = $env:ImageOS
    RunnerImageVersion = $env:ImageVersion
    Commit = (& git rev-parse HEAD)
} | ConvertTo-Json | Set-Content (Join-Path $outputRoot 'environment.json')

$results = @()
foreach ($variant in $variants) {
    Write-Host "Building $($variant.Name): $($variant.Change)"
    # Same directory, dependencies, compiler flags and output name for every build.
    [IO.File]::WriteAllText((Join-Path (Get-Location) 'src/usage.rs'), $variant.Source)
    [IO.File]::WriteAllText((Join-Path (Get-Location) 'Cargo.toml'), $manifest.Replace('version = "1.2.6"', ('version = "' + $variant.Version + '"')))
    $oldPackage = "name = `"start-chat-gpt`"`nversion = `"1.2.6`""
    $newPackage = "name = `"start-chat-gpt`"`nversion = `"$($variant.Version)`""
    [IO.File]::WriteAllText((Join-Path (Get-Location) 'Cargo.lock'), (Replace-Required $lockfile $oldPackage $newPackage))
    & cargo build --release --locked
    if ($LASTEXITCODE -ne 0) { throw "Build failed: $($variant.Name)" }
    $destination = Join-Path $outputRoot $variant.Name
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    Copy-Item -LiteralPath 'target/release/startChatGPT.exe' -Destination $destination
    Copy-Item -LiteralPath 'src/usage.rs', 'Cargo.toml', 'Cargo.lock' -Destination $destination
    $exe = Join-Path $destination 'startChatGPT.exe'
    $results += [pscustomobject]@{
        Variant = $variant.Name
        Change = $variant.Change
        SHA256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash
        Size = (Get-Item -LiteralPath $exe).Length
        Version = (Get-Item -LiteralPath $exe).VersionInfo.FileVersion
    }
}
$results | ConvertTo-Json | Set-Content (Join-Path $outputRoot 'builds.json')
$results | Format-Table -AutoSize
