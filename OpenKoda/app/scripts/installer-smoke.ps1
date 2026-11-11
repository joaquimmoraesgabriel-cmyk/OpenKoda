param([string]$Installer)
$ErrorActionPreference = 'Stop'
$appDirectory = Split-Path -Parent $PSScriptRoot
if (-not $Installer) {
    $packageReceipt = Get-Content -LiteralPath (Join-Path $appDirectory '.local/package-windows.json') -Raw | ConvertFrom-Json
    if ($packageReceipt.profile -ne 'release' -or @($packageReceipt.files).Count -ne 1) { throw 'Select one verified release installer.' }
    $Installer = $packageReceipt.files[0].path
    if ((Get-FileHash -LiteralPath $Installer -Algorithm SHA256).Hash.ToLowerInvariant() -ne $packageReceipt.files[0].sha256) { throw 'Installer differs from package receipt.' }
}
$installerPath = (Resolve-Path -LiteralPath $Installer).Path
$workspacePath = (Resolve-Path -LiteralPath $appDirectory).Path.TrimEnd('\') + '\'
if (-not $installerPath.StartsWith($workspacePath,[StringComparison]::OrdinalIgnoreCase)) { throw 'Installer must be inside the application workspace.' }
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda'
$locationKey = 'HKCU:\Software\openkoda\OpenKoda'
if ((Test-Path -LiteralPath $uninstallKey) -or (Test-Path -LiteralPath $locationKey)) { throw 'Existing installation registration found. Nothing was changed.' }
if (Get-Process openkoda -ErrorAction SilentlyContinue) { throw 'Close OpenKoda before testing the installer.' }
$testDirectory = Join-Path $appDirectory ('.local/installer-smoke-' + [guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $testDirectory | Out-Null
$installDirectory = [IO.Path]::GetFullPath((Join-Path $testDirectory 'installed'))
if (-not $installDirectory.StartsWith($workspacePath,[StringComparison]::OrdinalIgnoreCase)) { throw 'Test installation escaped the workspace.' }
function Get-DataFingerprint {
    $dataRoot = Join-Path $env:USERPROFILE '.openkoda'
    $fingerprints = [ordered]@{}
    if (Test-Path -LiteralPath $dataRoot) {
        Get-ChildItem -LiteralPath $dataRoot -File | Where-Object { $_.Name -match '\.sqlite3(-wal|-shm)?$|^active-profile\.json$' } | Sort-Object Name | ForEach-Object {
            $fingerprints[$_.Name] = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash
        }
    }
    return ($fingerprints | ConvertTo-Json -Compress)
}
$before = Get-DataFingerprint
$before | Set-Content -LiteralPath (Join-Path $testDirectory 'data-fingerprint-before.json') -Encoding utf8
$result = [ordered]@{ passed=$false; directory=$testDirectory; installer=$installerPath; installerSha256=(Get-FileHash -LiteralPath $installerPath -Algorithm SHA256).Hash.ToLowerInvariant(); installed=$false; uninstalled=$false; existingDataPreserved=$false }
try {
    # /D is the final NSIS argument. Paths generated here have no whitespace.
    if ($installDirectory -match '\s') { throw 'This fixture requires a workspace path without whitespace.' }
    $installProcess = Start-Process -FilePath $installerPath -ArgumentList @('/S','/NS',"/D=$installDirectory") -PassThru -WindowStyle Hidden
    if (-not $installProcess.WaitForExit(120000)) { throw 'Installer did not finish. Inspect its own process before taking further action.' }
    if ($installProcess.ExitCode -ne 0) { throw "Installer failed: $($installProcess.ExitCode)" }
    $installedBinary = Join-Path $installDirectory 'openkoda.exe'
    $installedUninstaller = Join-Path $installDirectory 'uninstall.exe'
    if (-not (Test-Path -LiteralPath $installedBinary) -or -not (Test-Path -LiteralPath $installedUninstaller)) { throw 'Installed executable or uninstaller is missing.' }
    $expectedBinary = Join-Path $appDirectory 'src-tauri/target/release/openkoda.exe'
    $verification = & python (Join-Path $PSScriptRoot 'verify-bundled-binary.py') $expectedBinary $installedBinary
    if ($LASTEXITCODE -ne 0) { throw 'Installed executable does not match the packaged release.' }
    $result.binaryVerification = $verification | ConvertFrom-Json
    $registration = Get-ItemProperty -LiteralPath $uninstallKey
    if ($registration.InstallLocation.Trim('"') -ne $installDirectory) { throw 'Installer registration has an unexpected location.' }
    $result.installed = $true
    $result.installedBinarySha256 = (Get-FileHash -LiteralPath $installedBinary -Algorithm SHA256).Hash.ToLowerInvariant()
    if ((Get-DataFingerprint) -ne $before) { throw 'Existing data changed during installation.' }
    # Exercise the NSIS replacement path against this task-owned installation.
    # Same-version repair is deliberately distinct from an untested future-version migration.
    $repairProcess = Start-Process -FilePath $installerPath -ArgumentList @('/S','/NS',"/D=$installDirectory") -PassThru -WindowStyle Hidden
    if (-not $repairProcess.WaitForExit(120000) -or $repairProcess.ExitCode -ne 0) { throw 'Same-version replacement did not complete successfully.' }
    $verification = & python (Join-Path $PSScriptRoot 'verify-bundled-binary.py') $expectedBinary $installedBinary
    if ($LASTEXITCODE -ne 0) { throw 'Replacement executable differs from the packaged release.' }
    if ((Get-DataFingerprint) -ne $before) { throw 'Existing data changed during same-version replacement.' }
    if ((Get-ItemProperty -LiteralPath $uninstallKey).InstallLocation.Trim('"') -ne $installDirectory) { throw 'Replacement registration moved outside the fixture.' }
    $result.sameVersionReplacement = $true
    # Start the exact installed binary twice. This checks logical preservation
    # across startup; SQLite may legitimately checkpoint its physical files.
    $startupOutput = & node (Join-Path $PSScriptRoot 'installed-startup-smoke.mjs') $installedBinary
    if ($LASTEXITCODE -ne 0) { throw 'Installed application startup/preservation check failed.' }
    $startupReceipt = ($startupOutput -join "`n") | ConvertFrom-Json
    if (-not $startupReceipt.passed) { throw 'Installed application startup was not verified.' }
    $result.startup = $startupReceipt
    $beforeRemoval = Get-DataFingerprint
    # No data-removal checkbox or flag is supplied. The generated NSIS template
    # removes only its exact executable/uninstaller and matching shortcuts here.
    $uninstallProcess = Start-Process -FilePath $installedUninstaller -ArgumentList '/S' -PassThru -WindowStyle Hidden
    if (-not $uninstallProcess.WaitForExit(60000)) { throw 'Uninstaller did not finish.' }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ((Test-Path -LiteralPath $installedBinary) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 250 }
    if ((Test-Path -LiteralPath $installedBinary) -or (Test-Path -LiteralPath $uninstallKey)) { throw 'Uninstallation did not remove the task-owned executable/registration.' }
    $result.uninstalled = $true
    $result.existingDataPreserved = (Get-DataFingerprint) -eq $beforeRemoval
    if (-not $result.existingDataPreserved) { throw 'Existing data changed during uninstallation.' }
    if (Test-Path -LiteralPath $locationKey) {
        if ((Get-Item -LiteralPath $locationKey).GetValue('') -ne $installDirectory) { throw 'Installation location changed externally; registration preserved.' }
        # This exact key was absent in preflight and belongs to this fixture.
        Remove-Item -LiteralPath $locationKey
    }
    $result.passed = $true
} catch {
    $result.error = $_.Exception.Message
} finally {
    $result.generatedAt = [DateTime]::UtcNow.ToString('o')
    $result.limitations = @('Tests current-user installation, same-version replacement, startup and removal on this host, not a clean Windows matrix.', 'Does not prove future-version migration, binary rollback or publisher signing.')
    $result | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $testDirectory 'result.json') -Encoding utf8
    $result | ConvertTo-Json -Depth 8
}
if (-not $result.passed) { exit 1 }
