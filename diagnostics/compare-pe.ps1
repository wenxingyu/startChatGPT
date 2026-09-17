$ErrorActionPreference = 'Stop'

function Inspect-PE([string]$Path) {
    $bytes = [IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $Path).Path)
    $pe = [BitConverter]::ToInt32($bytes, 0x3c)
    if ([BitConverter]::ToUInt32($bytes, $pe) -ne 0x4550) { throw 'Invalid PE signature' }
    $sectionCount = [BitConverter]::ToUInt16($bytes, $pe + 6)
    $optionalSize = [BitConverter]::ToUInt16($bytes, $pe + 20)
    $sections = @()
    for ($index = 0; $index -lt $sectionCount; $index++) {
        $offset = $pe + 24 + $optionalSize + 40 * $index
        $name = [Text.Encoding]::ASCII.GetString($bytes, $offset, 8).Trim([char]0)
        $rawSize = [BitConverter]::ToUInt32($bytes, $offset + 16)
        $rawOffset = [BitConverter]::ToUInt32($bytes, $offset + 20)
        $sha = [Security.Cryptography.SHA256]::Create()
        try { $hash = [Convert]::ToHexString($sha.ComputeHash($bytes, $rawOffset, $rawSize)) }
        finally { $sha.Dispose() }
        $sections += [pscustomobject]@{ Name=$name; Size=$rawSize; Offset=$rawOffset; SHA256=$hash }
    }
    [pscustomobject]@{
        Path=$Path
        SHA256=(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
        Size=$bytes.Length
        COFFTimestamp=[BitConverter]::ToUInt32($bytes, $pe + 8)
        Sections=$sections
    }
}

$published = Inspect-PE 'diagnostics/published/startChatGPT.exe'
$rebuilt = Inspect-PE 'diagnostics/controlled/complete-1.2.6/startChatGPT.exe'
$comparison = foreach ($section in $published.Sections) {
    $other = $rebuilt.Sections | Where-Object Name -eq $section.Name
    [pscustomobject]@{
        Section=$section.Name
        Identical=($section.SHA256 -eq $other.SHA256)
        PublishedSize=$section.Size
        RebuiltSize=$other.Size
        PublishedSHA256=$section.SHA256
        RebuiltSHA256=$other.SHA256
    }
}
New-Item -ItemType Directory -Path diagnostics/pe-output -Force | Out-Null
@{ Published=$published; Rebuilt=$rebuilt; Comparison=@($comparison) } |
    ConvertTo-Json -Depth 8 | Set-Content diagnostics/pe-output/sections.json
$comparison | Format-Table -AutoSize

function Debug-Records([byte[]]$Bytes) {
    $pe = [BitConverter]::ToInt32($Bytes, 0x3c)
    $optional = $pe + 24
    if ([BitConverter]::ToUInt16($Bytes, $optional) -ne 0x20b) { throw 'Expected PE32+' }
    $directoryRva = [BitConverter]::ToUInt32($Bytes, $optional + 112 + 6 * 8)
    $directorySize = [BitConverter]::ToUInt32($Bytes, $optional + 116 + 6 * 8)
    $sectionCount = [BitConverter]::ToUInt16($Bytes, $pe + 6)
    $optionalSize = [BitConverter]::ToUInt16($Bytes, $pe + 20)
    for ($index = 0; $index -lt $sectionCount; $index++) {
        $section = $pe + 24 + $optionalSize + 40 * $index
        $virtualAddress = [BitConverter]::ToUInt32($Bytes, $section + 12)
        $rawSize = [BitConverter]::ToUInt32($Bytes, $section + 16)
        if ($directoryRva -ge $virtualAddress -and $directoryRva -lt $virtualAddress + $rawSize) {
            $raw = [BitConverter]::ToUInt32($Bytes, $section + 20) + $directoryRva - $virtualAddress
            for ($offset = $raw; $offset -lt $raw + $directorySize; $offset += 28) {
                [pscustomobject]@{
                    DirectoryOffset=$offset
                    Timestamp=[BitConverter]::ToUInt32($Bytes, $offset + 4)
                    Type=[BitConverter]::ToUInt32($Bytes, $offset + 12)
                    DataSize=[BitConverter]::ToUInt32($Bytes, $offset + 16)
                    DataOffset=[BitConverter]::ToUInt32($Bytes, $offset + 24)
                }
            }
        }
    }
}

$left = [IO.File]::ReadAllBytes((Resolve-Path $published.Path).Path)
$right = [IO.File]::ReadAllBytes((Resolve-Path $rebuilt.Path).Path)
if ($left.Length -ne $right.Length) { throw 'Different binary lengths' }
$ranges = @()
$offset = 0
while ($offset -lt $left.Length) {
    if ($left[$offset] -eq $right[$offset]) { $offset++; continue }
    $start = $offset
    while ($offset -lt $left.Length -and $left[$offset] -ne $right[$offset]) { $offset++ }
    $length = $offset - $start
    $ranges += [pscustomobject]@{
        Offset=$start
        Length=$length
        PublishedHex=[Convert]::ToHexString($left, $start, [Math]::Min($length, 64))
        RebuiltHex=[Convert]::ToHexString($right, $start, [Math]::Min($length, 64))
    }
}
@{
    DifferenceRanges=$ranges
    PublishedDebug=@(Debug-Records $left)
    RebuiltDebug=@(Debug-Records $right)
    COFFTimestampOffset=([BitConverter]::ToInt32($left, 0x3c) + 8)
} | ConvertTo-Json -Depth 6 | Set-Content diagnostics/pe-output/differences.json
$ranges | Format-Table -AutoSize
