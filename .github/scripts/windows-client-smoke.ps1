$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or $env:RUNNER_ARCH -ne 'ARM64') {
    throw 'The client smoke requires a disposable GitHub-hosted Windows Arm64 runner.'
}

function Read-WebView2Runtime {
    $clientKey = 'SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    $registrations = foreach ($hive in 'LocalMachine','CurrentUser') {
        foreach ($view in 'Registry32','Registry64') {
            $root = [Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
            try {
                $key = $root.OpenSubKey($clientKey, $false)
                try {
                    $value = if ($null -ne $key) { $key.GetValue('pv') }
                    $version = [version]'0.0.0.0'
                    if ($value -and ![version]::TryParse($value,[ref]$version)) { throw "Unexpected WebView2 version in $hive/$view." }
                    [ordered]@{ hive = $hive; view = $view; version = $value; present = $version -gt [version]'0.0.0.0' }
                } finally { if ($null -ne $key) { $key.Dispose() } }
            } finally { $root.Dispose() }
        }
    }
    [ordered]@{
        observedAt = [DateTime]::UtcNow.ToString('o')
        present = @($registrations | Where-Object { $_.present }).Count -gt 0
        registrations = @($registrations)
    }
}

function Read-PeMachine([string]$Path) {
    $reader = [IO.BinaryReader]::new([IO.File]::OpenRead($Path))
    try {
        if ($reader.ReadUInt16() -ne 0x5a4d) { throw 'Expected an MZ header.' }
        $reader.BaseStream.Position = 0x3c
        $header = $reader.ReadUInt32()
        $reader.BaseStream.Position = $header
        if ($reader.ReadUInt32() -ne 0x4550) { throw 'Expected a PE header.' }
        '0x{0:X4}' -f $reader.ReadUInt16()
    } finally { $reader.Dispose() }
}

$receipt = [ordered]@{
    passed = $false
    stage = 'preflight'
    sourceCommit = $env:GITHUB_SHA
    workflowRun = $env:GITHUB_RUN_ID
    imageVersion = $env:ImageVersion
    webView2Observation = 'not-measured'
}
$resultPath = Join-Path $env:RUNNER_TEMP 'openkoda-client-smoke.json'
$guestPath = Join-Path $env:RUNNER_TEMP 'openkoda-release-smoke.json'
try {
    $os = Get-CimInstance Win32_OperatingSystem
    $architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
    $receipt.windows = [ordered]@{ caption = $os.Caption; version = $os.Version; build = $os.BuildNumber; productType = $os.ProductType; architecture = $architecture }
    if ($architecture -ne 'Arm64' -or $os.ProductType -ne 1 -or [int]$os.BuildNumber -lt 22000) { throw 'Expected Windows 11 client on a native Arm64 OS.' }

    $receipt.stage = 'transfer'
    $inputDir = Join-Path $env:RUNNER_TEMP 'client-input'
    $manifestPath = Join-Path $inputDir 'transfer.json'
    $receipt.manifestSha256 = (Get-FileHash -LiteralPath $manifestPath).Hash.ToLowerInvariant()
    if ($receipt.manifestSha256 -ne $env:MANIFEST_SHA256) { throw 'The transfer manifest hash differs from the x64 build job.' }
    $transfer = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    $appRoot = Join-Path $env:GITHUB_WORKSPACE 'OpenKoda\app'
    $version = (Get-Content -LiteralPath (Join-Path $appRoot 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version
    if ($version -notmatch '^\d+\.\d+\.\d+$' -or $transfer.version -ne $version -or $transfer.commit -ne $env:GITHUB_SHA -or $transfer.runId -ne $env:GITHUB_RUN_ID) {
        throw 'The CI build does not belong to this source version and workflow run.'
    }
    $receipt.version = $version
    $release = Join-Path $appRoot 'src-tauri\target\release'
    $bundle = New-Item -ItemType Directory -Path (Join-Path $release 'bundle\nsis') -Force
    $setup = Join-Path $bundle.FullName ("OpenKoda_" + $version + "_x64-setup.exe")
    $sourceExe = Join-Path $release 'openkoda.exe'
    Copy-Item -LiteralPath (Join-Path $inputDir 'setup.exe') -Destination $setup
    Copy-Item -LiteralPath (Join-Path $inputDir 'openkoda.exe') -Destination $sourceExe
    $receipt.installerSha256 = (Get-FileHash -LiteralPath $setup).Hash.ToLowerInvariant()
    $receipt.sourceExeSha256 = (Get-FileHash -LiteralPath $sourceExe).Hash.ToLowerInvariant()
    if ($receipt.installerSha256 -ne $transfer.setupSha256 -or $receipt.sourceExeSha256 -ne $transfer.sourceExeSha256) { throw 'An executable changed in transit.' }
    $receipt.installerPeMachine = Read-PeMachine $setup
    $receipt.appPeMachine = Read-PeMachine $sourceExe
    if ($receipt.appPeMachine -ne '0x8664') { throw 'The release payload must be an AMD64 executable.' }
    # The NSIS launcher may itself be 32-bit. The guest compares the installed AMD64 payload to the x64 build.
    $local = New-Item -ItemType Directory -Path (Join-Path $appRoot '.local') -Force
    [ordered]@{ version = $version; profile = 'release'; securityRuntime = $transfer.securityRuntime; files = @(@{path = $setup; sha256 = $receipt.installerSha256}) } |
        ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $local.FullName 'package-windows.json') -Encoding utf8

    $receipt.stage = 'installed-client'
    $receipt.webView2Before = Read-WebView2Runtime
    try {
        & (Join-Path $appRoot 'scripts\sandbox-release-guest.ps1') -GitHubHosted
    } finally {
        $receipt.webView2After = Read-WebView2Runtime
        if ($receipt.webView2Before.present -and $receipt.webView2After.present) { $receipt.webView2Observation = 'pre-existing-runtime' }
        elseif ($receipt.webView2After.present) { $receipt.webView2Observation = 'runtime-appeared-during-install-test' }
        else { $receipt.webView2Observation = 'runtime-absent-after-test' }
    }
    $guest = Get-Content -Raw -LiteralPath $guestPath | ConvertFrom-Json
    if ([version]$version -ge [version]'0.2.2') {
        if (@($guest.starts | Where-Object { $_.unifiedUsageNavigation -eq $true -and $_.modelRequired -eq $true }).Count -ne 2 -or !$guest.reviewLicenseSha256 -or !$guest.reviewGuideSha256) {
            throw 'The installed client is missing evidence for usage history, analytics, model admission or the offline SDK notices.'
        }
    }
    $persisted = if ([version]$version -ge [version]'0.2.0') { $guest.settingsReopened } else { $guest.projectReopened }
    if (!$receipt.webView2After.present -or !$guest.passed -or !$persisted -or !$guest.uninstalled -or !$guest.databasePreserved -or @($guest.starts).Count -ne 2 -or $guest.sourceSha256 -ne $receipt.sourceExeSha256 -or $guest.installerSha256 -ne $receipt.installerSha256) {
        throw 'The Windows client and the guest receipts do not confirm the complete installed release test.'
    }
    $receipt.stage = 'completed'
    $receipt.passed = $true
} catch {
    $receipt.error = $_.Exception.Message
    throw
} finally {
    $json = $receipt | ConvertTo-Json -Depth 6
    $json | Set-Content -LiteralPath $resultPath -Encoding utf8
    $json | Write-Output
    Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Value @('~~~json',$json,'~~~')
}
