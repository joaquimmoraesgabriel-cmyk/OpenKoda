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
    Invoke-Gate 'Kali: helper de manutencao' { python scripts/test-environment-helper.py }
    Invoke-Gate 'MCP: transporte stdio e limites' { python scripts/test_kali_executor.py }
    Invoke-Gate 'Kali: contratos da copia do ambiente' { python scripts/test_kali_archives.py }
    if (-not $SkipNative) {
        Invoke-Gate 'Desktop: build com interface incorporada' { cargo build --manifest-path src-tauri/Cargo.toml --features custom-protocol @cargoOptions }
        $nativeSha256 = (Get-FileHash -LiteralPath 'src-tauri/target/debug/openkoda.exe' -Algorithm SHA256).Hash.ToLowerInvariant()
        Invoke-Gate 'Desktop: inicio, projeto, conversas e painel lateral' { node scripts/native-project-chat-smoke.mjs --expected-sha256 $nativeSha256 }
        Invoke-Gate 'Desktop: IPC, evidencias, backup e reinicio reais' { node scripts/native-smoke.mjs --expected-sha256 $nativeSha256 }
        Invoke-Gate 'Desktop: politica de dados, leituras e exportacoes revisadas' { node scripts/native-data-policy-smoke.mjs --expected-sha256 $nativeSha256 }
        Invoke-Gate 'Desktop: validacao e reteste HTTP em loopback' { node scripts/native-http-smoke.mjs --expected-sha256 $nativeSha256 }
        Invoke-Gate 'Desktop: escopo, legado, revogacao e derivacao' { node scripts/native-scope-smoke.mjs --expected-sha256 $nativeSha256 }
        Invoke-Gate 'Desktop: alvos combinados, exclusoes e origem' { node scripts/native-combined-smoke.mjs --expected-sha256 $nativeSha256 }
        Invoke-Gate 'Desktop: fila, revisao humana, captura e recuperacao' { node scripts/native-workflow-smoke.mjs --expected-sha256 $nativeSha256 }
    }
} finally {
    New-Item -ItemType Directory -Path '.local' -Force | Out-Null
    @{ generatedAt=[DateTime]::UtcNow.ToString('o'); browserSkipped=[bool]$SkipBrowser; nativeSkipped=[bool]$SkipNative; checks=@($results.ToArray()); note='Kali instalado, conexao real da IA, assinatura e atualizacao exigem evidencias proprias.' } | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath '.local/foundation-checks.json' -Encoding utf8
    Pop-Location
}
