# Build an (unsigned) MSIX for Microsoft Store submission. The Store signs it.
#   ./build-msix.ps1 -SourceDir <unzipped release> -Version 2.0.0 -Arch x64 -Out Cardfile.msix
# Identity values (Partner Center > Product identity) come from environment variables:
#   MSIX_IDENTITY_NAME, MSIX_PUBLISHER, MSIX_PUBLISHER_DISPLAY_NAME
param(
  [Parameter(Mandatory)] [string]$SourceDir,
  [Parameter(Mandatory)] [string]$Version,
  [Parameter(Mandatory)] [ValidateSet("x64", "arm64")] [string]$Arch,
  [Parameter(Mandatory)] [string]$Out
)
$ErrorActionPreference = "Stop"
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$identity = if ($env:MSIX_IDENTITY_NAME) { $env:MSIX_IDENTITY_NAME } else { "FittyAr.Cardfile" }
$publisher = if ($env:MSIX_PUBLISHER) { $env:MSIX_PUBLISHER } else { "CN=FittyAr" }
$display = if ($env:MSIX_PUBLISHER_DISPLAY_NAME) { $env:MSIX_PUBLISHER_DISPLAY_NAME } else { "FittyAr" }

# MSIX versions are four numbers; the Store requires the last one to be 0.
$core = ($Version -split '[-+]')[0]
$msixVersion = "$core.0"

$stage = Join-Path ([IO.Path]::GetTempPath()) "cardfile-msix-$Arch"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $SourceDir "cardfile.exe") $stage
foreach ($extra in @("dictionaries", "THIRD_PARTY_LICENSES.html", "LICENSE-MIT", "LICENSE-APACHE")) {
  $p = Join-Path $SourceDir $extra
  if (Test-Path $p) { Copy-Item -Recurse $p $stage }
}
Copy-Item -Recurse (Join-Path $here "Assets") (Join-Path $stage "Assets")
(Get-Content (Join-Path $here "AppxManifest.xml.in") -Raw).
  Replace("@IDENTITY_NAME@", $identity).
  Replace("@PUBLISHER@", $publisher).
  Replace("@PUBLISHER_DISPLAY_NAME@", $display).
  Replace("@VERSION@", $msixVersion).
  Replace("@ARCH@", $Arch) | Set-Content -Encoding utf8 (Join-Path $stage "AppxManifest.xml")

$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe not found (install the Windows SDK)" }
& $makeappx.FullName pack /o /d $stage /p $Out
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
Write-Host "Built $Out ($msixVersion, $Arch)"
