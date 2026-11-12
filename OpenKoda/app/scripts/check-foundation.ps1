param([switch]$SkipBrowser, [switch]$SkipNative, [string]$CargoConfig = '', [switch]$Offline, [ValidateRange(1,16)][int]$Jobs = 2)
$ErrorActionPreference = 'Stop'
$appRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
Push-Location $appRoot
$results = [Collections.Generic.List[object]]::new()
$cargoOptions = @('--locked','--jobs',"$Jobs")
if ($Offline) { $cargoOptions += '--offline' }
if ($CargoConfig) { $cargoOptions += @('--config', [IO.Path]::GetFullPath($CargoConfig)) }
function Invoke-Gate([string]$Name, [scriptblock]$Action) {
    $started = [DateTime]::UtcNow
    & $Action
    $code = $LASTEXITCODE
    $results.Add(@{ name=$Name; exitCode=$code; startedAt=$started.ToString('o'); endedAt=[DateTime]::UtcNow.ToString('o') })
    if ($code -ne 0) { throw "Etapa falhou: $Name (exit $code). Nenhuma etapa seguinte sera executada." }
}
try {
    Invoke-Gate 'Frontend: TypeScript e build' { npm.cmd run build }
    Invoke-Gate 'Frontend: testes unitarios' { npm.cmd test }
    Invoke-Gate 'Nucleo: testes Rust' { cargo test --manifest-path src-tauri/Cargo.toml @cargoOptions }
    if (-not $SkipBrowser) { Invoke-Gate 'Navegacao: Playwright Edge' { npm.cmd run test:e2e } }
    Invoke-Gate 'SDK oficial: disponibilidade local' { & '.\security-runtime\bin\node.exe' 'security-runtime/cli.mjs' --check }
    Invoke-Gate 'SDK oficial: protocolo e preflight offline' { & '.\security-runtime\bin\node.exe' --test 'security-runtime/tests/runtime.test.mjs' }
    if (-not $SkipNative) {
        Invoke-Gate 'Desktop: build com interface incorporada' { cargo build --manifest-path src-tauri/Cargo.toml --features custom-protocol @cargoOptions }
        $metadataOptions = @('--format-version','1','--no-deps','--locked','--manifest-path','src-tauri/Cargo.toml')
        if ($CargoConfig) { $metadataOptions += @('--config', [IO.Path]::GetFullPath($CargoConfig)) }
        if ($Offline) { $metadataOptions += '--offline' }
        $metadata = cargo metadata @metadataOptions | ConvertFrom-Json
        if ($LASTEXITCODE -ne 0 -or !$metadata.target_directory) { throw 'Cargo nao informou o diretorio do binario de teste.' }
        $binary = Join-Path $metadata.target_directory 'debug/openkoda.exe'
        $nativeSha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
        $previousBinary = $env:OPENKODA_SMOKE_BINARY
        try {
            $env:OPENKODA_SMOKE_BINARY = $binary
            Invoke-Gate 'Desktop: projeto de codigo, motor oficial e reinicio sem chamada paga' { node scripts/native-source-edition-smoke.mjs --expected-sha256 $nativeSha256 }
        } finally { $env:OPENKODA_SMOKE_BINARY = $previousBinary }
    }
} finally {
    New-Item -ItemType Directory -Path '.local' -Force | Out-Null
    @{ generatedAt=[DateTime]::UtcNow.ToString('o'); browserSkipped=[bool]$SkipBrowser; nativeSkipped=[bool]$SkipNative; checks=@($results.ToArray()); note='Edicao de codigo. A chamada paga do SDK e a instalacao possuem recibos separados. Suite da edicao anterior: commit 01c1960.' } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath '.local/foundation-checks.json' -Encoding utf8
    Pop-Location
}
