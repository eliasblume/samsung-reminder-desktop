[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('current', 'patch', 'minor', 'major')]
    [string]$Part,

    [string]$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-JsonVersion {
    param([Parameter(Mandatory = $true)][string]$Path)

    $document = Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
    return [string]$document.version
}

function Read-CargoVersion {
    param([Parameter(Mandatory = $true)][string]$Path)

    $content = [IO.File]::ReadAllText($Path)
    $match = [regex]::Match($content, '(?m)^version\s*=\s*"(?<version>[^"]+)"\s*$')
    if (-not $match.Success) {
        throw "Could not find the package version in $Path."
    }
    return $match.Groups['version'].Value
}

function Write-TextWithoutBom {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Content
    )

    [IO.File]::WriteAllText($Path, $Content, [Text.UTF8Encoding]::new($false))
}

function Set-JsonVersion {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Version
    )

    $content = [IO.File]::ReadAllText($Path)
    $pattern = [regex]::new('("version"\s*:\s*")[^"]+(")')
    $match = $pattern.Match($content)
    if (-not $match.Success) {
        throw "Could not find the JSON version in $Path."
    }
    $replacement = '"version": "' + $Version + '"'
    $updated = $content.Substring(0, $match.Index) + $replacement + $content.Substring($match.Index + $match.Length)
    Write-TextWithoutBom -Path $Path -Content $updated
}

function Set-CargoVersion {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Version
    )

    $content = [IO.File]::ReadAllText($Path)
    $pattern = [regex]::new('(?m)^version\s*=\s*"[^"]+"\s*$')
    if (-not $pattern.IsMatch($content)) {
        throw "Could not find the package version in $Path."
    }
    $updated = $pattern.Replace($content, "version = `"$Version`"", 1)
    Write-TextWithoutBom -Path $Path -Content $updated
}

$packagePath = Join-Path $Root 'package.json'
$tauriPath = Join-Path $Root 'src-tauri\tauri.conf.json'
$cargoPath = Join-Path $Root 'src-tauri\Cargo.toml'

$versions = @(
    Read-JsonVersion -Path $packagePath
    Read-JsonVersion -Path $tauriPath
    Read-CargoVersion -Path $cargoPath
)

if (@($versions | Select-Object -Unique).Count -ne 1) {
    throw "Source versions do not match: $($versions -join ', ')."
}

$match = [regex]::Match($versions[0], '^(?<major>0|[1-9]\d*)\.(?<minor>0|[1-9]\d*)\.(?<patch>0|[1-9]\d*)$')
if (-not $match.Success) {
    throw "The source version '$($versions[0])' is not a stable SemVer version."
}

$major = [int]$match.Groups['major'].Value
$minor = [int]$match.Groups['minor'].Value
$patch = [int]$match.Groups['patch'].Value

switch ($Part) {
    'major' { $major++; $minor = 0; $patch = 0 }
    'minor' { $minor++; $patch = 0 }
    'patch' { $patch++ }
}

$nextVersion = "$major.$minor.$patch"

if ($Part -ne 'current') {
    Set-JsonVersion -Path $packagePath -Version $nextVersion
    Set-JsonVersion -Path $tauriPath -Version $nextVersion
    Set-CargoVersion -Path $cargoPath -Version $nextVersion
}

Write-Output $nextVersion
