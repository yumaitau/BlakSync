# Install the pinned Syncthing for Windows where BlakSync looks for it.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-syncthing.ps1
#   powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install-syncthing.ps1 -Destination DIR
#
# The default destination is the BlakSync config `bin` folder
# (%BLAKSYNC_CONFIG_DIR%\bin, else %APPDATA%\blaksync\bin). User scope; no admin.
# Works in Windows PowerShell 5.1 and PowerShell 7.
[CmdletBinding()]
param(
  [string]$Destination = '',
  [string]$Version = $(if ($env:SYNCTHING_VERSION) { $env:SYNCTHING_VERSION } else { '2.1.3' })
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
# Windows PowerShell 5.1 may not offer TLS 1.2 to GitHub by default.
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

if (-not $Destination) {
  $configDir = if ($env:BLAKSYNC_CONFIG_DIR) { $env:BLAKSYNC_CONFIG_DIR } else { Join-Path $env:APPDATA 'blaksync' }
  $Destination = Join-Path $configDir 'bin'
}

$arch = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
switch ($arch) {
  'AMD64' { $archName = 'amd64' }
  'ARM64' { $archName = 'arm64' }
  'x86' { $archName = '386' }
  default { throw "unsupported architecture: $arch" }
}

$asset = "syncthing-windows-$archName-v$Version"
$base = "https://github.com/syncthing/syncthing/releases/download/v$Version"
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("blaksync-syncthing-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  $archive = Join-Path $tmp "$asset.zip"
  $sums = Join-Path $tmp 'sha256sum.txt.asc'
  Invoke-WebRequest -UseBasicParsing -Uri "$base/$asset.zip" -OutFile $archive
  Invoke-WebRequest -UseBasicParsing -Uri "$base/sha256sum.txt.asc" -OutFile $sums

  # The sums file is clear-signed; each data line is "<sha256>  <file name>".
  $expected = $null
  foreach ($line in Get-Content -LiteralPath $sums) {
    if ($line -match '^([0-9a-fA-F]{64})\s+\*?(\S+)\s*$' -and $Matches[2] -eq "$asset.zip") {
      $expected = $Matches[1].ToLowerInvariant()
      break
    }
  }
  if (-not $expected) { throw "sha256sum.txt.asc has no entry for $asset.zip" }
  $actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $archive).Hash.ToLowerInvariant()
  if ($actual -ne $expected) { throw "SHA-256 mismatch for $asset.zip: expected $expected, got $actual" }

  $extract = Join-Path $tmp 'x'
  Expand-Archive -LiteralPath $archive -DestinationPath $extract
  $binary = Join-Path (Join-Path $extract $asset) 'syncthing.exe'
  if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw 'extracted archive did not contain syncthing.exe' }

  New-Item -ItemType Directory -Force -Path $Destination | Out-Null
  $target = Join-Path $Destination 'syncthing.exe'
  Copy-Item -LiteralPath $binary -Destination $target -Force

  $reported = & $target --version
  if ($LASTEXITCODE -ne 0 -or -not ($reported -match [regex]::Escape("v$Version"))) {
    throw "installed binary did not report Syncthing v${Version}: $reported"
  }
  Write-Output "Installed Syncthing $Version to $Destination (sha256 $actual)"
}
finally {
  Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
