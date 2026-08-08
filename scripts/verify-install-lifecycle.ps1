#requires -Version 5.1
<#
.SYNOPSIS
  Verify PureWall's clean install, optional upgrade, and uninstall lifecycle on a
  disposable Windows runner.

.DESCRIPTION
  Fail-fast lifecycle verification:
    1. Requires Windows and an explicit -DisposableRunner switch for any mutation.
    2. Verifies Authenticode before installation.
    3. Snapshots ONLY the documented PureWall-owned HKCU registry keys plus the
       HKCU\...\Run\PureWall value.
    4. Installs the previous MSI when supplied, otherwise performs a clean current
       install; installs the current MSI as an upgrade when a previous MSI was supplied.
    5. Verifies the installed PureWall executable exists and reports the expected file
       version WITHOUT launching it.
    6. Uninstalls via msiexec using the exact current MSI path.
    7. Verifies the installed binary is gone, compares the PureWall-owned registry
       snapshot, and reports residue (read-only).
    8. Stops on every non-zero msiexec exit except the documented reboot-required codes
       (3010 = success, reboot required; 1641 = success, reboot initiated).

  The script never enumerates or deletes arbitrary directories and never writes HKLM,
  Windows policy, or Windows 11 context-menu-mode registry keys.

  -SelfTest runs the pure helpers (path containment, MSI extension validation, version
  ordering, PureWall-owned registry allowlist, redacted command reporting) with synthetic
  inputs and performs no mutation.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/verify-install-lifecycle.ps1 `
    -CurrentMsi "D:\a\_temp\smoke-current\PureWall_0.1.0_x64_en-US.msi" `
    -PreviousMsi "D:\a\_temp\smoke-previous\PureWall_0.0.9_x64_en-US.msi" `
    -DisposableRunner

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/verify-install-lifecycle.ps1 -SelfTest

.PARAMETER CurrentMsi
  Path to the current signed MSI to smoke test.

.PARAMETER PreviousMsi
  Optional path to a previous signed MSI to exercise the clean-install + upgrade path.

.PARAMETER DisposableRunner
  Required for mutation. Confirms this run happens on a GitHub-hosted disposable Windows
  runner, never on a maintainer workstation.

.PARAMETER SelfTest
  Run the pure helper self-tests and exit without mutation.
#>
[CmdletBinding()]
param(
  [string]$CurrentMsi,
  [string]$PreviousMsi,
  [switch]$DisposableRunner,
  [switch]$SelfTest
)

$ErrorActionPreference = 'Stop'

# msiexec exit codes that still count as success (documented reboot-required codes).
$script:RebootRequiredExitCodes = @(3010, 1641)

# The ONLY registry scope this script may read: PureWall-owned HKCU entries plus the
# autostart Run value. Matches CONTRIBUTING.md and README.md.
$script:AllowedRegistryPaths = @(
  'HKCU:\Software\Classes\Directory\Background\shell\PureWall',
  'HKCU:\Software\Classes\Directory\Background\shell\PWNext',
  'HKCU:\Software\Classes\Directory\Background\shell\PWLike',
  'HKCU:\Software\Classes\Directory\Background\shell\PWDislike',
  'HKCU:\Software\Classes\Directory\Background\shell\PWPause',
  'HKCU:\Software\Classes\PureWall_Commands',
  'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run\PureWall'
)

# Fixed candidate install locations. The script NEVER enumerates arbitrary directories.
$script:InstallCandidatePaths = @(
  (Join-Path $env:ProgramFiles 'PureWall\PureWall.exe'),
  (Join-Path ${env:ProgramFiles(x86)} 'PureWall\PureWall.exe'),
  (Join-Path $env:LOCALAPPDATA 'PureWall\PureWall.exe'),
  (Join-Path $env:LOCALAPPDATA 'Programs\PureWall\PureWall.exe')
)

function Test-WindowsPlatform {
  if ($env:OS -eq 'Windows_NT') {
    return $true
  }
  return ([System.Environment]::OSVersion.Platform -eq [System.PlatformID]::Win32NT)
}

function ConvertTo-CanonicalPath {
  param([string]$Path)
  if ([string]::IsNullOrWhiteSpace($Path)) {
    return $null
  }
  $full = [System.IO.Path]::GetFullPath($Path)
  return $full.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
}

function Test-PathContained {
  param(
    [string]$Child,
    [string]$Root
  )
  $child = ConvertTo-CanonicalPath $Child
  $root = ConvertTo-CanonicalPath $Root
  if ($null -eq $child -or $null -eq $root) {
    return $false
  }
  if ($child -eq $root) {
    return $true
  }
  $prefix = $root + [System.IO.Path]::DirectorySeparatorChar
  return $child.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-IsMsiPath {
  param([string]$Path)
  if ([string]::IsNullOrWhiteSpace($Path)) {
    return $false
  }
  return ([System.IO.Path]::GetExtension($Path) -eq '.msi')
}

function Test-NewerVersion {
  param(
    [string]$Candidate,
    [string]$Baseline
  )
  $candidateVersion = [version]$Candidate
  $baselineVersion = [version]$Baseline
  return ($candidateVersion -gt $baselineVersion)
}

function Test-AllowedRegistryPath {
  param([string]$Path)
  if ([string]::IsNullOrWhiteSpace($Path)) {
    return $false
  }
  $normalized = $Path.TrimEnd('\')
  foreach ($allowed in $script:AllowedRegistryPaths) {
    if ($normalized -ieq $allowed) {
      return $true
    }
  }
  return $false
}

function Format-RedactedCommand {
  param([string]$CommandLine)
  $redacted = $CommandLine
  if (-not [string]::IsNullOrWhiteSpace($env:USERPROFILE)) {
    $redacted = $redacted.Replace($env:USERPROFILE, '%USERPROFILE%')
  }
  if (-not [string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
    $redacted = $redacted.Replace($env:LOCALAPPDATA, '%LOCALAPPDATA%')
  }
  return $redacted
}

function Get-VersionFromMsiName {
  param([string]$MsiPath)
  $name = [System.IO.Path]::GetFileName($MsiPath)
  if ($name -match '_(\d+\.\d+\.\d+(?:\.\d+)?)_') {
    return $Matches[1]
  }
  throw "Unable to derive the version from MSI file name: $name"
}

function Get-FileVersionString {
  param([string]$Path)
  $info = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($Path)
  if ($null -eq $info -or [string]::IsNullOrWhiteSpace($info.FileVersion)) {
    throw "Unable to read the file version of $Path"
  }
  return $info.FileVersion
}

function Find-InstalledPureWallExe {
  foreach ($candidate in $script:InstallCandidatePaths) {
    if (Test-Path -LiteralPath $candidate) {
      return $candidate
    }
  }
  return $null
}

function Get-PureWallRegistrySnapshot {
  $snapshot = [System.Collections.Generic.Dictionary[string, string]]::new()
  foreach ($path in $script:AllowedRegistryPaths) {
    if (-not (Test-AllowedRegistryPath -Path $path)) {
      throw "Refusing to snapshot a registry path outside the PureWall allowlist: $path"
    }
    if (Test-Path -LiteralPath $path) {
      $snapshot[$path] = 'present'
    }
  }
  $runPath = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run'
  if (Test-AllowedRegistryPath -Path (Join-Path $runPath 'PureWall')) {
    $runValue = (Get-ItemProperty -Path $runPath -Name 'PureWall' -ErrorAction SilentlyContinue).PureWall
    if (-not [string]::IsNullOrWhiteSpace($runValue)) {
      $snapshot[(Join-Path $runPath 'PureWall')] = 'value-present'
    }
  }
  return $snapshot
}

function Invoke-Msiexec {
  param([string[]]$Arguments)
  $redacted = Format-RedactedCommand ($Arguments -join ' ')
  Write-Host "Running msiexec: $redacted"
  $process = Start-Process -FilePath 'msiexec.exe' -ArgumentList $Arguments -Wait -PassThru
  return $process.ExitCode
}

function Assert-MsiexecExitCode {
  param(
    [int]$ExitCode,
    [string]$Operation
  )
  if ($ExitCode -eq 0) {
    Write-Host "$Operation succeeded (exit code 0)"
    return
  }
  if ($script:RebootRequiredExitCodes -contains $ExitCode) {
    Write-Host "$Operation succeeded with documented reboot-required exit code $ExitCode"
    return
  }
  throw "$Operation failed with msiexec exit code $ExitCode"
}

function Assert-AuthenticodeValid {
  param([string]$InstallerPath)
  $signature = Get-AuthenticodeSignature -FilePath $InstallerPath
  if ($null -eq $signature -or $signature.Status -ne 'Valid') {
    throw "Authenticode signature is not Valid for $InstallerPath (status: $($signature.Status))"
  }
  Write-Host "Authenticode Valid: $InstallerPath"
}

function Assert-DisposableRunnerGuard {
  if (-not $DisposableRunner) {
    throw 'Refusing to mutate this machine: pass -DisposableRunner, which is reserved for GitHub-hosted disposable Windows runners'
  }
}

function Assert-MsiAllowed {
  param([string]$MsiPath)
  if (-not (Test-Path -LiteralPath $MsiPath)) {
    throw "MSI does not exist: $MsiPath"
  }
  if (-not (Test-IsMsiPath $MsiPath)) {
    throw "Not an MSI file: $MsiPath"
  }
  $allowedRoots = @()
  if (-not [string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
    $allowedRoots += $env:RUNNER_TEMP
  }
  $allowedRoots += [System.IO.Path]::GetTempPath()

  # A junction/reparse point below the temp root can resolve outside it. Resolve
  # the final target (following reparse points) before containment checks so a
  # malicious or accidental junction cannot bypass the disposable-temp boundary.
  $resolved = $null
  try {
    $resolved = [System.IO.Path]::GetFullPath((Get-Item -LiteralPath $MsiPath -Force).Target)
  }
  catch {
    $resolved = ConvertTo-CanonicalPath $MsiPath
  }
  if (-not [string]::IsNullOrWhiteSpace($resolved)) {
    $MsiPath = $resolved
  }

  $contained = $false
  foreach ($root in $allowedRoots) {
    if (Test-PathContained -Child $MsiPath -Root $root) {
      $contained = $true
      break
    }
  }
  if (-not $contained) {
    throw "MSI path is not inside a disposable temp root: $MsiPath"
  }
}

function Invoke-LifecycleSelfTest {
  $failures = [System.Collections.Generic.List[string]]::new()

  # Path containment.
  if (-not (Test-PathContained -Child 'C:\repo\bundle\a.msi' -Root 'C:\repo')) {
    $failures.Add('containment rejected a direct child of the root')
  }
  if (-not (Test-PathContained -Child 'C:\repo' -Root 'C:\repo')) {
    $failures.Add('containment rejected the root itself')
  }
  if (Test-PathContained -Child 'C:\repo2\a.msi' -Root 'C:\repo') {
    $failures.Add('containment accepted a sibling prefix as inside the root')
  }
  if (Test-PathContained -Child 'C:\repo\..\outside\a.msi' -Root 'C:\repo') {
    $failures.Add('containment accepted a parent traversal as inside the root')
  }
  if (-not (Test-PathContained -Child 'c:\REPO\bundle\a.msi' -Root 'C:\repo')) {
    $failures.Add('containment rejected a case-variant of the root')
  }
  if (Test-PathContained -Child $null -Root 'C:\repo') {
    $failures.Add('containment accepted a null child path')
  }

  # MSI extension validation.
  if (-not (Test-IsMsiPath 'C:\repo\PureWall_0.1.0_x64_en-US.msi')) {
    $failures.Add('MSI validation rejected a .msi path')
  }
  if (-not (Test-IsMsiPath 'C:\repo\PureWall_0.1.0_x64_en-US.MSI')) {
    $failures.Add('MSI validation rejected a case-variant .MSI path')
  }
  if (Test-IsMsiPath 'C:\repo\PureWall_0.1.0_x64-setup.exe') {
    $failures.Add('MSI validation accepted a setup EXE path')
  }
  if (Test-IsMsiPath '') {
    $failures.Add('MSI validation accepted an empty path')
  }

  # Version ordering.
  if (-not (Test-NewerVersion -Candidate '0.2.0.0' -Baseline '0.1.0.0')) {
    $failures.Add('version ordering rejected a newer candidate')
  }
  if (Test-NewerVersion -Candidate '0.1.0.0' -Baseline '0.2.0.0') {
    $failures.Add('version ordering accepted an older candidate')
  }
  if (Test-NewerVersion -Candidate '1.0.0.0' -Baseline '1.0.0.0') {
    $failures.Add('version ordering accepted equal versions as newer')
  }
  if (Test-NewerVersion -Candidate '0.1.0' -Baseline '0.1.0') {
    $failures.Add('version ordering accepted equal 3-part versions as newer')
  }

  # PureWall-owned registry allowlist.
  if (-not (Test-AllowedRegistryPath 'HKCU:\Software\Classes\Directory\Background\shell\PWNext')) {
    $failures.Add('registry allowlist rejected a PureWall-owned HKCU key')
  }
  if (-not (Test-AllowedRegistryPath 'HKCU:\Software\Classes\Directory\Background\shell\PWNEXT')) {
    $failures.Add('registry allowlist rejected a case-variant PureWall-owned HKCU key')
  }
  if (-not (Test-AllowedRegistryPath 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Run\PureWall')) {
    $failures.Add('registry allowlist rejected the PureWall Run value key')
  }
  if (Test-AllowedRegistryPath 'HKLM:\Software\Classes\Directory\Background\shell\PWNext') {
    $failures.Add('registry allowlist accepted an HKLM key')
  }
  if (Test-AllowedRegistryPath 'HKCU:\Software\Classes\Directory\Background\shell\Evil') {
    $failures.Add('registry allowlist accepted a non-PureWall HKCU key')
  }
  if (Test-AllowedRegistryPath 'HKCU:\Software\Policies\PureWall') {
    $failures.Add('registry allowlist accepted a Windows policy key')
  }
  if (Test-AllowedRegistryPath $null) {
    $failures.Add('registry allowlist accepted a null path')
  }

  # Redacted command reporting.
  $profile = if ([string]::IsNullOrWhiteSpace($env:USERPROFILE)) { 'C:\Users\runneradmin' } else { $env:USERPROFILE }
  $rawCommand = "msiexec /i `"$profile\AppData\Local\Temp\smoke\PureWall.msi`" /qn /norestart"
  $redactedCommand = Format-RedactedCommand $rawCommand
  if ($redactedCommand.Contains($profile)) {
    $failures.Add('command redaction left the user profile path visible')
  }
  if (-not $redactedCommand.Contains('%USERPROFILE%')) {
    $failures.Add('command redaction did not substitute %USERPROFILE%')
  }

  if ($failures.Count -gt 0) {
    Write-Error ($failures -join [Environment]::NewLine)
    exit 1
  }
  Write-Host 'verify-install-lifecycle self-test OK (path containment, MSI validation, version ordering, registry allowlist, redacted commands)'
  exit 0
}

if ($SelfTest) {
  Invoke-LifecycleSelfTest
}

if (-not (Test-WindowsPlatform)) {
  throw 'verify-install-lifecycle.ps1 requires Windows'
}
Assert-DisposableRunnerGuard

if ([string]::IsNullOrWhiteSpace($CurrentMsi)) {
  throw 'CurrentMsi is required'
}
Assert-MsiAllowed -MsiPath $CurrentMsi

$hasPrevious = -not [string]::IsNullOrWhiteSpace($PreviousMsi)
if ($hasPrevious) {
  Assert-MsiAllowed -MsiPath $PreviousMsi
  if (-not (Test-NewerVersion -Candidate (Get-VersionFromMsiName $CurrentMsi) -Baseline (Get-VersionFromMsiName $PreviousMsi))) {
    throw 'Current MSI version must be newer than the Previous MSI version'
  }
}

Assert-AuthenticodeValid -InstallerPath $CurrentMsi
if ($hasPrevious) {
  Assert-AuthenticodeValid -InstallerPath $PreviousMsi
}

Write-Host 'Snapshotting PureWall-owned HKCU registry entries (read-only)'
$beforeSnapshot = Get-PureWallRegistrySnapshot

$expectedVersion = Get-VersionFromMsiName $CurrentMsi

if ($hasPrevious) {
  Write-Host 'Installing previous MSI as the upgrade baseline'
  $exitCode = Invoke-Msiexec -Arguments @('/i', ('"' + $PreviousMsi + '"'), '/qn', '/norestart')
  Assert-MsiexecExitCode -ExitCode $exitCode -Operation 'Previous MSI install'

  $installedExePath = Find-InstalledPureWallExe
  if ($null -eq $installedExePath) {
    throw 'Previous MSI install did not produce a PureWall executable'
  }
  $installedVersion = Get-FileVersionString $installedExePath
  $expectedPreviousVersion = Get-VersionFromMsiName $PreviousMsi
  if ($installedVersion -notin @($expectedPreviousVersion, "$expectedPreviousVersion.0")) {
    throw "Installed PureWall file version $installedVersion does not match previous MSI version $expectedPreviousVersion"
  }
  Write-Host "Previous install verified without launching: $installedExePath ($installedVersion)"

  Write-Host 'Installing current MSI as an upgrade over the previous MSI'
  $exitCode = Invoke-Msiexec -Arguments @('/i', ('"' + $CurrentMsi + '"'), '/qn', '/norestart')
  Assert-MsiexecExitCode -ExitCode $exitCode -Operation 'Current MSI upgrade install'
}
else {
  Write-Host 'Installing current MSI as a clean install'
  $exitCode = Invoke-Msiexec -Arguments @('/i', ('"' + $CurrentMsi + '"'), '/qn', '/norestart')
  Assert-MsiexecExitCode -ExitCode $exitCode -Operation 'Current MSI install'
}

$installedExePath = Find-InstalledPureWallExe
if ($null -eq $installedExePath) {
  throw 'PureWall executable not found after install'
}
$installedVersion = Get-FileVersionString $installedExePath
if ($installedVersion -notin @($expectedVersion, "$expectedVersion.0")) {
  throw "Installed PureWall file version $installedVersion does not match current MSI version $expectedVersion"
}
Write-Host "Installed PureWall executable verified without launching: $installedExePath ($installedVersion)"

Write-Host 'Uninstalling via msiexec with the exact current MSI path'
$exitCode = Invoke-Msiexec -Arguments @('/x', ('"' + $CurrentMsi + '"'), '/qn', '/norestart')
Assert-MsiexecExitCode -ExitCode $exitCode -Operation 'Uninstall'

if (Test-Path -LiteralPath $installedExePath) {
  throw "PureWall executable still present after uninstall: $installedExePath"
}
Write-Host "Uninstall verified: $installedExePath is gone"

Write-Host 'Comparing the PureWall-owned registry snapshot after uninstall'
$afterSnapshot = Get-PureWallRegistrySnapshot
$residue = [System.Collections.Generic.List[string]]::new()
foreach ($key in $afterSnapshot.Keys) {
  $origin = if ($beforeSnapshot.ContainsKey($key)) { 'pre-existing' } else { 'created-during-smoke' }
  $residue.Add("$key ($origin)")
}
if ($residue.Count -gt 0) {
  Write-Host 'PureWall-owned registry residue after uninstall (read-only report, nothing deleted):'
  foreach ($line in $residue) {
    Write-Host "  $line"
  }
}
else {
  Write-Host 'No PureWall-owned registry residue after uninstall'
}

Write-Host 'Installer lifecycle smoke completed successfully'
exit 0
