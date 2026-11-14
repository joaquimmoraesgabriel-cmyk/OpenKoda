param([switch]$Initialize,[switch]$TestKey)
$ErrorActionPreference = 'Stop'
if (!$IsWindows -and $env:OS -ne 'Windows_NT') { throw 'Este cofre local usa DPAPI do Windows.' }
$appDirectory = Split-Path -Parent $PSScriptRoot
$vault = if ($TestKey) { Join-Path $appDirectory '.local/update-test-signing' } else { Join-Path $env:USERPROFILE '.openkoda-release-signing/production' }
if (!$TestKey -and !(Test-Path -LiteralPath (Join-Path $vault 'private-key.dpapi'))) {
    foreach($legacy in @((Join-Path $env:LOCALAPPDATA 'OpenKodaReleaseSigning/production'),(Join-Path $appDirectory '.local/update-signing'))) {
        if (Test-Path -LiteralPath (Join-Path $legacy 'private-key.dpapi')) { $vault=$legacy;break }
    }
}
$protected = Join-Path $vault 'private-key.dpapi'
$public = Join-Path $vault 'public-key.txt'
if (Test-Path -LiteralPath $protected) {
    if (!(Test-Path -LiteralPath $public)) { throw 'Chave pública ausente; não recrie a chave privada existente.' }
    Write-Output $public
    exit 0
}
if (!$Initialize) { throw 'Cofre ausente. Execute com -Initialize uma única vez e faça backup seguro antes de distribuir.' }
New-Item -ItemType Directory -Path $vault -Force | Out-Null
# The CLI writes a short-lived plaintext key before DPAPI sealing. Restrict the
# directory first; failure to apply ACLs stops generation rather than leaking it.
$acl=[Security.AccessControl.DirectorySecurity]::new()
$acl.SetAccessRuleProtection($true,$false)
$owner=[Security.Principal.WindowsIdentity]::GetCurrent().User
$acl.SetOwner($owner)
foreach($identity in @($owner,[Security.Principal.SecurityIdentifier]::new('S-1-5-18'))) {
    $acl.AddAccessRule([Security.AccessControl.FileSystemAccessRule]::new($identity,'FullControl','ContainerInherit,ObjectInherit','None','Allow'))
}
Set-Acl -LiteralPath $vault -AclObject $acl
$temporary = Join-Path $vault ('new-key-' + [Guid]::NewGuid().ToString('N'))
try {
    Push-Location -LiteralPath $appDirectory
    try { & npm.cmd run tauri -- signer generate --ci --write-keys $temporary 2>&1 | Out-Null; if ($LASTEXITCODE -ne 0) { throw 'Não foi possível gerar a chave.' } }
    finally { Pop-Location }
    Add-Type -AssemblyName System.Security
    $bytes = [IO.File]::ReadAllBytes($temporary)
    try { [IO.File]::WriteAllBytes($protected, [Security.Cryptography.ProtectedData]::Protect($bytes, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)) }
    finally { [Array]::Clear($bytes, 0, $bytes.Length) }
    [IO.File]::WriteAllText($public, [IO.File]::ReadAllText("$temporary.pub").Trim(), [Text.UTF8Encoding]::new($false))
    Write-Output $public
} finally {
    foreach ($path in @($temporary, "$temporary.pub")) { if (Test-Path -LiteralPath $path) { Remove-Item -LiteralPath $path -Force } }
}
