#requires -Version 5.1
<#
.SYNOPSIS
  Prepare PureWall's secret-backed release signing configuration.

.DESCRIPTION
  Requires the four signing inputs (WINDOWS_CERTIFICATE, WINDOWS_CERTIFICATE_PASSWORD,
  TAURI_SIGNING_PRIVATE_KEY, TAURI_SIGNING_PUBLIC_KEY), decodes the base64 PFX into
  $env:RUNNER_TEMP, imports it into Cert:\CurrentUser\My, and writes
  src-tauri/tauri.release.generated.conf.json as UTF-8 without BOM.

  The generated file contains ONLY:
    - bundle.createUpdaterArtifacts = true
    - bundle.windows.certificateThumbprint (captured from the import)
    - bundle.windows.digestAlgorithm = "sha256"
    - bundle.windows.timestampUrl (must be HTTPS)
    - plugins.updater.pubkey (TAURI_SIGNING_PUBLIC_KEY)
    - plugins.updater.endpoints (the fixed WiseZenn/PureWall HTTPS latest.json endpoint)

  Never print certificate content, passwords, private keys, or the generated JSON.

  -ConfigOnly performs a dry-run with dummy non-secret inputs: it validates inputs,
  round-trips the generated JSON through ConvertFrom-Json, and writes the output file
  WITHOUT importing any certificate.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts/prepare-release-config.ps1 `
    -ConfigOnly -UpdaterPublicKey 'LOCAL_TEST_PUBLIC_KEY' `
    -CertificateThumbprint '0000000000000000000000000000000000000000' `
    -TimestampUrl 'https://timestamp.test.invalid'

.PARAMETER Certificate
  Base64-encoded PFX. Defaults to $env:WINDOWS_CERTIFICATE. Required unless -ConfigOnly.

.PARAMETER CertificatePassword
  Password for the PFX. Defaults to $env:WINDOWS_CERTIFICATE_PASSWORD. Required unless -ConfigOnly.

.PARAMETER SigningPrivateKey
  Tauri updater signing private key. Defaults to $env:TAURI_SIGNING_PRIVATE_KEY.
  Required unless -ConfigOnly; consumed later by `tauri build`, never written to disk here.

.PARAMETER SigningPrivateKeyPassword
  Optional password for the Tauri updater signing private key. Defaults to
  $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD.

.PARAMETER UpdaterPublicKey
  Tauri updater public key. Defaults to $env:TAURI_SIGNING_PUBLIC_KEY.

.PARAMETER TimestampUrl
  Authenticode timestamp URL. Defaults to $env:WINDOWS_CERTIFICATE_TIMESTAMP_URL.
  Must be non-empty and use HTTPS.

.PARAMETER CertificateThumbprint
  Only honored with -ConfigOnly (dry-run thumbprint). The real flow always uses the
  thumbprint captured from the certificate import.

.PARAMETER ConfigOnly
  Dry-run: no certificate decoding or import; validates the JSON structure only.
#>
[CmdletBinding()]
param(
  [string]$Certificate,
  [string]$CertificatePassword,
  [string]$SigningPrivateKey,
  [string]$SigningPrivateKeyPassword,
  [string]$UpdaterPublicKey,
  [string]$TimestampUrl,
  [string]$CertificateThumbprint,
  [switch]$ConfigOnly
)

$ErrorActionPreference = 'Stop'

$script:RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$script:OutputConfigPath = Join-Path $script:RepoRoot 'src-tauri\tauri.release.generated.conf.json'
$script:FixedUpdaterEndpoint = 'https://github.com/WiseZenn/PureWall/releases/latest/download/latest.json'
$script:PfxFileName = 'purewall-signing-certificate.pfx'

function Get-RequiredInput {
  param(
    [string]$Name,
    [string]$Value
  )
  if ([string]::IsNullOrWhiteSpace($Value)) {
    throw "Missing required signing input: $Name"
  }
  return $Value
}

function Assert-HttpsTimestampUrl {
  param([string]$TimestampUrl)
  if ([string]::IsNullOrWhiteSpace($TimestampUrl)) {
    throw 'Timestamp URL must not be empty'
  }
  if (-not $TimestampUrl.StartsWith('https://', [System.StringComparison]::OrdinalIgnoreCase)) {
    throw 'Timestamp URL must use HTTPS'
  }
  return $TimestampUrl
}

function New-ReleaseConfigJson {
  param(
    [string]$CertificateThumbprint,
    [string]$TimestampUrl,
    [string]$UpdaterPublicKey
  )
  $config = [ordered]@{
    bundle = [ordered]@{
      createUpdaterArtifacts = $true
      windows = [ordered]@{
        certificateThumbprint = $CertificateThumbprint
        digestAlgorithm = 'sha256'
        timestampUrl = $TimestampUrl
      }
    }
    plugins = [ordered]@{
      updater = [ordered]@{
        pubkey = $UpdaterPublicKey
        endpoints = @($script:FixedUpdaterEndpoint)
      }
    }
  }
  $json = $config | ConvertTo-Json -Depth 10

  # Prove the generated document parses and keeps every required field.
  $parsed = $json | ConvertFrom-Json
  if ($parsed.bundle.windows.certificateThumbprint -ne $CertificateThumbprint) {
    throw 'Generated config validation failed: certificateThumbprint'
  }
  if ($parsed.bundle.windows.digestAlgorithm -ne 'sha256') {
    throw 'Generated config validation failed: digestAlgorithm'
  }
  if ($parsed.bundle.windows.timestampUrl -ne $TimestampUrl) {
    throw 'Generated config validation failed: timestampUrl'
  }
  if ($parsed.bundle.createUpdaterArtifacts -ne $true) {
    throw 'Generated config validation failed: createUpdaterArtifacts'
  }
  if ($parsed.plugins.updater.pubkey -ne $UpdaterPublicKey) {
    throw 'Generated config validation failed: updater pubkey'
  }
  if ($parsed.plugins.updater.endpoints.Count -ne 1 -or
      $parsed.plugins.updater.endpoints[0] -ne $script:FixedUpdaterEndpoint) {
    throw 'Generated config validation failed: updater endpoint'
  }
  return $json
}

function Write-ConfigFileUtf8NoBom {
  param(
    [string]$Path,
    [string]$Content
  )
  $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
  # Normalize to LF so the generated config matches the repository's line-ending
  # convention regardless of the PowerShell host's newline style.
  $normalized = $Content -replace "`r?`n", "`n"
  [System.IO.File]::WriteAllText($Path, $normalized, $utf8NoBom)
}

function Test-ConfigOnlyDryRun {
  $thumbprint = Get-RequiredInput -Name 'CertificateThumbprint' -Value $CertificateThumbprint
  $publicKey = Get-RequiredInput -Name 'TAURI_SIGNING_PUBLIC_KEY' -Value $UpdaterPublicKey
  $timestampUrl = Assert-HttpsTimestampUrl $TimestampUrl
  $json = New-ReleaseConfigJson -CertificateThumbprint $thumbprint -TimestampUrl $timestampUrl -UpdaterPublicKey $publicKey
  Write-ConfigFileUtf8NoBom -Path $script:OutputConfigPath -Content $json
  Write-Host "DRY-RUN OK: release config structure validated and written to $script:OutputConfigPath (no certificate imported, contents not printed)"
  exit 0
}

function Test-RealConfigurationFlow {
  $certificate = Get-RequiredInput -Name 'WINDOWS_CERTIFICATE' -Value $Certificate
  $certificatePassword = Get-RequiredInput -Name 'WINDOWS_CERTIFICATE_PASSWORD' -Value $CertificatePassword
  $signingKey = Get-RequiredInput -Name 'TAURI_SIGNING_PRIVATE_KEY' -Value $SigningPrivateKey
  $publicKey = Get-RequiredInput -Name 'TAURI_SIGNING_PUBLIC_KEY' -Value $UpdaterPublicKey
  $timestampUrl = Assert-HttpsTimestampUrl $TimestampUrl
  # $signingKey and $SigningPrivateKeyPassword are validated/optional only; they are
  # consumed later by `tauri build` through their environment variables and are never
  # written to the generated config or printed.

  if ([string]::IsNullOrWhiteSpace($env:RUNNER_TEMP)) {
    throw 'RUNNER_TEMP is not set; refusing to write the certificate outside the runner temp directory'
  }

  $pfxPath = Join-Path $env:RUNNER_TEMP $script:PfxFileName
  try {
    $certificateBytes = [Convert]::FromBase64String($certificate)
    [System.IO.File]::WriteAllBytes($pfxPath, $certificateBytes)

    $securePassword = ConvertTo-SecureString -String $certificatePassword -AsPlainText -Force
    $imported = Import-PfxCertificate -FilePath $pfxPath -CertStoreLocation 'Cert:\CurrentUser\My' -Password $securePassword -ErrorAction Stop
    $thumbprint = $imported.Thumbprint
    if ([string]::IsNullOrWhiteSpace($thumbprint)) {
      throw 'Certificate import did not return a thumbprint'
    }

    $json = New-ReleaseConfigJson -CertificateThumbprint $thumbprint -TimestampUrl $timestampUrl -UpdaterPublicKey $publicKey
    Write-ConfigFileUtf8NoBom -Path $script:OutputConfigPath -Content $json
    Write-Host "Certificate imported and release signing config written to $script:OutputConfigPath (thumbprint captured, contents not printed)"
  }
  finally {
    if (Test-Path -LiteralPath $pfxPath) {
      Remove-Item -LiteralPath $pfxPath -Force
    }
  }
}

if ([string]::IsNullOrWhiteSpace($TimestampUrl)) {
  $TimestampUrl = $env:WINDOWS_CERTIFICATE_TIMESTAMP_URL
}
if ([string]::IsNullOrWhiteSpace($UpdaterPublicKey)) {
  $UpdaterPublicKey = $env:TAURI_SIGNING_PUBLIC_KEY
}
if ([string]::IsNullOrWhiteSpace($Certificate)) {
  $Certificate = $env:WINDOWS_CERTIFICATE
}
if ([string]::IsNullOrWhiteSpace($CertificatePassword)) {
  $CertificatePassword = $env:WINDOWS_CERTIFICATE_PASSWORD
}
if ([string]::IsNullOrWhiteSpace($SigningPrivateKey)) {
  $SigningPrivateKey = $env:TAURI_SIGNING_PRIVATE_KEY
}
if ([string]::IsNullOrWhiteSpace($SigningPrivateKeyPassword)) {
  $SigningPrivateKeyPassword = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
}

if ($ConfigOnly) {
  Test-ConfigOnlyDryRun
}
else {
  Test-RealConfigurationFlow
}
