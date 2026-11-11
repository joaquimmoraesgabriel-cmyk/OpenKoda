# Prepare a Windows Sandbox session from a verified local release and its
# extraction receipt. Opening the generated .wsb starts the disposable test.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$ReleaseManifest)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$release=Get-Content -Raw -LiteralPath $ReleaseManifest | ConvertFrom-Json
$inputInstaller=Get-Item -LiteralPath $release.installer
if ((Get-FileHash -LiteralPath $inputInstaller.FullName).Hash.ToLowerInvariant() -ne $release.sha256) { throw 'Installer hash differs from the release manifest.' }
$extract=Get-Content -Raw -LiteralPath $release.receipts.packageExtraction.path | ConvertFrom-Json
if (-not $extract.passed -or $extract.mode -ne 'ExtractOnly' -or $extract.newRelease.installerSha256 -ne $release.sha256 -or -not $extract.newPayload.verification.verified -or $extract.newPayload.verification.sourceSha256 -ne $release.sourceBinarySha256) { throw 'Extraction receipt differs from the selected release.' }
$directory=Join-Path $root ('.local/release-sandbox-'+[guid]::NewGuid())
$in=Join-Path $directory 'input'; $out=Join-Path $directory 'result'
New-Item -ItemType Directory -Path $in,$out | Out-Null
Copy-Item -LiteralPath $inputInstaller.FullName -Destination (Join-Path $in 'setup.exe')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'sandbox-release-guest.ps1') -Destination $in
if ((Get-FileHash -LiteralPath (Join-Path $in 'setup.exe')).Hash.ToLowerInvariant() -ne $release.sha256) { throw 'Copied installer changed.' }
@{version=$release.version;installerSha256=$release.sha256;payloadSha256=$extract.newPayload.verification.sha256} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $in 'release.json') -Encoding UTF8
$inputXml=[Security.SecurityElement]::Escape($in); $outputXml=[Security.SecurityElement]::Escape($out)
$config=Join-Path $directory 'test-release.wsb'
@"
<Configuration>
  <Networking>Disable</Networking><ClipboardRedirection>Disable</ClipboardRedirection>
  <MappedFolders>
    <MappedFolder><HostFolder>$inputXml</HostFolder><SandboxFolder>C:\KodaInput</SandboxFolder><ReadOnly>true</ReadOnly></MappedFolder>
    <MappedFolder><HostFolder>$outputXml</HostFolder><SandboxFolder>C:\KodaResult</SandboxFolder><ReadOnly>false</ReadOnly></MappedFolder>
  </MappedFolders>
  <LogonCommand><Command>powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File C:\KodaInput\sandbox-release-guest.ps1</Command></LogonCommand>
</Configuration>
"@ | Set-Content -LiteralPath $config -Encoding UTF8
$null=[xml](Get-Content -LiteralPath $config -Raw)
[pscustomobject]@{configuration=$config;result=(Join-Path $out 'result.json');version=$release.version;executed=$false} | ConvertTo-Json
