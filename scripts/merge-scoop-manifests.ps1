[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$X64Manifest,

    [Parameter(Mandatory = $true)]
    [string]$Arm64Manifest,

    [Parameter(Mandatory = $true)]
    [string]$OutputPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Read-Manifest {
    param([Parameter(Mandatory = $true)][string]$Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "Scoop manifest fragment is missing: $Path"
    }
    return Get-Content -LiteralPath $Path -Raw | ConvertFrom-Json
}

$x64 = Read-Manifest -Path $X64Manifest
$arm64 = Read-Manifest -Path $Arm64Manifest
if ($x64.version -ne $arm64.version) {
    throw "Scoop manifest versions differ: $($x64.version) and $($arm64.version)."
}
foreach ($property in @('description', 'homepage', 'license', 'notes')) {
    $x64Value = $x64.$property | ConvertTo-Json -Compress
    $arm64Value = $arm64.$property | ConvertTo-Json -Compress
    if ($x64Value -ne $arm64Value) {
        throw "Scoop manifest property '$property' differs between architectures."
    }
}

$merged = [ordered]@{
    version = $x64.version
    description = $x64.description
    homepage = $x64.homepage
    license = $x64.license
    notes = @($x64.notes)
    architecture = [ordered]@{
        '64bit' = [ordered]@{
            url = $x64.architecture.'64bit'.url
            hash = $x64.architecture.'64bit'.hash
        }
        arm64 = [ordered]@{
            url = $arm64.architecture.arm64.url
            hash = $arm64.architecture.arm64.hash
        }
    }
}

foreach ($property in @('bin', 'shortcuts')) {
    $x64Property = $x64.PSObject.Properties[$property]
    $arm64Property = $arm64.PSObject.Properties[$property]
    if (($null -eq $x64Property) -ne ($null -eq $arm64Property)) {
        throw "Scoop manifest property '$property' is missing from one architecture."
    }
    if ($null -ne $x64Property) {
        $x64Value = $x64Property.Value | ConvertTo-Json -Compress
        $arm64Value = $arm64Property.Value | ConvertTo-Json -Compress
        if ($x64Value -ne $arm64Value) {
            throw "Scoop manifest property '$property' differs between architectures."
        }
        $merged[$property] = $x64Property.Value
    }
}

$merged.checkver = 'github'
$merged.autoupdate = [ordered]@{
    architecture = [ordered]@{
        '64bit' = [ordered]@{
            url = $x64.autoupdate.architecture.'64bit'.url
        }
        arm64 = [ordered]@{
            url = $arm64.autoupdate.architecture.arm64.url
        }
    }
}

$outputFullPath = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $outputFullPath
New-Item -ItemType Directory -Path $outputDirectory -Force | Out-Null
$json = $merged | ConvertTo-Json -Depth 10
[IO.File]::WriteAllText($outputFullPath, "$json`n", [Text.UTF8Encoding]::new($false))

Write-Output "Merged x64 and ARM64 Scoop manifests for v$($x64.version)."
