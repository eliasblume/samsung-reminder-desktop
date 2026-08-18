[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidatePattern('^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$')]
    [string]$Version,

    [Parameter(Mandatory = $true)]
    [ValidatePattern('^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$')]
    [string]$Repository,

    [Parameter(Mandatory = $true)]
    [ValidateSet('x64', 'arm64')]
    [string]$Architecture,

    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
    [string]$OutputDirectory = (Join-Path $Root 'artifacts')
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-File {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Required release file is missing: $Path"
    }
}

function Assert-PeExecutable {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][uint16]$ExpectedMachine,
        [Parameter(Mandatory = $true)][uint16]$ExpectedSubsystem,
        [Parameter(Mandatory = $true)][string]$Label
    )

    $bytes = [IO.File]::ReadAllBytes($Path)
    if ($bytes.Length -lt 256 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) {
        throw "$Label is not a valid PE executable: $Path"
    }
    $peOffset = [BitConverter]::ToInt32($bytes, 0x3c)
    if ($peOffset -lt 0 -or $peOffset + 94 -ge $bytes.Length -or
        $bytes[$peOffset] -ne 0x50 -or $bytes[$peOffset + 1] -ne 0x45 -or
        $bytes[$peOffset + 2] -ne 0 -or $bytes[$peOffset + 3] -ne 0) {
        throw "$Label has an invalid PE header: $Path"
    }

    $machine = [BitConverter]::ToUInt16($bytes, $peOffset + 4)
    $subsystem = [BitConverter]::ToUInt16($bytes, $peOffset + 24 + 68)
    if ($machine -ne $ExpectedMachine) {
        throw ('{0} uses PE machine 0x{1:X4}; expected 0x{2:X4}.' -f $Label, $machine, $ExpectedMachine)
    }
    if ($subsystem -ne $ExpectedSubsystem) {
        throw "$Label uses PE subsystem $subsystem; expected $ExpectedSubsystem."
    }
}

function Copy-SingleBundle {
    param(
        [Parameter(Mandatory = $true)][string]$Directory,
        [Parameter(Mandatory = $true)][string]$Filter,
        [Parameter(Mandatory = $true)][string]$Destination
    )

    $matches = @(Get-ChildItem -LiteralPath $Directory -Filter $Filter -File)
    if ($matches.Count -ne 1) {
        throw "Expected exactly one '$Filter' bundle in $Directory, found $($matches.Count)."
    }
    Copy-Item -LiteralPath $matches[0].FullName -Destination $Destination
}

function New-ReleaseArchive {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][System.Collections.IDictionary]$Files,
        [Parameter(Mandatory = $true)][string]$Directory
    )

    $stagingDirectory = Join-Path $Directory $Name
    New-Item -ItemType Directory -Path $stagingDirectory | Out-Null
    foreach ($entry in $Files.GetEnumerator()) {
        Copy-Item -LiteralPath $entry.Value -Destination (Join-Path $stagingDirectory $entry.Key)
    }

    $archive = Join-Path $Directory "$Name.zip"
    Compress-Archive -Path (Join-Path $stagingDirectory '*') -DestinationPath $archive -CompressionLevel Optimal
    Remove-Item -LiteralPath $stagingDirectory -Recurse -Force

    $hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText(
        "$archive.sha256",
        "$hash  $([IO.Path]::GetFileName($archive))`n",
        [Text.UTF8Encoding]::new($false)
    )

    return [pscustomobject]@{
        Name = $Name
        Hash = $hash
    }
}

function Write-ScoopFragment {
    param(
        [Parameter(Mandatory = $true)][ValidateSet('desktop', 'mcp')][string]$Package,
        [Parameter(Mandatory = $true)][pscustomobject]$Archive,
        [Parameter(Mandatory = $true)][string]$Directory
    )

    $scoopArchitecture = if ($Architecture -eq 'x64') { '64bit' } else { 'arm64' }
    $downloadUrl = "https://github.com/$Repository/releases/download/v$Version/$($Archive.Name).zip"
    $autoupdateName = $Archive.Name.Replace($Version, '$version')
    $autoupdateUrl = "https://github.com/$Repository/releases/download/v`$version/$autoupdateName.zip"
    $notes = @(
        'Requires Samsung Browser for Windows installed in its standard location.'
        'Download Samsung Browser from https://browser.samsung.com/ and sign in before the first sync.'
    )
    if ($Package -eq 'desktop') {
        $notes += 'Upgrading from v0.1.1 or older? Update samsung-reminder before installing samsung-reminder-mcp so Scoop removes the old bundled shim first.'
    }

    $manifest = [ordered]@{
        version = $Version
        description = if ($Package -eq 'desktop') {
            'Unofficial Samsung Reminder desktop client'
        }
        else {
            'Local MCP server for Samsung Reminder'
        }
        homepage = "https://github.com/$Repository"
        license = 'MIT'
        notes = $notes
        architecture = [ordered]@{
            $scoopArchitecture = [ordered]@{
                url = $downloadUrl
                hash = $Archive.Hash
            }
        }
    }

    if ($Package -eq 'desktop') {
        $manifest.shortcuts = @(, @('Reminder.exe', 'Reminder'))
    }
    else {
        $manifest.bin = @(, @('samsung-reminder-mcp.exe', 'samsung-reminder-mcp'))
    }

    $manifest.checkver = 'github'
    $manifest.autoupdate = [ordered]@{
        architecture = [ordered]@{
            $scoopArchitecture = [ordered]@{
                url = $autoupdateUrl
            }
        }
    }

    $fragmentName = if ($Package -eq 'desktop') {
        "samsung-reminder-$Architecture.json"
    }
    else {
        "samsung-reminder-mcp-$Architecture.json"
    }
    $manifestPath = Join-Path $Directory $fragmentName
    $manifestJson = $manifest | ConvertTo-Json -Depth 10
    [IO.File]::WriteAllText($manifestPath, "$manifestJson`n", [Text.UTF8Encoding]::new($false))
}

$rootPath = [IO.Path]::GetFullPath($Root).TrimEnd('\')
$outputPath = [IO.Path]::GetFullPath($OutputDirectory)
if (-not $outputPath.StartsWith("$rootPath\", [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The output directory must be inside the repository root.'
}

$releaseDirectory = Join-Path $rootPath 'src-tauri\target\release'
$appExecutable = Join-Path $releaseDirectory 'samsung-reminder-desktop.exe'
$mcpExecutable = Join-Path $releaseDirectory 'samsung-reminder-mcp.exe'
$licensePath = Join-Path $rootPath 'LICENSE'
$readmePath = Join-Path $rootPath 'README.md'
$legalPath = Join-Path $rootPath 'LEGAL.md'
$noticesPath = Join-Path $rootPath 'THIRD_PARTY_NOTICES.md'

Assert-File -Path $appExecutable
Assert-File -Path $mcpExecutable
Assert-File -Path $licensePath
Assert-File -Path $readmePath
Assert-File -Path $legalPath
Assert-File -Path $noticesPath

$expectedMachine = if ($Architecture -eq 'x64') { [uint16]0x8664 } else { [uint16]0xaa64 }
Assert-PeExecutable -Path $appExecutable -ExpectedMachine $expectedMachine -ExpectedSubsystem 2 -Label 'Reminder GUI'
Assert-PeExecutable -Path $mcpExecutable -ExpectedMachine $expectedMachine -ExpectedSubsystem 3 -Label 'Reminder MCP server'

if (Test-Path -LiteralPath $outputPath) {
    Remove-Item -LiteralPath $outputPath -Recurse -Force
}
New-Item -ItemType Directory -Path $outputPath | Out-Null

$desktopArchive = New-ReleaseArchive `
    -Name "Samsung-Reminder-$Version-windows-$Architecture" `
    -Directory $outputPath `
    -Files ([ordered]@{
        'Reminder.exe' = $appExecutable
        'LICENSE' = $licensePath
        'README.md' = $readmePath
        'LEGAL.md' = $legalPath
        'THIRD_PARTY_NOTICES.md' = $noticesPath
    })

$mcpArchive = New-ReleaseArchive `
    -Name "Samsung-Reminder-MCP-$Version-windows-$Architecture" `
    -Directory $outputPath `
    -Files ([ordered]@{
        'samsung-reminder-mcp.exe' = $mcpExecutable
        'LICENSE' = $licensePath
        'README.md' = $readmePath
        'LEGAL.md' = $legalPath
        'THIRD_PARTY_NOTICES.md' = $noticesPath
    })

$nsisDestination = Join-Path $outputPath "Samsung-Reminder-$Version-windows-$Architecture-setup.exe"
$msiDestination = Join-Path $outputPath "Samsung-Reminder-$Version-windows-$Architecture.msi"
Copy-SingleBundle -Directory (Join-Path $releaseDirectory 'bundle\nsis') -Filter "*_${Version}_${Architecture}-setup.exe" -Destination $nsisDestination
Copy-SingleBundle -Directory (Join-Path $releaseDirectory 'bundle\msi') -Filter "*_${Version}_${Architecture}_*.msi" -Destination $msiDestination

Write-ScoopFragment -Package desktop -Archive $desktopArchive -Directory $outputPath
Write-ScoopFragment -Package mcp -Archive $mcpArchive -Directory $outputPath

Write-Output "Prepared $Architecture release artifacts for v$Version."
