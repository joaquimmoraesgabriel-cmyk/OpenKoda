param(
    [string]$CargoConfig,
    [switch]$Offline,
    [switch]$DebugBuild,
    [ValidateRange(1,16)][int]$Jobs = 2
)
$ErrorActionPreference = 'Stop'
$appDirectory = Split-Path -Parent $PSScriptRoot
Push-Location -LiteralPath $appDirectory
try {
    $prepared = & (Join-Path $PSScriptRoot 'prepare-security-runtime.ps1') -Offline:$Offline -BundleStage
    $stageName = Split-Path $prepared.directory -Leaf
    if ($stageName -notmatch '^security-runtime-stage-[0-9a-f-]{36}$' -or $prepared.directory -ne (Join-Path $appDirectory ".local/$stageName" | Convert-Path)) { throw 'Destino de preparo inesperado.' }
    # JSON Merge Patch removes the development tree and maps only this fresh projection.
    $map = [ordered]@{'../security-runtime/'=$null}
    $map["../.local/$stageName/"] = 'security-runtime/'
    $bundleConfig = Join-Path $appDirectory ".local/$stageName.json"
    $patch = @{bundle=@{resources=$map}} | ConvertTo-Json -Depth 4
    [IO.File]::WriteAllText($bundleConfig,$patch,[Text.UTF8Encoding]::new($false))
    $arguments = @('run', 'tauri', '--', 'build', '--config', $bundleConfig, '--bundles', 'nsis')
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
    $expectedInstaller = Join-Path $releaseRoot "bundle/nsis/OpenKoda_${releaseVersion}_x64-setup.exe"
    $started = [DateTime]::UtcNow
    & npm.cmd @arguments
    if ($LASTEXITCODE -ne 0) { throw 'O empacotamento falhou; nenhum pacote foi declarado pronto.' }
    $installers = @(Get-Item -LiteralPath $expectedInstaller)
    if ($installers[0].PSIsContainer -or $installers[0].Attributes -band [IO.FileAttributes]::ReparsePoint -or $installers[0].LastWriteTimeUtc -lt $started) { throw 'O build não forneceu um novo instalador NSIS.' }
    $files = @($installers | ForEach-Object {
        $signature = Get-AuthenticodeSignature -LiteralPath $_.FullName
        [ordered]@{
            path = $_.FullName
            bytes = $_.Length
            sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
            authenticode = $signature.Status.ToString()
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
        limitations = @('Empacotamento não comprova instalação, assinatura ou atualização.', 'Pacote sem assinatura é destinado a validação local; não é uma release pública certificada.')
    }
    New-Item -ItemType Directory -Path '.local' -Force | Out-Null
    $receipt | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath '.local/package-windows.json' -Encoding utf8
    $receipt | ConvertTo-Json -Depth 6
} finally {
    Pop-Location
}
