param([Parameter(Mandatory=$true)][string]$File,[Parameter(Mandatory=$true)][string]$Version,[switch]$TestKey)
$ErrorActionPreference='Stop'
$appDirectory=Split-Path -Parent $PSScriptRoot
$filePath=(Resolve-Path -LiteralPath $File).Path
if ($Version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') { throw 'Versão semântica inválida.' }
$savedKey=$env:TAURI_SIGNING_PRIVATE_KEY; $savedPassword=$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
try {
    if (!$env:TAURI_SIGNING_PRIVATE_KEY) {
        Add-Type -AssemblyName System.Security
        $vault=if ($TestKey) { Join-Path $appDirectory '.local/update-test-signing/private-key.dpapi' } else { Join-Path $env:USERPROFILE '.openkoda-release-signing/production/private-key.dpapi' }
        if (!$TestKey -and !(Test-Path -LiteralPath $vault)) {
            foreach($legacy in @((Join-Path $env:LOCALAPPDATA 'OpenKodaReleaseSigning/production/private-key.dpapi'),(Join-Path $appDirectory '.local/update-signing/private-key.dpapi'))) { if(Test-Path -LiteralPath $legacy){$vault=$legacy;break} }
        }
        $bytes=[Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($vault),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
        try { $env:TAURI_SIGNING_PRIVATE_KEY=[Text.Encoding]::UTF8.GetString($bytes) } finally { [Array]::Clear($bytes,0,$bytes.Length) }
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD=''
    }
    Push-Location -LiteralPath $appDirectory
    try { & npm.cmd run tauri -- signer sign --app-version $Version $filePath 2>&1 | Out-Null; if ($LASTEXITCODE -ne 0) { throw 'Não foi possível assinar o artefato.' } }
    finally { Pop-Location }
    if (!(Test-Path -LiteralPath "$filePath.sig")) { throw 'Assinatura não foi produzida.' }
    Write-Output "$filePath.sig"
} finally { $env:TAURI_SIGNING_PRIVATE_KEY=$savedKey; $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$savedPassword }
