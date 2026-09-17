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
