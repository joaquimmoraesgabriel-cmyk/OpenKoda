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
function Find-WindowControl($App,$Type,[string]$Name,[switch]$Prefix,[switch]$IncludeDisabled) {
    $deadline = [DateTime]::UtcNow.AddSeconds(35)
    $typeFilter = [System.Windows.Automation.PropertyCondition]::new(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,$Type)
    $names = @()
    do {
        $App.Refresh()
        if ($App.HasExited) { throw 'Installed app exited before its controls appeared.' }
        if ($App.MainWindowHandle -ne [IntPtr]::Zero) {
            $window = [System.Windows.Automation.AutomationElement]::FromHandle($App.MainWindowHandle)
            $items = $window.FindAll([System.Windows.Automation.TreeScope]::Descendants,$typeFilter)
            $names = @(foreach ($item in $items) { $item.Current.Name })
            foreach ($item in $items) {
                $caption = $item.Current.Name
                if (($caption -eq $Name -or ($Prefix -and $caption.StartsWith($Name,[StringComparison]::Ordinal))) -and ($IncludeDisabled -or $item.Current.IsEnabled)) { return $item }
            }
        }
        Start-Sleep -Milliseconds 200
    } until ([DateTime]::UtcNow -ge $deadline)
    throw "Missing control '$Name' in installed app. Available: $(@($names | Select-Object -First 18) -join ', ')"
}
function Invoke-WindowButton($App,[string]$Name,[switch]$Prefix) {
    $element = Find-WindowControl $App ([System.Windows.Automation.ControlType]::Button) $Name -Prefix:$Prefix
    $pattern = $null
    if ($element.TryGetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern,[ref]$pattern)) {
        $pattern.Invoke()
    } elseif ($element.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern,[ref]$pattern)) {
        # WebView2 exposes buttons with aria-pressed as toggle buttons.
        $pattern.Toggle()
    } elseif ($element.TryGetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern,[ref]$pattern)) {
        if ($pattern.Current.ExpandCollapseState -eq [System.Windows.Automation.ExpandCollapseState]::Expanded) {
            $pattern.Collapse()
        } else {
            $pattern.Expand()
        }
    } else {
        $supported = @($element.GetSupportedPatterns() | ForEach-Object ProgrammaticName) -join ', '
        throw "Cannot activate button '$Name'; available UI Automation patterns: $supported."
    }
}
function Read-ProjectFixture([string]$Profile) {
    $reader = @'
import json, sqlite3, sys
from pathlib import Path
db = Path(sys.argv[1]) / 'openkoda.sqlite3'
with sqlite3.connect(db.as_uri() + '?mode=ro', uri=True) as conn:
    state = json.loads(conn.execute('SELECT json FROM app_state WHERE id=1').fetchone()[0])
print(json.dumps({'projects': [{k: p[k] for k in ('name','id','target','kind')} for p in state['projects']], 'assessments': len(state['assessments']), 'timeoutSeconds': state['settings'].get('timeoutSeconds')}))
'@
    $raw = & python -c $reader $Profile
    if ($LASTEXITCODE -ne 0) { throw 'Could not read the installed fixture database.' }
    $raw | ConvertFrom-Json
}
function Test-InstalledSourceWindow($App,[int]$Iteration,[string]$DataRoot,[string]$Target,[int]$ExpectedTimeout,[bool]$CheckUnifiedUsage) {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    $button = [System.Windows.Automation.ControlType]::Button
    $valuePattern = [System.Windows.Automation.ValuePattern]::Pattern
    $null = Find-WindowControl $App ([System.Windows.Automation.ControlType]::RadioButton) 'Pasta local' -Prefix
    $before = Read-ProjectFixture $DataRoot
    if (@($before.projects).Count -or $before.assessments -ne 0 -or $null -eq $before.timeoutSeconds) { throw 'Unexpected native state in the clean source edition profile.' }
    Invoke-WindowButton $App 'Criar projeto'
    $folder = Find-WindowControl $App ([System.Windows.Automation.ControlType]::Edit) 'Pasta do projeto'
    $folder.GetCurrentPattern($valuePattern).SetValue($Target)
    $deadline = [DateTime]::UtcNow.AddSeconds(8)
    do {
        $folderValue = $folder.GetCurrentPattern($valuePattern).Current.Value
        if ($folderValue -ceq $Target) { break }
        Start-Sleep -Milliseconds 100
    } until ([DateTime]::UtcNow -ge $deadline)
    if ($folderValue -cne $Target) { throw "The Windows UI did not retain the fixture path (observed length $($folderValue.Length), expected $($Target.Length))." }
    $null = Find-WindowControl $App $button 'Escolher modelo · Nenhum selecionado'
    $submit = Find-WindowControl $App $button 'Criar projeto' -IncludeDisabled
    if ($submit.Current.IsEnabled) { throw 'The source scan could start without a configured model.' }

    Invoke-WindowButton $App 'Configurações'
    if ($CheckUnifiedUsage) {
        Invoke-WindowButton $App 'Histórico de uso'
        $null = Find-WindowControl $App $button 'Atualizar histórico'
        Invoke-WindowButton $App 'Análises'
        $null = Find-WindowControl $App $button 'Atualizar orçamento e preços'
        Invoke-WindowButton $App 'Gerenciar limites e preços'
        $null = Find-WindowControl $App $button 'Fechar limites e preços'
        Invoke-WindowButton $App 'Histórico'
        $null = Find-WindowControl $App $button 'Atualizar histórico'
    }
    Invoke-WindowButton $App 'Permissões'
    $control = Find-WindowControl $App ([System.Windows.Automation.ControlType]::Spinner) 'Tempo por operação (segundos)'
    $range = $control.GetCurrentPattern([System.Windows.Automation.RangeValuePattern]::Pattern)
    if ($range.Current.Value -ne $before.timeoutSeconds) { throw 'The displayed operation timeout differs from SQLite.' }
    if ($Iteration -eq 1) {
        $ExpectedTimeout = if ($before.timeoutSeconds -eq 97) { 98 } else { 97 }
        $range.SetValue($ExpectedTimeout)
        Invoke-WindowButton $App 'Salvar alterações'
        $deadline = [DateTime]::UtcNow.AddSeconds(12)
        do {
            $after = Read-ProjectFixture $DataRoot
            if ($after.timeoutSeconds -eq $ExpectedTimeout) { break }
            Start-Sleep -Milliseconds 150
        } until ([DateTime]::UtcNow -ge $deadline)
    } else { $after = $before }
    if ($after.timeoutSeconds -ne $ExpectedTimeout -or $range.Current.Value -ne $ExpectedTimeout -or @($after.projects).Count -or $after.assessments -ne 0) { throw 'The UI preference was not persisted or the empty project graph changed.' }
    return [pscustomobject]@{
        iteration=$Iteration; view='Permissões'; projects=0; assessments=0; timeoutSeconds=$ExpectedTimeout
        modelRequired=$true; settingsReopened=($Iteration -eq 2); unifiedUsageNavigation=$CheckUnifiedUsage; transport='Windows UI Automation'
    }
}
function Get-ProjectCreationExpression([string]$Kind,[string]$Target) {
    $envelope = @{request=@{action='project.create';payload=@{name='Startup fixture';kind=$Kind;target=$Target;description='Disposable release verification'}}}
    $payload = ConvertTo-Json -InputObject $envelope -Depth 5 -Compress
    return "window.__TAURI_INTERNALS__.invoke('request',$payload).then(p=>p.id)"
}
function Test-InstalledWindow($App,[int]$Iteration,[string]$Profile,[string]$Target,[string]$ExpectedId) {
    Add-Type -AssemblyName UIAutomationClient
    Add-Type -AssemblyName UIAutomationTypes
    Invoke-WindowButton $App 'Configurações'
    Invoke-WindowButton $App 'Abrir área técnica'
    Invoke-WindowButton $App 'Projetos'
    $state = Read-ProjectFixture $Profile
    if ($state.assessments -ne 0 -or @($state.projects).Count -ne ($Iteration-1)) { throw 'Unexpected initial native state.' }
    if ($Iteration -eq 1) {
        Invoke-WindowButton $App 'Novo projeto'
        foreach ($field in @(@('Nome do projeto','Startup fixture'),@('Alvo',$Target))) {
            $control = Find-WindowControl $App ([System.Windows.Automation.ControlType]::Edit) $field[0]
            $control.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($field[1])
        }
        Invoke-WindowButton $App 'Criar projeto'
    } else {
        if ($state.projects[0].id -ne $ExpectedId) { throw 'Project identity changed after reopening.' }
        Invoke-WindowButton $App 'Startup fixture' -Prefix
    }
    $null = Find-WindowControl $App ([System.Windows.Automation.ControlType]::Button) 'Nova avaliação neste projeto'
    $after = Read-ProjectFixture $Profile
    $saved = @($after.projects)
    $uuid = [guid]::Empty
    if ($saved.Count -ne 1 -or $after.assessments -ne 0 -or $saved[0].name -ne 'Startup fixture' -or $saved[0].kind -ne 'code' -or $saved[0].target -ne $Target -or -not [guid]::TryParse($saved[0].id,[ref]$uuid)) { throw 'Installed project did not match the UI fixture.' }
    return $uuid.ToString()
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
        $releaseRoot = if ($manifest.releaseRoot) { $manifest.releaseRoot } else { Join-Path $appRoot 'src-tauri\target\release' }
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
    if ([version]$manifest.version -ge [version]'0.2.0') {
        $result.stage = 'installed-sdk'
        $runtime = Join-Path $destination 'security-runtime'
        foreach ($entry in @($destination,$runtime,(Join-Path $runtime 'bin'),(Join-Path $runtime 'bin/node.exe'))) {
            if ((Get-Item -LiteralPath $entry).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Redirected installed runtime.' }
        }
        if (Get-ChildItem -LiteralPath $runtime -Force -Recurse -Attributes ReparsePoint | Select-Object -First 1) { throw 'Redirected component in installed runtime.' }
        $node = Join-Path $runtime 'bin/node.exe'
        $tree = & $node (Join-Path $PSScriptRoot 'security-runtime-inventory.mjs') $runtime
        if ($LASTEXITCODE -ne 0) { throw 'Could not hash installed SDK files.' }
        $result.securityRuntime = $tree | ConvertFrom-Json
        if ($manifest.securityRuntime.sha256 -notmatch '^[a-f0-9]{64}$' -or $result.securityRuntime.sha256 -cne $manifest.securityRuntime.sha256 -or $result.securityRuntime.files -ne $manifest.securityRuntime.files -or $result.securityRuntime.bytes -ne $manifest.securityRuntime.bytes) { throw 'Installed SDK resources differ from the build.' }
        if ([version]$manifest.version -ge [version]'0.2.2') {
            $sdkPackage = Join-Path $runtime 'node_modules/@openai/codex-security'
            $result.reviewGuideSha256 = (Get-FileHash -LiteralPath (Join-Path $sdkPackage '_bundled_plugin/skills/attack-path-analysis/references/attack-path-facts.md')).Hash.ToLowerInvariant()
            $result.reviewLicenseSha256 = (Get-FileHash -LiteralPath (Join-Path $sdkPackage 'LICENSE')).Hash.ToLowerInvariant()
            if ($result.reviewGuideSha256 -cne '1979ea81991ba1c095c1846d4b8e09429edc6295912f87a6ebb7fa7e32651a1e' -or $result.reviewLicenseSha256 -cne 'd17f227e4df5da1600391338865ce0f3055211760a36688f816941d58232d8dc') {
                throw 'The installed review guide or its offline license differs from the pinned SDK.'
            }
        }
        $frame = & $node (Join-Path $runtime 'cli.mjs') --check
        $checkExit = $LASTEXITCODE
        $result.sdkDiagnostic = $frame | ConvertFrom-Json
        $missing = @($result.sdkDiagnostic.missing)
        $gitAbsent = !(Test-Path -LiteralPath (Join-Path $env:ProgramFiles 'Git/cmd/git.exe') -PathType Leaf)
        if ($result.sdkDiagnostic.type -cne 'check' -or ($result.sdkDiagnostic.available -ne ($checkExit -eq 0))) { throw 'Invalid installed SDK diagnostic.' }
        if ($result.sdkDiagnostic.available -ne $true -and !($gitAbsent -and $missing.Count -eq 1 -and $missing[0] -ceq 'git')) { throw ('Installed SDK unavailable: '+($missing -join ', ')) }
        $result.sdkPrerequisiteGitMissing = !$result.sdkDiagnostic.available
    }
    $result.stage = 'launch'
    $cacheRoot = if ($GitHubHosted) { $runnerTemp } else { $env:TEMP }
    $browserCache = Join-Path $cacheRoot ('openkoda-webview-'+[guid]::NewGuid())
    $project = $null
    $sourceEdition = [version]$manifest.version -ge [version]'0.2.0'
    $result.projectReopened = $false
    $result.settingsReopened = $false
    $savedTimeout = 0
    if ($sourceEdition) {
        $sourceFolder = Join-Path $cacheRoot ('openkoda-source-'+[guid]::NewGuid())
        $null = New-Item -ItemType Directory -Path $sourceFolder -ErrorAction Stop
    }
    $probe = @'
(async () => {
  const main = document.querySelector('main');
  if (location.hostname !== 'tauri.localhost' || !main || main.getAttribute('aria-busy') !== 'false') return null;
  const request = (action,payload={}) => window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}});
  const state = await request('snapshot');
  return {projects:state.projects.map(p=>({id:p.id,name:p.name,kind:p.kind,target:p.target})),heading:main.querySelector('h1')?.textContent??'',assessments:state.assessments.length};
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
        if ($GitHubHosted) { $launch.EnvironmentVariables['WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS'] = '--force-renderer-accessibility' }
        $app = [Diagnostics.Process]::Start($launch)
        if ($GitHubHosted) {
            try {
                if ($sourceEdition) {
                    $observation = Test-InstalledSourceWindow $app $iteration $profileRoot $sourceFolder $savedTimeout ([version]$manifest.version -ge [version]'0.2.2')
                    $savedTimeout = $observation.timeoutSeconds
                    $result.starts += $observation
                    $result.settingsReopened = $observation.settingsReopened
                } else {
                    $project = Test-InstalledWindow $app $iteration $profileRoot $destination $project
                    $result.starts += @{iteration=$iteration;view='Projetos';projects=($iteration-1);verifiedProjectId=$project;transport='Windows UI Automation'}
                    if ($iteration -eq 2) { $result.projectReopened = $true }
                }
            } catch {
                $result.diagnostics = Get-StartupDiagnostics $app $port $browserCache $profileRoot $null
                throw
            }
            $app.Kill(); if (-not $app.WaitForExit(10000)) { throw 'Owned process failed to exit.' }
            $app.Dispose(); $app = $null
            continue
        }
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
            $kind = if ($sourceEdition) { 'code' } else { 'web' }
            $target = if ($sourceEdition) { $sourceFolder } else { 'http://127.0.0.1/' }
            $project = Read-Renderer (Get-ProjectCreationExpression $kind $target)
            $projectId = [guid]::Empty
            if (-not ([guid]::TryParse($project,[ref]$projectId))) { throw 'Native project creation failed.' }
        } else {
            if (@($state.projects).Count -ne 1 -or $state.projects[0].id -ne $project -or $state.projects[0].name -ne 'Startup fixture' -or $state.projects[0].kind -ne $kind -or ($sourceEdition -and $state.projects[0].target -ne $target)) { throw 'Saved project did not survive release restart.' }
            $result.projectReopened = $true
        }
        $result.starts += @{iteration=$iteration;heading=$state.heading;projects=@($state.projects).Count}
        $client.Dispose(); $client = $null
        $app.Kill(); if (-not $app.WaitForExit(10000)) { throw 'Owned process failed to exit.' }; $app.Dispose(); $app = $null
    }
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
