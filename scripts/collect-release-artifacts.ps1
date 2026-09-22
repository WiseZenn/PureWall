#requires -Version 5.1
<#
.SYNOPSIS
  Collect PureWall release artifacts from a Tauri bundle root and write SHA256SUMS.txt.

.DESCRIPTION
  Copies only the release deliverables (.msi, setup .exe, .sig updater signatures, and
  latest.json) from -BundleRoot into -OutputDirectory, sorted by file name, computes a
  SHA-256 digest for each copied file with Get-FileHash, and writes SHA256SUMS.txt using
  UTF8Encoding(false). Both -BundleRoot and -OutputDirectory must resolve inside the
  repository workspace. The script fails when any of MSI, setup EXE, updater .sig, or
  latest.json is absent.

  -SelfTest runs the pure helpers (path containment, artifact allowlist, required-set
  detection) with synthetic inputs and performs no filesystem mutation.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/collect-release-artifacts.ps1 `
    -BundleRoot "src-tauri\target\release\bundle" -OutputDirectory "release-artifacts"

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/collect-release-artifacts.ps1 -SelfTest

.PARAMETER BundleRoot
  Directory that contains the Tauri bundle output (msi/, nsis/, updater artifacts).

.PARAMETER OutputDirectory
  Directory that receives the copied artifacts and SHA256SUMS.txt.

.PARAMETER SelfTest
  Run the pure helper self-tests and exit without touching the filesystem.
#>
[CmdletBinding()]
param(
  [string]$BundleRoot,
  [string]$OutputDirectory,
  [switch]$SelfTest
)

$ErrorActionPreference = 'Stop'

$script:RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$script:Sha256SumsName = 'SHA256SUMS.txt'

function ConvertTo-CanonicalPath {
  param([string]$Path)
  if ([string]::IsNullOrWhiteSpace($Path)) {
    return $null
  }
  $full = [System.IO.Path]::GetFullPath($Path)
  return $full.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
}

function Test-PathInsideWorkspace {
  param(
    [string]$Candidate,
    [string]$WorkspaceRoot
  )
  $candidate = ConvertTo-CanonicalPath $Candidate
  $root = ConvertTo-CanonicalPath $WorkspaceRoot
  if ($null -eq $candidate -or $null -eq $root) {
    return $false
  }
  if ($candidate -eq $root) {
    return $true
  }
  $prefix = $root + [System.IO.Path]::DirectorySeparatorChar
  return $candidate.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)
}

function Test-AllowedArtifactFile {
  param([string]$FileName)
  if ([string]::IsNullOrWhiteSpace($FileName)) {
    return $false
  }
  $extension = [System.IO.Path]::GetExtension($FileName)
  if ($extension -eq '.msi') {
    return $true
  }
  if ($extension -eq '.sig') {
    return $true
  }
  if ($FileName -ieq 'latest.json') {
    return $true
  }
  if ($extension -eq '.exe' -and $FileName -match 'setup') {
    return $true
  }
  return $false
}

function Test-RequiredArtifactsPresent {
  param([string[]]$FileNames)
  $hasMsi = $false
  $hasSetupExe = $false
  $hasSignature = $false
  $hasLatestJson = $false
  foreach ($name in $FileNames) {
    $extension = [System.IO.Path]::GetExtension($name)
    if ($extension -eq '.msi') {
      $hasMsi = $true
    }
    if ($extension -eq '.exe' -and $name -match 'setup') {
      $hasSetupExe = $true
    }
    if ($extension -eq '.sig') {
      $hasSignature = $true
    }
    if ($name -ieq 'latest.json') {
      $hasLatestJson = $true
    }
  }
  return ($hasMsi -and $hasSetupExe -and $hasSignature -and $hasLatestJson)
}

function Get-ArtifactFiles {
  param([string]$BundleRoot)
  $bundle = ConvertTo-CanonicalPath $BundleRoot
  if (-not (Test-Path -LiteralPath $bundle)) {
    throw "Bundle root does not exist: $BundleRoot"
  }
  $files = Get-ChildItem -LiteralPath $bundle -Recurse -File |
    Where-Object { Test-AllowedArtifactFile -FileName $_.Name } |
    Sort-Object -Property Name

  # Distinct basenames must map to exactly one artifact. Two files with the same
  # name from different subdirectories would overwrite each other in the flat
  # output directory and silently drop one artifact from checksum coverage.
  $duplicates = @($files | Group-Object -Property Name | Where-Object { $_.Count -gt 1 })
  if ($duplicates.Count -gt 0) {
    $names = ($duplicates | ForEach-Object { $_.Name }) -join ', '
    throw "Duplicate artifact basenames in bundle root: $names"
  }
  return @($files)
}

function Invoke-CollectSelfTest {
  $failures = [System.Collections.Generic.List[string]]::new()

  # Path containment.
  if (-not (Test-PathInsideWorkspace -Candidate 'C:\repo\bundle\a.msi' -WorkspaceRoot 'C:\repo')) {
    $failures.Add('containment rejected a direct child of the workspace root')
  }
  if (-not (Test-PathInsideWorkspace -Candidate 'C:\repo' -WorkspaceRoot 'C:\repo')) {
    $failures.Add('containment rejected the workspace root itself')
  }
  if (Test-PathInsideWorkspace -Candidate 'C:\repo2\bundle\a.msi' -WorkspaceRoot 'C:\repo') {
    $failures.Add('containment accepted a sibling prefix as inside the workspace')
  }
  if (Test-PathInsideWorkspace -Candidate 'C:\repo\bundle\..\..\outside\a.msi' -WorkspaceRoot 'C:\repo') {
    $failures.Add('containment accepted a parent traversal as inside the workspace')
  }
  if (-not (Test-PathInsideWorkspace -Candidate 'c:\REPO\bundle\a.msi' -WorkspaceRoot 'C:\repo')) {
    $failures.Add('containment rejected a case-variant of the workspace root')
  }
  if (Test-PathInsideWorkspace -Candidate $null -WorkspaceRoot 'C:\repo') {
    $failures.Add('containment accepted a null candidate path')
  }

  # Artifact allowlist.
  if (-not (Test-AllowedArtifactFile -FileName 'PureWall_0.1.0_x64_en-US.msi')) {
    $failures.Add('allowlist rejected an MSI artifact')
  }
  if (-not (Test-AllowedArtifactFile -FileName 'PureWall_0.1.0_x64-setup.exe')) {
    $failures.Add('allowlist rejected a setup EXE artifact')
  }
  if (-not (Test-AllowedArtifactFile -FileName 'PureWall_0.1.0_x64_en-US.msi.sig')) {
    $failures.Add('allowlist rejected an updater signature artifact')
  }
  if (-not (Test-AllowedArtifactFile -FileName 'latest.json')) {
    $failures.Add('allowlist rejected latest.json')
  }
  if (-not (Test-AllowedArtifactFile -FileName 'PureWall_0.1.0_x64-setup.EXE')) {
    $failures.Add('allowlist rejected a case-variant setup EXE artifact')
  }
  if (Test-AllowedArtifactFile -FileName 'PureWall_0.1.0_x64.exe') {
    $failures.Add('allowlist accepted a plain EXE without setup in the name')
  }
  if (Test-AllowedArtifactFile -FileName 'README.txt') {
    $failures.Add('allowlist accepted a non-artifact file')
  }
  if (Test-AllowedArtifactFile -FileName $null) {
    $failures.Add('allowlist accepted a null file name')
  }

  # Required artifact set.
  if (-not (Test-RequiredArtifactsPresent -FileNames @('a.msi', 'a-setup.exe', 'a.msi.sig', 'latest.json'))) {
    $failures.Add('required-set detection rejected a complete artifact set')
  }
  if (Test-RequiredArtifactsPresent -FileNames @('a.msi', 'a-setup.exe', 'a.msi.sig')) {
    $failures.Add('required-set detection accepted a set without latest.json')
  }
  if (Test-RequiredArtifactsPresent -FileNames @('a-setup.exe', 'a.msi.sig', 'latest.json')) {
    $failures.Add('required-set detection accepted a set without an MSI')
  }
  if (Test-RequiredArtifactsPresent -FileNames @('a.msi', 'a.msi.sig', 'latest.json')) {
    $failures.Add('required-set detection accepted a set without a setup EXE')
  }
  if (Test-RequiredArtifactsPresent -FileNames @('a.msi', 'a-setup.exe', 'latest.json')) {
    $failures.Add('required-set detection accepted a set without updater signatures')
  }
  if (Test-RequiredArtifactsPresent -FileNames @()) {
    $failures.Add('required-set detection accepted an empty artifact set')
  }

  if ($failures.Count -gt 0) {
    Write-Error ($failures -join [Environment]::NewLine)
    exit 1
  }
  Write-Host 'collect-release-artifacts self-test OK (path containment, artifact allowlist, required artifact set)'
  exit 0
}

if ($SelfTest) {
  Invoke-CollectSelfTest
}

if ([string]::IsNullOrWhiteSpace($BundleRoot)) {
  throw 'BundleRoot is required'
}
if ([string]::IsNullOrWhiteSpace($OutputDirectory)) {
  throw 'OutputDirectory is required'
}

$bundleRootCanonical = ConvertTo-CanonicalPath $BundleRoot
if (-not (Test-PathInsideWorkspace -Candidate $bundleRootCanonical -WorkspaceRoot $script:RepoRoot)) {
  throw "BundleRoot is outside the workspace: $BundleRoot"
}

$outputCanonical = ConvertTo-CanonicalPath $OutputDirectory
if (-not (Test-PathInsideWorkspace -Candidate $outputCanonical -WorkspaceRoot $script:RepoRoot)) {
  throw "OutputDirectory is outside the workspace: $OutputDirectory"
}

$artifactFiles = Get-ArtifactFiles -BundleRoot $bundleRootCanonical
$artifactNames = @($artifactFiles | ForEach-Object { $_.Name })
if (-not (Test-RequiredArtifactsPresent -FileNames $artifactNames)) {
  throw 'Missing required release artifacts: an MSI, a setup EXE, updater .sig signatures, and latest.json are all required'
}

if (-not (Test-Path -LiteralPath $outputCanonical)) {
  New-Item -ItemType Directory -Path $outputCanonical -Force | Out-Null
}

# Refuse to merge into an output directory that already contains files this run
# did not produce. A stale file from a previous run could otherwise be attested
# and shipped without ever being covered by the freshly written SHA256SUMS.txt.
$unexpectedPreExisting = @(Get-ChildItem -LiteralPath $outputCanonical -File -Force |
  Where-Object { $_.Name -ne $script:Sha256SumsName })
if ($unexpectedPreExisting.Count -gt 0) {
  throw "Output directory contains unexpected pre-existing files: $($unexpectedPreExisting.Name -join ', ')"
}


$sortedArtifacts = $artifactFiles | Sort-Object -Property Name
foreach ($file in $sortedArtifacts) {
  Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $outputCanonical $file.Name) -Force
}

$sumsLines = [System.Collections.Generic.List[string]]::new()
foreach ($file in $sortedArtifacts) {
  $hash = Get-FileHash -LiteralPath (Join-Path $outputCanonical $file.Name) -Algorithm SHA256
  $sumsLines.Add(('{0}  {1}' -f $hash.Hash.ToLowerInvariant(), $file.Name))
}

$utf8NoBom = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllLines((Join-Path $outputCanonical $script:Sha256SumsName), $sumsLines, $utf8NoBom)

Write-Host "Collected $($sortedArtifacts.Count) release artifacts into $outputCanonical and wrote $script:Sha256SumsName"
