# Cardfile installer for Windows (x64 and ARM64).
#
#   irm https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.ps1 | iex
#   & ([scriptblock]::Create((irm https://raw.githubusercontent.com/FittyAr/Cardfile/main/install.ps1))) [-Version v2.0.0] [-Stable] [-NoDesktopShortcut] [-Uninstall] [-VerifyOnly] [-InsecureSkipSignature]
#
# Always installs the newest published release (not a fixed version): the
# newest stable one, or the newest pre-release while no stable release is newer.
# -Stable ignores pre-releases; -Version installs that exact tag.
#
# Installs to %LOCALAPPDATA%\Programs\Cardfile, adds Start menu and desktop
# shortcuts, puts the folder on the user PATH and registers Cardfile in
# "Installed apps" so it can be uninstalled from Windows Settings. No admin
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
    [switch]$Yes = $false
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

$installDir = Join-Path $env:LOCALAPPDATA "Programs\Cardfile"
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

# Downloads, verifies and extracts the release into $tempDir; returns the
# folder that holds cardfile.exe.
function Get-Package([string]$tempDir) {
    Write-Host "Looking for the newest Cardfile release..."
    $release = Get-ReleaseInfo
    $script:tag = $release.tag_name
    if ($script:tag -notmatch '^v\d+\.\d+\.\d+[A-Za-z0-9.-]*$') {
        throw "Unexpected release tag '$($script:tag)'."
    }
    $kind = if ($release.prerelease -or $script:tag.Contains("-")) { " (pre-release)" } else { "" }
    Write-Host "Newest version: $($script:tag)$kind" -ForegroundColor Green

    $target = if (Test-Arm64) { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
    $zipName = "cardfile-$target.zip"
    if (-not (@($release.assets | ForEach-Object { $_.name }) -contains $zipName)) {
        throw "Release $($script:tag) has no $zipName."
    }
    $base = "https://github.com/$repo/releases/download/$($script:tag)"
    $zipPath = Join-Path $tempDir $zipName
    Write-Host "Downloading $zipName..."
    Invoke-WebRequest -Uri "$base/$zipName" -OutFile $zipPath -UseBasicParsing
    Test-Download $zipPath $zipName $base $script:tag $tempDir

    Write-Host "Extracting..."
    $extract = Join-Path $tempDir "extract"
    Expand-Archive -Path $zipPath -DestinationPath $extract -Force
    $exe = Get-ChildItem -Path $extract -Recurse -Filter "cardfile.exe" | Select-Object -First 1
    if (-not $exe) {
        throw "cardfile.exe not found in $zipName."
    }
    return $exe.DirectoryName
}

# --- Install ---

function New-Shortcut([string]$path, [string]$target) {
    New-Item -ItemType Directory -Force -Path (Split-Path $path) | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut($path)
    $link.TargetPath = $target
    $link.WorkingDirectory = Split-Path $target
    $link.IconLocation = "$target,0"
    $link.Description = "Cardfile - a modern digital card file"
    $link.Save()
}

function Register-Uninstall([string]$version) {
    # A copy of this script runs the uninstall from Windows Settings, offline.
    $local = Join-Path $installDir "uninstall.ps1"
    try {
        if ($PSCommandPath -and (Test-Path $PSCommandPath) -and ($PSCommandPath -ne $local)) {
            Copy-Item -Path $PSCommandPath -Destination $local -Force
        } elseif (-not $PSCommandPath) {
            Invoke-WebRequest -Uri $scriptUrl -OutFile $local -UseBasicParsing
        }
    } catch {
        Write-Host "Could not save the uninstaller ($($_.Exception.Message)); skipping Windows Settings entry." -ForegroundColor Yellow
        return
    }
    New-Item -Path $uninstallKey -Force | Out-Null
    $size = [int]((Get-ChildItem $installDir -Recurse | Measure-Object -Property Length -Sum).Sum / 1KB)
    $values = @{
        DisplayName     = "Cardfile"
        DisplayVersion  = $version.TrimStart("v")
        Publisher       = "FittyAr"
        DisplayIcon     = "$exePath,0"
        InstallLocation = $installDir
        URLInfoAbout    = "https://github.com/$repo"
        UninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$local`" -Uninstall"
        QuietUninstallString = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$local`" -Uninstall -Yes"
    }
    foreach ($name in $values.Keys) {
        Set-ItemProperty -Path $uninstallKey -Name $name -Value $values[$name]
    }
    Set-ItemProperty -Path $uninstallKey -Name EstimatedSize -Value $size -Type DWord
    Set-ItemProperty -Path $uninstallKey -Name NoModify -Value 1 -Type DWord
    Set-ItemProperty -Path $uninstallKey -Name NoRepair -Value 1 -Type DWord
}

# Copies the package into $installDir. A running Cardfile cannot be
# overwritten, but it can be renamed aside (it deletes `*.old` itself later).
function Install-Files([string]$package) {
    Write-Host "Installing to $installDir..."
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    if (Test-Path $exePath) {
        $old = "$exePath.old"
        Remove-Item -Force $old -ErrorAction SilentlyContinue
        try {
            Rename-Item -Path $exePath -NewName "cardfile.exe.old" -ErrorAction Stop
        } catch {
            throw "Cardfile is running and could not be replaced. Close it and try again."
        }
    }
    Copy-Item -Path (Join-Path $package "*") -Destination $installDir -Recurse -Force
}

function Remove-Legacy {
    $legacyExe = Join-Path $legacyDir "cardfile.exe"
    if (-not (Test-Path $legacyExe)) { return }
    Write-Host "An older Cardfile installation was found in $legacyDir."
    if (Read-YesNo "Remove it (your cards are kept)?") {
        Remove-Item -Recurse -Force $legacyDir -ErrorAction SilentlyContinue
        $parent = Split-Path $legacyDir
        if ((Test-Path $parent) -and -not (Get-ChildItem $parent)) {
            Remove-Item -Force $parent -ErrorAction SilentlyContinue
        }
        Remove-FromUserPath $legacyDir
    }
}

# No `exit` here: run through `irm | iex`, it would close the user's shell.
function Invoke-Install {
    Write-Host "Cardfile installer for Windows" -ForegroundColor Blue
    Write-Host "=============================="
    $tempDir = Join-Path $env:TEMP "cardfile_install_$([guid]::NewGuid().ToString('N'))"
    New-Item -ItemType Directory -Force -Path $tempDir | Out-Null
    try {
        $package = Get-Package $tempDir
        if ($VerifyOnly) {
            Write-Host "Cardfile $tag downloaded and verified; nothing was installed (-VerifyOnly)." -ForegroundColor Green
            return
        }
        Install-Files $package
    } catch {
        Write-Host "Error: $($_.Exception.Message) Aborting." -ForegroundColor Red
        throw
    } finally {
        Remove-Item -Recurse -Force $tempDir -ErrorAction SilentlyContinue
    }

    New-Shortcut $startMenuLink $exePath
    if (-not $NoDesktopShortcut) {
        New-Shortcut $desktopLink $exePath
    }
    Add-ToUserPath
    Register-Uninstall $tag
    Remove-Legacy

    Write-Host "==============================" -ForegroundColor Blue
    Write-Host "Cardfile $tag installed." -ForegroundColor Green
    Write-Host "  Program:   $exePath"
    Write-Host "  Shortcuts: Start menu$(if (-not $NoDesktopShortcut) { ' and desktop' })"
    Write-Host "  Updates:   automatic, from inside Cardfile"
    Write-Host "Open it from the Start menu, or type: cardfile" -ForegroundColor Green
}

function Invoke-Uninstall {
    Write-Host "Cardfile uninstaller" -ForegroundColor Blue
    Write-Host "=============================="
    if (-not (Test-Path $installDir) -and -not (Test-Path $uninstallKey)) {
        Write-Host "Cardfile is not installed in $installDir."
    } elseif (Read-YesNo "Uninstall Cardfile from $installDir?") {
        Get-Process cardfile -ErrorAction SilentlyContinue | Where-Object { $_.Path -like "$installDir*" } | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Milliseconds 500
        Remove-Item -Force $startMenuLink, $desktopLink -ErrorAction SilentlyContinue
        Remove-FromUserPath $installDir
        Remove-Item -Recurse -Force $uninstallKey -ErrorAction SilentlyContinue
        # The running uninstall.ps1 lives in $installDir; PowerShell has read it already.
        Remove-Item -Recurse -Force $installDir -ErrorAction SilentlyContinue
        Write-Host "Cardfile was removed." -ForegroundColor Green
    } else {
        Write-Host "Uninstall cancelled."
        return
    }
    if ((Test-Path $dataDir) -and -not $Yes) {
        if (Read-YesNo "Also delete your cards and settings in $dataDir? This cannot be undone.") {
            Remove-Item -Recurse -Force $dataDir -ErrorAction SilentlyContinue
            Write-Host "Your cards and settings were deleted."
        } else {
            Write-Host "Your cards and settings were kept in $dataDir."
        }
    }
}

if ($Uninstall) {
    Invoke-Uninstall
} else {
    Invoke-Install
}
