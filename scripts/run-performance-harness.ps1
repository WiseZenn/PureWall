param(
  [ValidateSet('Run','Compare')][string]$Mode = 'Run',
  [ValidateSet('Ci','Standard','Stress')][string]$Scale = 'Ci',
  [Parameter(Mandatory=$true)][string]$Report,
  [string]$Baseline = '',
  [string]$Candidate = '',
  [string]$Hotspot = '',
  [switch]$KeepData,
  [switch]$AllowStress
)
$ErrorActionPreference = 'Stop'

function Resolve-JsonInputPath {
  param(
    [Parameter(Mandatory=$true)][string]$Value,
    [Parameter(Mandatory=$true)][string]$Role
  )

  if ([string]::IsNullOrWhiteSpace($Value)) {
    throw "$Role is required"
  }
  $resolved = (Resolve-Path -LiteralPath $Value).Path
  if ([System.IO.Path]::GetExtension($resolved) -cne '.json') {
    throw "$Role must use the .json extension"
  }
  return $resolved
}

function Resolve-JsonOutputPath {
  param([Parameter(Mandatory=$true)][string]$Value)

  if ([string]::IsNullOrWhiteSpace($Value)) {
    throw 'Report is required'
  }
  $resolved = [System.IO.Path]::GetFullPath($Value)
  if ([System.IO.Path]::GetExtension($resolved) -cne '.json') {
    throw 'Report must use the .json extension'
  }
  return $resolved
}

$reportPath = Resolve-JsonOutputPath -Value $Report
$reportParent = [System.IO.Path]::GetDirectoryName($reportPath)
if ([string]::IsNullOrWhiteSpace($reportParent)) {
  throw 'Report must have a parent directory'
}
[System.IO.Directory]::CreateDirectory($reportParent) | Out-Null

$repositoryRoot = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$manifestPath = Join-Path $repositoryRoot 'src-tauri/Cargo.toml'
$cargoArguments = @(
  'run',
  '--release',
  '--manifest-path', $manifestPath,
  '--features', 'performance-harness',
  '--', '--performance-harness'
)

if ($Mode -eq 'Run') {
  if (-not [string]::IsNullOrWhiteSpace($Baseline)) {
    throw 'Baseline is valid only in Compare mode'
  }
  if (-not [string]::IsNullOrWhiteSpace($Candidate)) {
    throw 'Candidate is valid only in Compare mode'
  }
  if ($AllowStress -and $Scale -ne 'Stress') {
    throw 'AllowStress is valid only with Stress scale'
  }
  if ($Scale -eq 'Stress' -and -not $AllowStress) {
    throw 'Stress scale requires AllowStress'
  }

  $cargoArguments += @(
    'run',
    '--scale', $Scale.ToLowerInvariant(),
    '--report', $reportPath
  )
  if ($KeepData) {
    $cargoArguments += '--keep-data'
  }
  if ($AllowStress) {
    $cargoArguments += '--allow-stress'
  }
  if (-not [string]::IsNullOrWhiteSpace($Hotspot)) {
    $cargoArguments += @('--hotspot', $Hotspot)
  }
} else {
  if ($KeepData) {
    throw 'KeepData is valid only in Run mode'
  }
  if ($AllowStress) {
    throw 'AllowStress is valid only in Run mode'
  }
  if (-not [string]::IsNullOrWhiteSpace($Hotspot)) {
    throw 'Hotspot is valid only in Run mode'
  }
  $baselinePath = Resolve-JsonInputPath -Value $Baseline -Role 'Baseline'
  $candidatePath = Resolve-JsonInputPath -Value $Candidate -Role 'Candidate'
  if ([System.StringComparer]::OrdinalIgnoreCase.Equals($baselinePath, $candidatePath) -or
      [System.StringComparer]::OrdinalIgnoreCase.Equals($baselinePath, $reportPath) -or
      [System.StringComparer]::OrdinalIgnoreCase.Equals($candidatePath, $reportPath)) {
    throw 'Baseline, Candidate, and Report must use distinct paths'
  }

  $cargoArguments += @(
    'compare',
    '--baseline', $baselinePath,
    '--candidate', $candidatePath,
    '--output', $reportPath
  )
}

& cargo @cargoArguments
if ($LASTEXITCODE -ne 0) {
  exit $LASTEXITCODE
}
