# Build with a verified portable compiler. Never install or launch Cardfile.
param(
    [Parameter(Mandatory)][string]$SourceDir,
    [Parameter(Mandatory)][string]$AppVersion,
    [Parameter(Mandatory)][ValidateSet('x64', 'arm64')][string]$Arch,
    [string]$OutputDirectory = '',
    [string]$ToolRoot = ''
)
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $ToolRoot) { $ToolRoot = Join-Path $repoRoot 'target/tooling/inno-7.1.1' }
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $repoRoot 'packaging/out' }
$ToolRoot = [IO.Path]::GetFullPath($ToolRoot)
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
$SourceDir = (Resolve-Path -LiteralPath $SourceDir).Path
if ($ToolRoot.Contains('"')) { throw 'Compiler path cannot contain quotation marks.' }
New-Item -ItemType Directory -Force -Path $ToolRoot, $OutputDirectory | Out-Null
$bootstrap = Join-Path $ToolRoot 'innosetup-7.1.1-x64.exe'
$compilerFolder = Join-Path $ToolRoot 'compiler'
$compiler = Join-Path $compilerFolder 'ISCC.exe'
if (-not (Test-Path -LiteralPath $bootstrap)) {
    Invoke-WebRequest -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-7_1_1/innosetup-7.1.1-x64.exe' -OutFile $bootstrap
}
if ((Get-FileHash -LiteralPath $bootstrap -Algorithm SHA256).Hash.ToLowerInvariant() -ne '8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0') {
    throw 'Inno Setup compiler bootstrap checksum mismatch.'
}
if (-not (Test-Path -LiteralPath $compiler)) {
    # Inno's own /PORTABLE=1 skips external installation/registration steps.
    $process = Start-Process -FilePath $bootstrap -ArgumentList @(
        '/PORTABLE=1', '/CURRENTUSER', '/VERYSILENT', '/SUPPRESSMSGBOXES',
        '/NORESTART', '/NOICONS', '/TASKS=', "/DIR=`"$compilerFolder`""
    ) -WindowStyle Hidden -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw "Portable compiler extraction failed: $($process.ExitCode)" }
}
$fileVersion = $AppVersion -replace '-.*$', ''
& $compiler "/DAppVersion=$AppVersion" "/DFileVersion=$fileVersion" "/DArch=$Arch" "/DSourceDir=$SourceDir" "/O$OutputDirectory" (Join-Path $repoRoot 'packaging/windows/installer.iss')
if ($LASTEXITCODE -ne 0) { throw "Windows installer compilation failed: $Arch" }
