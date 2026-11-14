param(
    [string]$CargoConfig,
    [switch]$Offline,
    [switch]$DebugBuild,
    [switch]$SkipUpdaterSigning,
    [string]$AdditionalConfig,
    [switch]$IsolatedUpdateTest,
    [ValidateRange(1,16)][int]$Jobs = 2
)
$ErrorActionPreference = 'Stop'
$appDirectory = Split-Path -Parent $PSScriptRoot
$savedSigningKey=$env:TAURI_SIGNING_PRIVATE_KEY
$savedSigningPassword=$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
Push-Location -LiteralPath $appDirectory
try {
    if ($env:OPENKODA_BUILD_TEST_DATA_DIR -and !$IsolatedUpdateTest) { throw 'Configuração de teste isolado não pode entrar em um pacote normal.' }
    if (!$SkipUpdaterSigning -and !$env:TAURI_SIGNING_PRIVATE_KEY) {
        $vault=if ($IsolatedUpdateTest) { Join-Path $appDirectory '.local/update-test-signing/private-key.dpapi' } else { Join-Path $env:USERPROFILE '.openkoda-release-signing/production/private-key.dpapi' }
        if (!$IsolatedUpdateTest -and !(Test-Path -LiteralPath $vault)) {
            foreach($legacy in @((Join-Path $env:LOCALAPPDATA 'OpenKodaReleaseSigning/production/private-key.dpapi'),(Join-Path $appDirectory '.local/update-signing/private-key.dpapi'))) { if(Test-Path -LiteralPath $legacy){$vault=$legacy;break} }
        }
        if (!(Test-Path -LiteralPath $vault)) { throw 'Chave de assinatura do atualizador ausente. Configure o cofre local ou TAURI_SIGNING_PRIVATE_KEY.' }
        Add-Type -AssemblyName System.Security
        $bytes=[Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($vault),$null,[Security.Cryptography.DataProtectionScope]::CurrentUser)
        try { $env:TAURI_SIGNING_PRIVATE_KEY=[Text.Encoding]::UTF8.GetString($bytes) } finally { [Array]::Clear($bytes,0,$bytes.Length) }
        $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD=''
    }
    $prepared = & (Join-Path $PSScriptRoot 'prepare-security-runtime.ps1') -Offline:$Offline -BundleStage
    $stageName = Split-Path $prepared.directory -Leaf
    if ($stageName -notmatch '^security-runtime-stage-[0-9a-f-]{36}$' -or $prepared.directory -ne (Join-Path $appDirectory ".local/$stageName" | Convert-Path)) { throw 'Destino de preparo inesperado.' }
    # JSON Merge Patch removes the development tree and maps only this fresh projection.
    $map = [ordered]@{'../security-runtime/'=$null}
    $map["../.local/$stageName/"] = 'security-runtime/'
    $bundleConfig = Join-Path $appDirectory ".local/$stageName.json"
    $bundlePatch=@{resources=$map}
    if ($SkipUpdaterSigning) { $bundlePatch.createUpdaterArtifacts=$false }
    $patch = @{bundle=$bundlePatch} | ConvertTo-Json -Depth 4
    [IO.File]::WriteAllText($bundleConfig,$patch,[Text.UTF8Encoding]::new($false))
    $arguments = @('run', 'tauri', '--', 'build', '--config', $bundleConfig, '--bundles', 'nsis')
    if ($AdditionalConfig) { $arguments += @('--config',(Resolve-Path -LiteralPath $AdditionalConfig).Path) }
    if ($IsolatedUpdateTest) {
        if (!$env:OPENKODA_BUILD_TEST_DATA_DIR) { throw 'Informe OPENKODA_BUILD_TEST_DATA_DIR no build do ensaio.' }
        $arguments += @('--features','updater-test-harness')
    }
    if ($DebugBuild) { $arguments += '--debug' }
    $arguments += @('--', '--locked', '--jobs', "$Jobs")
    if ($Offline) { $arguments += '--offline' }
    if ($CargoConfig) {
        $resolvedConfig = (Resolve-Path -LiteralPath $CargoConfig).Path
        $arguments += @('--config', $resolvedConfig)
    }
    $profile = if ($DebugBuild) { 'debug' } else { 'release' }
    $metadataArgs = @('metadata','--format-version','1','--no-deps','--locked')
    if ($Offline) { $metadataArgs += '--offline' }
    if ($CargoConfig) { $metadataArgs += @('--config',$resolvedConfig) }
    Push-Location -LiteralPath 'src-tauri'
    try { $metadata = & cargo @metadataArgs; if ($LASTEXITCODE -ne 0) { throw 'Falha ao resolver o destino Cargo do build.' } }
    finally { Pop-Location }
    $target = ($metadata | ConvertFrom-Json).target_directory
    if (![IO.Path]::IsPathRooted($target)) { throw 'Destino Cargo inválido.' }
    $releaseRoot = Join-Path $target $profile
    $releaseVersion = (Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json).version
    $releaseProduct = (Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json).productName
    if ($AdditionalConfig) {
        $configPatch=Get-Content -LiteralPath $AdditionalConfig -Raw | ConvertFrom-Json
        if ($configPatch.version) { $releaseVersion=$configPatch.version }
        if ($configPatch.productName) { $releaseProduct=$configPatch.productName }
    }
    $expectedInstaller = Join-Path $releaseRoot "bundle/nsis/${releaseProduct}_${releaseVersion}_x64-setup.exe"
    $started = [DateTime]::UtcNow
    & npm.cmd @arguments
    if ($LASTEXITCODE -ne 0) { throw 'O empacotamento falhou; nenhum pacote foi declarado pronto.' }
    $installers = @(Get-Item -LiteralPath $expectedInstaller)
    if ($installers[0].PSIsContainer -or $installers[0].Attributes -band [IO.FileAttributes]::ReparsePoint -or $installers[0].LastWriteTimeUtc -lt $started) { throw 'O build não forneceu um novo instalador NSIS.' }
    if (!$SkipUpdaterSigning -and !(Test-Path -LiteralPath "$expectedInstaller.sig")) { throw 'A assinatura do atualizador não foi produzida.' }
    $files = @($installers | ForEach-Object {
        $signature = Get-AuthenticodeSignature -LiteralPath $_.FullName
        [ordered]@{
            path = $_.FullName
            bytes = $_.Length
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            authenticode = $signature.Status.ToString()
            updaterSignature = if (Test-Path -LiteralPath "$($_.FullName).sig") { "$($_.FullName).sig" } else { $null }
        }
    })
    $receipt = [ordered]@{
        generatedAt = [DateTime]::UtcNow.ToString('o')
        profile = $profile
        version = $releaseVersion
        files = $files
        releaseRoot = $releaseRoot
        securityRuntime = $prepared.inventory
        runtimeDiagnostic = $prepared.diagnostic
        securityRuntimeLockSha256 = (Get-FileHash -LiteralPath (Join-Path $prepared.directory 'package-lock.json')).Hash.ToLowerInvariant()
        npmLockSha256 = (Get-FileHash -LiteralPath 'package-lock.json' -Algorithm SHA256).Hash.ToLowerInvariant()
        cargoLockSha256 = (Get-FileHash -LiteralPath 'src-tauri/Cargo.lock' -Algorithm SHA256).Hash.ToLowerInvariant()
        publicReleaseApproved = $false
        isolatedUpdateTest = [bool]$IsolatedUpdateTest
        updaterSigned = !$SkipUpdaterSigning
        limitations = @('Empacotamento não comprova instalação, assinatura ou atualização.', 'Pacote sem assinatura é destinado a validação local; não é uma release pública certificada.')
    }
    New-Item -ItemType Directory -Path '.local' -Force | Out-Null
    $receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath '.local/package-windows.json' -Encoding utf8
    $receipt | ConvertTo-Json -Depth 6
} finally {
    $env:TAURI_SIGNING_PRIVATE_KEY=$savedSigningKey
    $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD=$savedSigningPassword
    Pop-Location
}
