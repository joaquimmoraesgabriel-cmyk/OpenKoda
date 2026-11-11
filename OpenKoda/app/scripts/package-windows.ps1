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
    $arguments = @('run', 'tauri', '--', 'build', '--bundles', 'nsis')
    if ($DebugBuild) { $arguments += '--debug' }
    $arguments += @('--', '--locked', '--jobs', "$Jobs")
    if ($Offline) { $arguments += '--offline' }
    if ($CargoConfig) {
        $resolvedConfig = (Resolve-Path -LiteralPath $CargoConfig).Path
        $arguments += @('--config', $resolvedConfig)
    }
    & npm.cmd @arguments
    if ($LASTEXITCODE -ne 0) { throw 'O empacotamento falhou; nenhum pacote foi declarado pronto.' }
    $profile = if ($DebugBuild) { 'debug' } else { 'release' }
    $bundleDirectory = Join-Path $appDirectory "src-tauri/target/$profile/bundle/nsis"
    $releaseVersion = (Get-Content -LiteralPath 'src-tauri/tauri.conf.json' -Raw | ConvertFrom-Json).version
    $installers = @(Get-ChildItem -LiteralPath $bundleDirectory -Filter "OpenKoda_${releaseVersion}_x64-setup.exe" -File)
    if ($installers.Count -eq 0) { throw 'O build terminou sem um instalador NSIS.' }
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
