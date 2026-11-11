# This script runs inside a fresh Windows Sandbox. Its only writable host
# folder is C:\KodaResult, where it records the outcome of this run.
$ErrorActionPreference = 'Stop'
$result = [ordered]@{ passed = $false; stage = 'guest'; starts = @() }
$app = $null
$client = $null
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
    if ($env:USERNAME -ne 'WDAGUtilityAccount' -or -not (Test-Path -LiteralPath 'C:\KodaInput\release.json')) { throw 'Fresh Windows Sandbox is required.' }
    $manifest = Get-Content -Raw -LiteralPath 'C:\KodaInput\release.json' | ConvertFrom-Json
    $result.version = $manifest.version
    $result.installerSha256 = (Get-FileHash -LiteralPath 'C:\KodaInput\setup.exe').Hash.ToLowerInvariant()
    if ($result.installerSha256 -ne $manifest.installerSha256) { throw 'Installer does not match the selected release.' }
    $profileRoot = Join-Path ([Environment]::GetFolderPath('UserProfile')) '.openkoda'
    if (Test-Path -LiteralPath $profileRoot) { throw 'The disposable profile must be empty.' }
    $result.stage = 'install'
    $destination = Join-Path $env:LOCALAPPDATA 'OpenKodaFixture'
    $setup = Start-Process -FilePath 'C:\KodaInput\setup.exe' -ArgumentList @('/S','/NS',('/D='+$destination)) -WindowStyle Hidden -PassThru
    if (-not $setup.WaitForExit(180000) -or $setup.ExitCode -ne 0) { throw 'Installation did not complete successfully.' }
    $exe = Join-Path $destination 'openkoda.exe'
    if ((Get-FileHash -LiteralPath $exe).Hash.ToLowerInvariant() -ne $manifest.payloadSha256) { throw 'Installed payload hash mismatch.' }
    $result.stage = 'launch'
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
        $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-address=127.0.0.1 --remote-debugging-port=$port"
        $app = Start-Process -FilePath $exe -WindowStyle Hidden -PassThru
        $deadline = [DateTime]::UtcNow.AddSeconds(75)
        $tab = $null
        do {
            if ($app.HasExited) { throw 'Release executable exited during startup.' }
            try { $tab = @(Invoke-RestMethod -Uri "http://127.0.0.1:$port/json/list" -TimeoutSec 2 | ForEach-Object { $_ } | Where-Object { $_.type -eq 'page' -and $_.url -match '^https?://tauri\.localhost/' })[0] } catch {}
            if (-not $tab) { Start-Sleep -Milliseconds 250 }
        } until ($tab -or [DateTime]::UtcNow -gt $deadline)
        if (-not $tab) { throw 'Installed WebView did not reach the local application origin.' }
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
        $result.starts += @{iteration=$iteration;heading=$state.heading;pid=$app.Id}
        $client.Dispose(); $client = $null
        $app.Kill(); if (-not $app.WaitForExit(10000)) { throw 'Owned process failed to exit.' }; $app.Dispose(); $app = $null
    }
    $result.stage = 'completed'
    $result.passed = $true
} catch { $result.error = $_.Exception.Message }
finally {
    if ($client) { $client.Dispose() }
    if ($app -and -not $app.HasExited) { $app.Kill() }
    if ($env:USERNAME -eq 'WDAGUtilityAccount' -and (Test-Path -LiteralPath 'C:\KodaResult')) {
        $result.generatedAt = [DateTime]::UtcNow.ToString('o')
        $result | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath 'C:\KodaResult\result.json' -Encoding UTF8
    }
}
