param([string]$CargoConfig,[switch]$Offline,[ValidateRange(1,16)][int]$Jobs=2)
$ErrorActionPreference='Stop'
$appDirectory=Split-Path -Parent $PSScriptRoot
$savedData=$env:OPENKODA_BUILD_TEST_DATA_DIR
$fixture=Join-Path $appDirectory ('.local/update-rehearsal-'+[Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $fixture | Out-Null
$data=Join-Path $fixture 'data'
$env:OPENKODA_BUILD_TEST_DATA_DIR=$data
try {
    & (Join-Path $PSScriptRoot 'init-update-signing.ps1') -Initialize -TestKey | Out-Null
    $publicKey=[IO.File]::ReadAllText((Join-Path $appDirectory '.local/update-test-signing/public-key.txt')).Trim()
    $receipts=@()
    foreach ($version in @('1.0.9','1.1.0')) {
        $configPath=Join-Path $fixture "config-$version.json"
        $config=@{version=$version;productName='OpenKodaUpdateTest';identifier='com.openkoda.updater-rehearsal';app=@{windows=@(@{label='main';title="OpenKoda update rehearsal $version";width=1400;height=900;minWidth=900;minHeight=620})};plugins=@{updater=@{pubkey=$publicKey;requireSignedVersion=$true;endpoints=@('https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/desktop-updater-rehearsal/latest.json')}}}
        [IO.File]::WriteAllText($configPath,($config|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
        & (Join-Path $PSScriptRoot 'package-windows.ps1') -CargoConfig $CargoConfig -Offline:$Offline -Jobs $Jobs -IsolatedUpdateTest -AdditionalConfig $configPath | Out-Null
        $receipt=Get-Content -LiteralPath (Join-Path $appDirectory '.local/package-windows.json') -Raw | ConvertFrom-Json
        $archive=Join-Path $fixture $version;New-Item -ItemType Directory -Path $archive | Out-Null
        $original=$receipt.files[0].path
        $artifact=Join-Path $archive (Split-Path $original -Leaf)
        Copy-Item -LiteralPath $original -Destination $artifact
        Copy-Item -LiteralPath "$original.sig" -Destination "$artifact.sig"
        Copy-Item -LiteralPath (Join-Path $receipt.releaseRoot 'openkoda.exe') -Destination (Join-Path $archive 'openkoda.exe')
        $receipt.files[0].path=$artifact;$receipt.files[0].updaterSignature="$artifact.sig"
        $manifest=Join-Path $archive 'receipt.json';$receipt | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $manifest -Encoding utf8
        $receipts+=@{version=$version;receipt=$manifest;installer=$artifact;binary=(Join-Path $archive 'openkoda.exe')}
    }
    Push-Location -LiteralPath $appDirectory
    try { & node scripts/prepare-update-manifest.mjs --receipt $receipts[1].receipt --internal-test --tag desktop-updater-rehearsal --rollout 100 --out (Join-Path $fixture 'latest.json'); if ($LASTEXITCODE -ne 0) {throw 'O manifesto do ensaio não foi produzido.'} }
    finally { Pop-Location }
    $summary=@{schema='openkoda.update-rehearsal.v1';directory=$fixture;dataDirectory=$data;installDirectory=(Join-Path $fixture 'installed');releases=$receipts;channelManifest=(Join-Path $fixture 'latest.json');testSigningOnly=$true;published=$false}
    $summary | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $fixture 'rehearsal.json') -Encoding utf8
    $summary | ConvertTo-Json -Depth 8
} finally { $env:OPENKODA_BUILD_TEST_DATA_DIR=$savedData }
