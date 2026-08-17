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

$portableName = "Samsung-Reminder-$Version-windows-$Architecture"
$portableDirectory = Join-Path $outputPath $portableName
New-Item -ItemType Directory -Path $portableDirectory | Out-Null
Copy-Item -LiteralPath $appExecutable -Destination (Join-Path $portableDirectory 'Reminder.exe')
Copy-Item -LiteralPath $mcpExecutable -Destination (Join-Path $portableDirectory 'samsung-reminder-mcp.exe')
Copy-Item -LiteralPath $licensePath -Destination $portableDirectory
Copy-Item -LiteralPath $readmePath -Destination $portableDirectory
Copy-Item -LiteralPath $legalPath -Destination $portableDirectory
Copy-Item -LiteralPath $noticesPath -Destination $portableDirectory

$portableArchive = Join-Path $outputPath "$portableName.zip"
Compress-Archive -Path (Join-Path $portableDirectory '*') -DestinationPath $portableArchive -CompressionLevel Optimal
Remove-Item -LiteralPath $portableDirectory -Recurse -Force

$hash = (Get-FileHash -LiteralPath $portableArchive -Algorithm SHA256).Hash.ToLowerInvariant()
$hashFile = "$portableArchive.sha256"
[IO.File]::WriteAllText(
    $hashFile,
    "$hash  $([IO.Path]::GetFileName($portableArchive))`n",
    [Text.UTF8Encoding]::new($false)
)

$nsisDestination = Join-Path $outputPath "Samsung-Reminder-$Version-windows-$Architecture-setup.exe"
$msiDestination = Join-Path $outputPath "Samsung-Reminder-$Version-windows-$Architecture.msi"
Copy-SingleBundle -Directory (Join-Path $releaseDirectory 'bundle\nsis') -Filter '*.exe' -Destination $nsisDestination
Copy-SingleBundle -Directory (Join-Path $releaseDirectory 'bundle\msi') -Filter '*.msi' -Destination $msiDestination

$downloadUrl = "https://github.com/$Repository/releases/download/v$Version/$portableName.zip"
$autoupdateUrl = "https://github.com/$Repository/releases/download/v`$version/Samsung-Reminder-`$version-windows-$Architecture.zip"
$scoopArchitecture = if ($Architecture -eq 'x64') { '64bit' } else { 'arm64' }
$manifest = [ordered]@{
    version = $Version
    description = 'Unofficial Samsung Reminder desktop client and local MCP server'
    homepage = "https://github.com/$Repository"
    license = 'MIT'
    notes = @(
        'Requires Samsung Browser for Windows installed in its standard location.'
        'Download Samsung Browser from https://browser.samsung.com/ and sign in before the first sync.'
    )
    architecture = [ordered]@{
        $scoopArchitecture = [ordered]@{
            url = $downloadUrl
            hash = $hash
        }
    }
    bin = @(, @('samsung-reminder-mcp.exe', 'samsung-reminder-mcp'))
    shortcuts = @(, @('Reminder.exe', 'Reminder'))
    checkver = 'github'
    autoupdate = [ordered]@{
        architecture = [ordered]@{
            $scoopArchitecture = [ordered]@{
                url = $autoupdateUrl
            }
        }
    }
}

$manifestPath = Join-Path $outputPath "samsung-reminder-$Architecture.json"
$manifestJson = $manifest | ConvertTo-Json -Depth 10
[IO.File]::WriteAllText($manifestPath, "$manifestJson`n", [Text.UTF8Encoding]::new($false))

Write-Output "Prepared $Architecture release artifacts for v$Version."
