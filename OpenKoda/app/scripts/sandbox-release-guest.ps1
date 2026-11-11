# Run in a fresh Windows Sandbox or in the hosted Windows release workflow.
param([switch]$GitHubHosted)
$ErrorActionPreference = 'Stop'
$result = [ordered]@{ passed = $false; stage = 'preflight'; starts = @() }
$receiptPath = $null
$app = $null
$client = $null
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda'
$locationKey = 'HKCU:\Software\openkoda\OpenKoda'
function Database-Fingerprint([string]$Root) {
    $files = @(Get-ChildItem -LiteralPath $Root -File | Where-Object { $_.Name -match '\.sqlite3(-wal|-shm)?$|^active-profile\.json$' } | Sort-Object Name)
    if ($files.Name -notcontains 'openkoda.sqlite3') { throw 'Missing persisted application database.' }
    return ($files | ForEach-Object { $_.Name + ' ' + (Get-FileHash -LiteralPath $_.FullName).Hash }) -join "`n"
}
function Assert-CleanInstallState {
    foreach ($key in @($uninstallKey,$locationKey,'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda','HKLM:\Software\openkoda\OpenKoda')) {
        if (Test-Path -LiteralPath $key) { throw 'Unexpected OpenKoda installation registration.' }
    }
    foreach ($folder in 'Desktop','CommonDesktopDirectory','Programs','CommonPrograms') {
        if (Test-Path -LiteralPath (Join-Path ([Environment]::GetFolderPath($folder)) 'OpenKoda.lnk')) { throw 'Unexpected OpenKoda shortcut.' }
    }
    if (Get-Process -Name 'openkoda','openkoda-debug' -ErrorAction SilentlyContinue) { throw 'Unexpected running OpenKoda process.' }
}
function Read-LocalTargets([int]$Port) {
    # The browser and the probe share a loopback port on this guest. A system proxy
    # must never handle these requests or supply the response to the smoke.
    $request = [Net.HttpWebRequest]::Create("http://127.0.0.1:$Port/json/list")
    $request.Proxy = $null
    $request.Timeout = 1500
    $response = $request.GetResponse()
    try {
        $reader = [IO.StreamReader]::new($response.GetResponseStream())
        try { $reader.ReadToEnd() | ConvertFrom-Json } finally { $reader.Dispose() }
    } finally { $response.Dispose() }
}
function Get-StartupDiagnostics($App,[int]$Port,[string]$Cache,[string]$Profile,$Observations) {
    $App.Refresh()
    $children = @(Get-CimInstance Win32_Process -Filter "Name = 'msedgewebview2.exe'" -ErrorAction SilentlyContinue |
        Where-Object { $_.ParentProcessId -eq $App.Id -or ($_.CommandLine -and $_.CommandLine.Contains($Cache)) })
    return [ordered]@{
        appSession = $App.SessionId
        runnerSession = (Get-Process -Id $PID).SessionId
        appHasWindow = $App.MainWindowHandle -ne [IntPtr]::Zero
        appWindowTitle = $App.MainWindowTitle
        databaseCreated = Test-Path -LiteralPath (Join-Path $Profile 'openkoda.sqlite3')
        browserCacheCreated = Test-Path -LiteralPath $Cache
        browserProcesses = $children.Count
        browserWithDebugPort = @($children | Where-Object { $_.CommandLine -like "*--remote-debugging-port=$Port*" }).Count
        discovery = $Observations
    }
}
function Read-Renderer([string]$Expression) {
    $token = [Threading.CancellationTokenSource]::new(15000)
    try {
        $packet = @{id=1; method='Runtime.evaluate'; params=@{expression=$Expression; awaitPromise=$true; returnByValue=$true}} | ConvertTo-Json -Compress
        $out = [ArraySegment[byte]]::new([Text.Encoding]::UTF8.GetBytes($packet))
        $null = $client.SendAsync($out,[Net.WebSockets.WebSocketMessageType]::Text,$true,$token.Token).GetAwaiter().GetResult()
        do {
            $stream = [IO.MemoryStream]::new()
            try {
                do {
                    $buffer = [byte[]]::new(8192)
                    $message = $client.ReceiveAsync([ArraySegment[byte]]::new($buffer),$token.Token).GetAwaiter().GetResult()
                    if ($message.MessageType -ne 'Text' -or $stream.Length + $message.Count -gt 1048576) { throw 'Invalid debugger response.' }
                    $stream.Write($buffer,0,$message.Count)
                } until ($message.EndOfMessage)
                $response = [Text.Encoding]::UTF8.GetString($stream.ToArray()) | ConvertFrom-Json
            } finally { $stream.Dispose() }
        } until ($response.id -eq 1)
        if ($response.error -or $response.result.exceptionDetails) { throw 'Native renderer check failed.' }
        $response.result.result.value
    } finally { $token.Dispose() }
}
try {
    if ($GitHubHosted) {
        if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted' -or $env:RUNNER_OS -ne 'Windows' -or !$env:GITHUB_WORKSPACE -or !$env:RUNNER_TEMP) { throw 'A disposable GitHub-hosted Windows runner is required.' }
        $workspace = (Get-Item -LiteralPath $env:GITHUB_WORKSPACE).FullName
        $runnerTemp = (Get-Item -LiteralPath $env:RUNNER_TEMP).FullName
        $appRoot = Join-Path $workspace 'OpenKoda\app'
        if ($PSScriptRoot -ne (Join-Path $appRoot 'scripts') -or $runnerTemp -eq $workspace -or $runnerTemp.StartsWith($workspace+'\',[StringComparison]::OrdinalIgnoreCase) -or $runnerTemp -eq [IO.Path]::GetPathRoot($runnerTemp)) { throw 'Unexpected workflow workspace or temporary directory.' }
        $receiptPath = Join-Path $runnerTemp 'openkoda-release-smoke.json'
        $manifest = Get-Content -Raw -LiteralPath (Join-Path $appRoot '.local\package-windows.json') | ConvertFrom-Json
        if ($manifest.profile -ne 'release' -or $manifest.version -notmatch '^\d+\.\d+\.\d+$' -or @($manifest.files).Count -ne 1) { throw 'Expected one packaged release.' }
        $releaseRoot = Join-Path $appRoot 'src-tauri\target\release'
        $installer = Join-Path $releaseRoot ('bundle\nsis\OpenKoda_'+$manifest.version+'_x64-setup.exe')
        if ($manifest.files[0].path -ne $installer) { throw 'Unexpected installer path in build receipt.' }
        $installerSha = $manifest.files[0].sha256
        $destination = Join-Path $runnerTemp ('openkoda-installed-'+[guid]::NewGuid())
    } else {
        if ($env:USERNAME -ne 'WDAGUtilityAccount' -or -not (Test-Path -LiteralPath 'C:\KodaInput\release.json') -or -not (Test-Path -LiteralPath 'C:\KodaResult' -PathType Container)) { throw 'Fresh Windows Sandbox is required.' }
        $receiptPath = 'C:\KodaResult\result.json'
        $manifest = Get-Content -Raw -LiteralPath 'C:\KodaInput\release.json' | ConvertFrom-Json
        $installer = 'C:\KodaInput\setup.exe'
        $installerSha = $manifest.installerSha256
        $destination = Join-Path $env:LOCALAPPDATA 'OpenKodaFixture'
    }
    $result.version = $manifest.version
    $result.installerSha256 = (Get-FileHash -LiteralPath $installer).Hash.ToLowerInvariant()
    if ($result.installerSha256 -ne $installerSha) { throw 'Installer does not match the selected release.' }
    $profileRoot = Join-Path ([Environment]::GetFolderPath('UserProfile')) '.openkoda'
    if ((Test-Path -LiteralPath $profileRoot) -or (Test-Path -LiteralPath (Join-Path $env:APPDATA 'com.openkoda.desktop\openkoda.sqlite3'))) { throw 'The disposable profile must be empty.' }
    if (Test-Path -LiteralPath $destination) { throw 'Destination must be new.' }
    Assert-CleanInstallState
    $result.stage = 'install'
    $setup = Start-Process -FilePath $installer -ArgumentList @('/S','/NS',('/D='+$destination)) -WindowStyle Hidden -PassThru
    if (-not $setup.WaitForExit(180000) -or $setup.ExitCode -ne 0) { throw 'Installation did not complete successfully.' }
    $exe = Join-Path $destination 'openkoda.exe'
    $uninstaller = Join-Path $destination 'uninstall.exe'
    $registration = Get-ItemProperty -LiteralPath $uninstallKey
    if ($registration.InstallLocation.Trim('"') -ne $destination -or $registration.UninstallString -ne ('"'+$uninstaller+'"') -or !(Test-Path -LiteralPath $uninstaller -PathType Leaf)) { throw 'Installation registration does not match the disposable destination.' }
    $result.payloadSha256 = (Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant()
    if ($GitHubHosted) {
        $verification = & python (Join-Path $PSScriptRoot 'verify-bundled-binary.py') (Join-Path $releaseRoot 'openkoda.exe') $exe
        if ($LASTEXITCODE -ne 0) { throw 'Installed executable differs beyond the NSIS bundle marker.' }
        $proof = $verification | ConvertFrom-Json
        if (!$proof.verified -or $result.payloadSha256 -ne $proof.sha256) { throw 'Installed payload hash mismatch.' }
        $result.sourceSha256 = $proof.sourceSha256
        $result.differingBytes = $proof.differingBytes
    } elseif ($result.payloadSha256 -ne $manifest.payloadSha256) { throw 'Installed payload hash mismatch.' }
    $result.stage = 'launch'
    $cacheRoot = if ($GitHubHosted) { $runnerTemp } else { $env:TEMP }
    $browserCache = Join-Path $cacheRoot ('openkoda-webview-'+[guid]::NewGuid())
    $probe = @'
(async () => {
  const main = document.querySelector('main');
  if (location.hostname !== 'tauri.localhost' || !main || main.getAttribute('aria-busy') !== 'false') return null;
  const request = (action,payload={}) => window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}});
  const state = await request('snapshot');
  return {projects:state.projects.map(p=>({id:p.id,name:p.name})),heading:main.querySelector('h1')?.textContent??'',assessments:state.assessments.length};
})()
'@
    foreach ($iteration in 1,2) {
        $server = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback,0)
        $server.Start(); $port = $server.LocalEndpoint.Port; $server.Stop()
        $launch = [Diagnostics.ProcessStartInfo]::new()
        $launch.FileName = $exe
        $launch.WorkingDirectory = $destination
        $launch.UseShellExecute = $false
        $launch.EnvironmentVariables['WEBVIEW2_USER_DATA_FOLDER'] = $browserCache
        $launch.EnvironmentVariables['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = "--remote-debugging-address=127.0.0.1 --remote-debugging-port=$port"
        $app = [Diagnostics.Process]::Start($launch)
        $deadline = [DateTime]::UtcNow.AddSeconds(75)
        $tab = $null
        $observed = [ordered]@{available=$false;targets=@();failure=$null}
        do {
            if ($app.HasExited) { throw 'Release executable exited during startup.' }
            try {
                $targets = @(Read-LocalTargets $port)
                $observed.available = $true
                $observed.targets = @($targets | ForEach-Object {
                    $url = [Uri]$_.url
                    @{type=$_.type;scheme=$url.Scheme;host=$url.Host}
                })
                $tab = $targets | Where-Object { $_.type -eq 'page' -and ([Uri]$_.url).Host -eq 'tauri.localhost' } | Select-Object -First 1
            } catch { $observed.failure = $_.Exception.GetBaseException().GetType().Name }
            if (-not $tab) { Start-Sleep -Milliseconds 250 }
        } until ($tab -or [DateTime]::UtcNow -gt $deadline)
        if (-not $tab) {
            $result.diagnostics = Get-StartupDiagnostics $app $port $browserCache $profileRoot $observed
            throw 'Installed WebView did not reach the local application origin.'
        }
        $client = [Net.WebSockets.ClientWebSocket]::new()
        $connect = [Threading.CancellationTokenSource]::new(10000)
        try { $client.ConnectAsync([Uri]$tab.webSocketDebuggerUrl,$connect.Token).GetAwaiter().GetResult() } finally { $connect.Dispose() }
        do {
            $state = Read-Renderer $probe
            if (-not $state) { Start-Sleep -Milliseconds 200 }
        } until ($state -or [DateTime]::UtcNow -gt $deadline)
        if (-not $state -or -not $state.heading -or $state.assessments -ne 0) { throw 'The native app did not return its initial state.' }
        if ($iteration -eq 1) {
            if (@($state.projects).Count -ne 0) { throw 'Unexpected projects in new profile.' }
            $project = Read-Renderer "window.__TAURI_INTERNALS__.invoke('request',{request:{action:'project.create',payload:{name:'Startup fixture',kind:'web',target:'http://127.0.0.1/',description:'Disposable release verification'}}}).then(p=>p.id)"
            $projectId = [guid]::Empty
            if (-not ([guid]::TryParse($project,[ref]$projectId))) { throw 'Native project creation failed.' }
        } elseif (@($state.projects).Count -ne 1 -or $state.projects[0].id -ne $project -or $state.projects[0].name -ne 'Startup fixture') { throw 'Saved project did not survive release restart.' }
        $result.starts += @{iteration=$iteration;heading=$state.heading;projects=@($state.projects).Count}
        $client.Dispose(); $client = $null
        $app.Kill(); if (-not $app.WaitForExit(10000)) { throw 'Owned process failed to exit.' }; $app.Dispose(); $app = $null
    }
    $result.projectReopened = $true
    $result.stage = 'uninstall'
    $beforeRemoval = Database-Fingerprint $profileRoot
    $remove = Start-Process -FilePath $uninstaller -ArgumentList '/S' -WindowStyle Hidden -PassThru
    if (!$remove.WaitForExit(120000) -or $remove.ExitCode -ne 0) { throw 'Uninstallation did not exit successfully.' }
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ((Test-Path -LiteralPath $exe) -or (Test-Path -LiteralPath $uninstaller) -or (Test-Path -LiteralPath $uninstallKey)) {
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Uninstallation left a registered package or executable.' }
        Start-Sleep -Milliseconds 200
    }
    # NSIS may retain its install-location preference. It was absent at preflight.
    if (Test-Path -LiteralPath $locationKey) {
        $key = Get-Item -LiteralPath $locationKey
        if ($key.GetValue('') -ne $destination -or $key.GetSubKeyNames().Count -or @($key.GetValueNames() | Where-Object { $_ -notin @('','Installer Language') }).Count) { throw 'Unexpected residual registration.' }
        Remove-Item -LiteralPath $locationKey
    }
    Assert-CleanInstallState
    if ((Database-Fingerprint $profileRoot) -cne $beforeRemoval) { throw 'Uninstallation changed the saved profile.' }
    $result.uninstalled = $true
    $result.databasePreserved = $true
    $result.stage = 'completed'
    $result.passed = $true
} catch {
    $result.error = $_.Exception.Message
    if ($GitHubHosted) {
        foreach ($directory in @($env:USERPROFILE,$env:GITHUB_WORKSPACE,$env:RUNNER_TEMP)) {
            if ($directory) { $result.error = $result.error.Replace($directory,'[runner]') }
        }
    }
}
finally {
    if ($client) { $client.Dispose() }
    if ($app -and -not $app.HasExited) { $app.Kill() }
    if ($receiptPath) {
        $result.generatedAt = [DateTime]::UtcNow.ToString('o')
        $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $receiptPath -Encoding UTF8
    }
}
if (!$result.passed) { throw $result.error }
