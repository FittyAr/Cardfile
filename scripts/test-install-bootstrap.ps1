# Contract tests. All downloads, process launches and legacy cleanup are mocked.
$ErrorActionPreference = 'Stop'
$source = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot '../install.ps1')
$source = $source.Substring(0, $source.LastIndexOf('if ($Uninstall)'))
. ([scriptblock]::Create($source))

function Assert-Contract([bool]$condition, [string]$message) {
    if (-not $condition) { throw $message }
}

$script:launches = @()
$script:retired = 0
function Get-Package([string]$tempDir) {
    $script:tag = 'v2.0.0-test'
    return Join-Path $tempDir 'verified-setup.exe'
}
function Start-Process {
    param($FilePath, $ArgumentList, $WindowStyle, [switch]$Wait, [switch]$PassThru)
    $script:launches += ,@($ArgumentList)
    return [pscustomobject]@{ ExitCode = 0 }
}
function Remove-LegacyRegistration { $script:retired++ }

$Portable = $true
$Destination = Join-Path ([IO.Path]::GetTempPath()) 'USB destination with spaces'
Invoke-Install
Assert-Contract ($script:launches.Count -eq 1) 'Portable setup did not launch exactly once.'
Assert-Contract ($script:launches[0] -contains '/MODE=portable') 'Missing portable mode.'
Assert-Contract ($script:launches[0] -contains "/DIR=`"$Destination`"") 'Destination was not quoted.'
Assert-Contract ($script:retired -eq 0) 'Portable setup must not retire installed registrations.'

$Portable = $false
$NoDesktopShortcut = $true
Invoke-Install
Assert-Contract ($script:launches[1] -contains '/MODE=installed') 'Missing installed mode.'
Assert-Contract ($script:launches[1] -contains '/CURRENTUSER') 'Default setup must not elevate.'
Assert-Contract ($script:launches[1] -contains '/TASKS=') 'NoDesktopShortcut was ignored.'
Assert-Contract ($script:retired -eq 1) 'Legacy registration was not retired after success.'

$VerifyOnly = $true
Invoke-Install
Assert-Contract ($script:launches.Count -eq 2) 'VerifyOnly launched an installer.'
Assert-Contract ($script:retired -eq 1) 'VerifyOnly changed legacy registrations.'

$VerifyOnly = $false
function Get-Package([string]$tempDir) { throw 'simulated signature rejection' }
$rejected = $false
try { Invoke-Install } catch { $rejected = $_.Exception.Message -eq 'simulated signature rejection' }
Assert-Contract $rejected 'Signature rejection was swallowed.'
Assert-Contract ($script:launches.Count -eq 2) 'Unverified setup was launched.'
Assert-Contract ($script:retired -eq 1) 'Failed verification retired a working installation.'
Write-Host 'Bootstrap contract tests passed (no application or installer launched).'
