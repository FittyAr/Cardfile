# Cardfile installer for Windows (x64 and ARM64).
#
#   irm https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.ps1))) [-Version v2.0.0] [-Stable] [-NoDesktopShortcut] [-Uninstall] [-VerifyOnly] [-InsecureSkipSignature]
#
# Always installs the newest published release (not a fixed version): the
# newest stable one, or the newest pre-release while no stable release is newer.
# -Stable ignores pre-releases; -Version installs that exact tag.
#
# Delegates to the verified Inno Setup .exe (no separate ZIP installation).
# -Portable -Destination E:\Cardfile prepares an unregistered portable folder.
# Installs to %LOCALAPPDATA%\Programs\FittyAr\Cardfile (all FittyAr apps share the
# FittyAr folder; the cards live in %APPDATA%\FittyAr\Cardfile), adds Start menu and desktop
# shortcuts and registers Cardfile in
# "Installed apps" using the Inno uninstaller. No admin
# rights are needed. Cardfile updates itself afterwards (Settings -> General).
#
# The download is checked against the signed SHA256SUMS and its own minisign
# signature. minisign itself is downloaded from its official release
# (jedisct1/minisign), pinned by version and SHA-256 below.
#   -InsecureSkipSignature  skip the signatures (only the checksum is checked)
#   -VerifyOnly             download and verify the release, install nothing
param (
    [string]$Version = "",
    [switch]$Stable = $false,
    [switch]$NoDesktopShortcut = $false,
    [switch]$Uninstall = $false,
    [switch]$VerifyOnly = $false,
    [switch]$InsecureSkipSignature = $false,
    [switch]$Yes = $false,
    [switch]$Portable = $false,
    [string]$Destination = ""
)

# Also accept bare words, e.g. `... | iex` cannot take parameters, but the
# scriptblock form can: `uninstall`, `stable`.
if ($args -contains "uninstall") { $Uninstall = $true }
if ($args -contains "stable") { $Stable = $true }

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

$repo = "FittyAr/Cardfile"
$scriptUrl = "https://raw.githubusercontent.com/$repo/main/install.ps1"
# Cardfile release signing key (minisign). Must match PUBLIC_KEY in
# crates/cardfile-updater/src/lib.rs and install.sh.
$minisignPubKey = "RWSEWpMd7tkiGAaQwVFXiSYyfV6ODYRttbEFQ/WBOTzrbMB1diAfrG+w"
# minisign for Windows: https://github.com/jedisct1/minisign/releases
$minisignVersion = "0.12"
$minisignZipSha256 = "37b600344e20c19314b2e82813db2bfdcc408b77b876f7727889dbd46d539479"

$vendorDir = Join-Path $env:LOCALAPPDATA "Programs\FittyAr"
$installDir = Join-Path $vendorDir "Cardfile"
$exePath = Join-Path $installDir "cardfile.exe"
# Shell folders, with the usual locations as a fallback (GetFolderPath can be empty).
function Get-ShellFolder([string]$name, [string]$fallback) {
    $path = [Environment]::GetFolderPath($name)
    if ([string]::IsNullOrWhiteSpace($path)) { $path = $fallback }
    return $path
}
$startMenuLink = Join-Path (Get-ShellFolder "Programs" (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs")) "Cardfile.lnk"
$desktopLink = Join-Path (Get-ShellFolder "Desktop" (Join-Path $env:USERPROFILE "Desktop")) "Cardfile.lnk"
$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Cardfile"
# Where Cardfile keeps the cards (asked before deleting on uninstall).
$dataDir = Join-Path $env:APPDATA "FittyAr\Cardfile"
# Installed by the cargo-dist installer of 2.0.0-alpha.1 to 2.0.0-alpha.3.
$legacyDir = Join-Path $env:USERPROFILE ".cardfile\bin"
# Used by this script before installs moved under the FittyAr folder.
$previousDir = Join-Path $env:LOCALAPPDATA "Programs\Cardfile"

# Whether this machine runs ARM64 Windows (also when this PowerShell itself
# runs emulated as x64).
function Test-Arm64 {
    $arch = $null
    try {
        $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    } catch {
        $arch = $null
    }
    if (-not $arch) {
        $arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
    }
    return $arch -match '^arm64$'
}

function Read-YesNo([string]$question) {
    if ($Yes) { return $true }
    $answer = Read-Host "$question [y/N]"
    return $answer -match "^[yY](es)?$"
}

# --- PATH (raw registry value, so entries like %USERPROFILE%\bin keep their variables) ---

function Get-UserPathEntries {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Environment")
    try {
        $raw = $key.GetValue("Path", "", [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
    } finally {
        $key.Close()
    }
    return @("$raw" -split ';' | Where-Object { $_ })
}

function Set-UserPathEntries([string[]]$entries) {
    $key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Environment", $true)
    try {
        $key.SetValue("Path", ($entries -join ';'), [Microsoft.Win32.RegistryValueKind]::ExpandString)
    } finally {
        $key.Close()
    }
    # Setting a user variable through .NET broadcasts WM_SETTINGCHANGE, so
    # terminals opened from now on see the new PATH.
    [Environment]::SetEnvironmentVariable("CARDFILE_PATH_REFRESH", "1", "User")
    [Environment]::SetEnvironmentVariable("CARDFILE_PATH_REFRESH", $null, "User")
}

function Test-IsDir([string]$entry, [string]$dir) {
    return [Environment]::ExpandEnvironmentVariables($entry).TrimEnd('\') -eq $dir.TrimEnd('\')
}

function Add-ToUserPath {
    $entries = Get-UserPathEntries
    if (-not @($entries | Where-Object { Test-IsDir $_ $installDir })) {
        Set-UserPathEntries (@($entries) + $installDir)
        $env:PATH = "$env:PATH;$installDir"
    }
}

function Remove-FromUserPath([string]$dir) {
    $entries = Get-UserPathEntries
    $kept = @($entries | Where-Object { -not (Test-IsDir $_ $dir) })
    if ($kept.Count -ne $entries.Count) {
        Set-UserPathEntries $kept
    }
}

# --- Download and verification ---

function Get-Text([string]$url) {
    $response = Invoke-WebRequest -Uri $url -UseBasicParsing
    if ($response.Content -is [byte[]]) {
        return [System.Text.Encoding]::UTF8.GetString($response.Content)
    }
    return [string]$response.Content
}

# The release to install: -Version, or the newest non-draft release
# (pre-releases included unless -Stable). GitHub lists newest first.
function Get-ReleaseInfo {
    if ($Version) {
        $tag = if ($Version.StartsWith("v")) { $Version } else { "v$Version" }
        return Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/tags/$tag" -UseBasicParsing
    }
    $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases?per_page=30" -UseBasicParsing
    # A tag with a suffix (v2.0.0-alpha.3) is a pre-release too. 0.x and 1.x
    # were the old Python app, without these archives.
    $candidates = @($releases | Where-Object {
            -not $_.draft -and $_.tag_name -notmatch '^v[01]\.' -and
            (-not $Stable -or -not ($_.prerelease -or $_.tag_name.Contains("-")))
        })
    if ($candidates.Count -eq 0) {
        if ($Stable) {
            throw "There is no stable release yet. Run without -Stable to install the newest pre-release."
        }
        throw "No published release found."
    }
    return $candidates[0]
}

function Get-Minisign([string]$dir) {
    $zip = Join-Path $dir "minisign-$minisignVersion-win64.zip"
    $url = "https://github.com/jedisct1/minisign/releases/download/$minisignVersion/minisign-$minisignVersion-win64.zip"
    Write-Host "Downloading minisign $minisignVersion..."
    Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
    $actual = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash.ToLowerInvariant()
    if ($actual -ne $minisignZipSha256) {
        throw "The minisign download does not match its pinned SHA-256 ($actual)."
    }
    $out = Join-Path $dir "minisign"
    Expand-Archive -Path $zip -DestinationPath $out -Force
    $arch = if (Test-Arm64) { "aarch64" } else { "x86_64" }
    $exe = Join-Path $out "minisign-win64\$arch\minisign.exe"
    if (-not (Test-Path $exe)) {
        throw "minisign.exe not found in the minisign $minisignVersion archive."
    }
    return $exe
}

# Checks $file (published as $name) against the release's SHA256SUMS and,
# unless -InsecureSkipSignature, the minisign signatures of both. The trusted
# comment written by the release pipeline names the tag and the file.
function Test-Download([string]$file, [string]$name, [string]$base, [string]$tag, [string]$tempDir) {
    Write-Host "Verifying SHA-256 checksum..."
    $sumsPath = Join-Path $tempDir "SHA256SUMS"
    Invoke-WebRequest -Uri "$base/SHA256SUMS" -OutFile $sumsPath -UseBasicParsing
    $line = Get-Content $sumsPath | Where-Object { ($_ -split '\s+')[-1].TrimStart('*') -eq $name } | Select-Object -First 1
    if (-not $line) {
        throw "$name is not listed in SHA256SUMS."
    }
    $expected = ($line -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $file).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        throw "SHA-256 checksum mismatch for $name."
    }

    if ($InsecureSkipSignature) {
        Write-Host "Warning: -InsecureSkipSignature given; the signatures were not checked." -ForegroundColor Yellow
        return
    }
    $minisign = Get-Minisign $tempDir
    Write-Host "Verifying minisign signatures..."
    foreach ($pair in @(@($sumsPath, "SHA256SUMS"), @($file, $name))) {
        $target = $pair[0]
        $published = $pair[1]
        $sig = "$target.minisig"
        Invoke-WebRequest -Uri "$base/$published.minisig" -OutFile $sig -UseBasicParsing
        $out = & $minisign -V -P $minisignPubKey -m $target -x $sig 2>&1 | Out-String
        $flat = ($out -replace "\s+", " ")
        if ($LASTEXITCODE -ne 0 -or -not $flat.Contains("Cardfile $tag release file: $published")) {
            throw "minisign signature verification failed for $published."
        }
    }
}

# Downloads and verifies the architecture-specific Inno Setup installer.
function Get-Package([string]$tempDir) {
    $release = Get-ReleaseInfo
    $script:tag = $release.tag_name
    if ($script:tag -notmatch '^v\d+\.\d+\.\d+[A-Za-z0-9.-]*$') { throw "Unexpected release tag." }
    $arch = if (Test-Arm64) { "arm64" } else { "x64" }
    $name = "Cardfile-$($script:tag.TrimStart('v'))-setup-$arch.exe"
    if (-not (@($release.assets | ForEach-Object { $_.name }) -contains $name)) {
        throw "Release $($script:tag) has no $name. Choose a release containing the Windows installer."
    }
    $base = "https://github.com/$repo/releases/download/$($script:tag)"
    $file = Join-Path $tempDir $name
    Invoke-WebRequest -Uri "$base/$name" -OutFile $file -UseBasicParsing
    Test-Download $file $name $base $script:tag $tempDir
    return $file
}

# Retire the script's registration only after Inno reports success. These exact
# keys/files belong to the old bootstrap; never recursively remove its folders.
function Remove-LegacyRegistration {
    foreach ($dir in @($installDir, $previousDir, $legacyDir)) {
        Remove-FromUserPath $dir
        $legacyUninstaller = Join-Path $dir 'uninstall.ps1'
        if (Test-Path -LiteralPath $legacyUninstaller) {
            Remove-Item -LiteralPath $legacyUninstaller -Force
        }
    }
    if (Test-Path -LiteralPath $uninstallKey) {
        Remove-Item -LiteralPath $uninstallKey -Recurse -Force
    }
}

function Invoke-Install {
    if ($Portable -and -not $VerifyOnly -and [string]::IsNullOrWhiteSpace($Destination)) {
        throw '-Portable requires -Destination, for example E:\Cardfile.'
    }
    $targetDir = if ($Destination) { [IO.Path]::GetFullPath($Destination) } else { $installDir }
    if ($targetDir.Contains('"')) { throw 'The destination cannot contain quotation marks.' }
    $tempDir = Join-Path ([IO.Path]::GetTempPath()) ("cardfile-setup-" + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $tempDir | Out-Null
    try {
        $package = Get-Package $tempDir
        if ($VerifyOnly) { Write-Host "Installer verified: $tag"; return }
        $mode = if ($Portable) { 'portable' } else { 'installed' }
        $parameters = @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', '/CURRENTUSER', "/MODE=$mode", "/DIR=`"$targetDir`"")
        if (-not $Portable -and -not $NoDesktopShortcut) { $parameters += '/TASKS=desktopicon' }
        else { $parameters += '/TASKS=' }
        $process = Start-Process -FilePath $package -ArgumentList $parameters -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "Installer failed with exit code $($process.ExitCode). Existing data was retained." }
        if (-not $Portable) { Remove-LegacyRegistration }
        Write-Host "Cardfile $tag ready in $targetDir" -ForegroundColor Green
        Write-Host "Open $(Join-Path $targetDir 'cardfile.exe')."
    } finally {
        # This path was created above, is absolute and stays under the OS temp root.
        $resolvedTemp = [IO.Path]::GetFullPath($tempDir)
        $tempRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
        if (-not $resolvedTemp.StartsWith($tempRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe temporary cleanup path.' }
        Remove-Item -LiteralPath $resolvedTemp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

function Invoke-Uninstall {
    if ($Portable) { throw 'Portable copies have no uninstaller. Close Cardfile and remove the chosen folder when no longer needed.' }
    $targetDir = if ($Destination) { [IO.Path]::GetFullPath($Destination) } else { $installDir }
    $uninstaller = Join-Path $targetDir 'unins000.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        $parameters = if ($Yes) { @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') } else { @('/NORESTART') }
        $process = Start-Process -FilePath $uninstaller -ArgumentList $parameters -WindowStyle Hidden -Wait -PassThru
        if ($process.ExitCode -ne 0) { throw "Uninstall failed with exit code $($process.ExitCode)." }
        Write-Host 'Cardfile removed. Your cards and settings were kept.'
    } elseif (Test-Path -LiteralPath $uninstallKey) {
        if (-not (Read-YesNo 'Remove the legacy Cardfile program? All data folders will be kept.')) { return }
        # Remove only known program files, never an arbitrary installation tree.
        foreach ($name in @('cardfile.exe', 'THIRD_PARTY_LICENSES.html', 'LICENSE-MIT', 'LICENSE-APACHE')) {
            $file = Join-Path $installDir $name
            Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
        }
        Remove-LegacyRegistration
        Write-Host 'Legacy program removed. Data folders were kept.'
    } else { Write-Host "No Cardfile uninstaller found in $targetDir." }
}

if ($Uninstall) { Invoke-Uninstall } else { Invoke-Install }
