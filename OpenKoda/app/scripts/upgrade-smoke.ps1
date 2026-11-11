# Positive, current-user upgrade rehearsal. Never repairs or removes another installation.
# -SelfTest is synthetic; -ValidateOnly checks artifacts without starting an installer/app.
# -AllowSameVersionRebuild checks replacement of a rebuilt package, not schema migration.
param(
    [string]$OldReleaseManifest = (Join-Path $PSScriptRoot '../releases/0.1.0-local-20261107/release.json'),
    [string]$NewReleaseManifest = (Join-Path $PSScriptRoot '../.local/package-windows.json'),
    [string]$NewSourceBinary,
    [switch]$AllowSameVersionRebuild,
    [switch]$ValidateOnly,
    [switch]$SelfTest
)
$ErrorActionPreference = 'Stop'
$appDirectory = [IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$workspacePrefix = $appDirectory.TrimEnd('\') + '\'
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda'
$locationKey = 'HKCU:\Software\openkoda\OpenKoda'
$artifactLeases = [Collections.Generic.List[IO.FileStream]]::new()

function Resolve-AppFile([string]$Path) {
    if (-not [IO.Path]::IsPathRooted($Path)) { $Path = Join-Path $appDirectory $Path }
    $full = [IO.Path]::GetFullPath($Path)
    if (-not $full.StartsWith($workspacePrefix,[StringComparison]::OrdinalIgnoreCase) -or $full.Substring(3).Contains(':')) { throw 'Artifact path must be inside the app workspace without alternate streams.' }
    $item = Get-Item -LiteralPath $full
    if ($item.PSIsContainer) { throw 'Expected an artifact file.' }
    $node=$item
    while ($null -ne $node) {
        if (($node.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Artifact path contains a reparse point.' }
        if ($node.FullName -eq $appDirectory) { break }
        $node=if ($node -is [IO.FileInfo]) { $node.Directory } else { $node.Parent }
    }
    return $full
}
function Read-Version([string]$Value) {
    if ($Value -notmatch '^\d+\.\d+\.\d+(\.\d+)?$') { throw 'A numeric release FileVersion is required.' }
    $v=[version]$Value
    return [version]::new($v.Major,$v.Minor,$v.Build,[Math]::Max(0,$v.Revision))
}
function Assert-Upgrade($OldVersion,$NewVersion,[bool]$SameVersionRebuild = $false) {
    if ($SameVersionRebuild) {
        if ($NewVersion -ne $OldVersion) { throw 'A same-version rebuild rehearsal requires identical FileVersions.' }
        return
    }
    if ($NewVersion -le $OldVersion) { throw 'New FileVersion must be strictly greater than the preserved old release.' }
}
function Get-Sha([string]$Path) { return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant() }
function Hold-Artifact([string]$Path) {
    $artifactLeases.Add([IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read))
}
function Read-Release([string]$Manifest,[string]$SourceOverride) {
    $path=Resolve-AppFile $Manifest
    Hold-Artifact $path
    $data=Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    if ($data.profile -and $data.profile -ne 'release') { throw 'A release package manifest is required; debug packages cannot certify upgrades.' }
    $source=$SourceOverride
    if (-not $source) { $source=$data.sourceBinary }
    if (-not $source) { $source=Join-Path $appDirectory 'src-tauri/target/release/openkoda.exe' }
    $source=Resolve-AppFile $source
    Hold-Artifact $source
    $sourceHash=Get-Sha $source
    if ($data.sourceBinarySha256 -and $sourceHash -ne $data.sourceBinarySha256) { throw 'Source binary hash differs from the manifest.' }
    $version=Read-Version (Get-Item -LiteralPath $source).VersionInfo.FileVersion
    if ($data.version -and (Read-Version $data.version) -ne $version) { throw 'Manifest and source FileVersion disagree.' }
    if ($data.installer) { $candidates=@([pscustomobject]@{path=$data.installer;sha256=$data.sha256;bytes=$data.bytes}) }
    else { $candidates=@($data.files) }
    $matching=@($candidates | Where-Object {
        $candidate=Resolve-AppFile $_.path
        (Read-Version (Get-Item -LiteralPath $candidate).VersionInfo.FileVersion) -eq $version
    })
    if ($matching.Count -ne 1) { throw 'Manifest must identify exactly one installer matching the source FileVersion.' }
    $entry=$matching[0]; $installer=Resolve-AppFile $entry.path
    Hold-Artifact $installer
    if ($entry.sha256 -notmatch '^[a-fA-F0-9]{64}$' -or (Get-Sha $installer) -ne $entry.sha256 -or (Get-Item -LiteralPath $installer).Length -ne $entry.bytes) { throw 'Installer manifest hash/size validation failed.' }
    return [pscustomobject][ordered]@{manifest=$path;manifestSha256=(Get-Sha $path);installer=$installer;installerSha256=(Get-Sha $installer);bytes=$entry.bytes;sourceBinary=$source;sourceBinarySha256=$sourceHash;version=$version.ToString();authenticode=(Get-AuthenticodeSignature -LiteralPath $installer).Status.ToString()}
}
function Assert-NoApplication {
    if (Get-Process openkoda -ErrorAction SilentlyContinue) { throw 'An OpenKoda process exists. Close it before this isolated installation rehearsal.' }
}
function Assert-NoRegistration {
    foreach ($key in @($uninstallKey,$locationKey,'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda')) {
        if (Test-Path -LiteralPath $key) { throw 'An existing OpenKoda installation registration was found; no installer was started.' }
    }
}
function Get-NsisArguments([string]$Directory,[switch]$Uninstall) {
    # NSIS requires its unquoted directory argument LAST. Restrict the generated fixture.
    if ($Directory -match '[\s"]' -or -not [IO.Path]::IsPathRooted($Directory)) { throw 'This rehearsal requires an absolute workspace without whitespace or quotes.' }
    if ($Uninstall) { return @('/S',"_?=$Directory") }
    return @('/S','/NS',"/D=$Directory")
}
function Run-OwnedProcess([string]$Executable,[string[]]$Arguments,[int]$TimeoutSeconds=120) {
    $process=Start-Process -FilePath $Executable -ArgumentList $Arguments -PassThru -WindowStyle Hidden
    $watch=[Diagnostics.Stopwatch]::StartNew()
    while ($true) {
        $remaining=[Math]::Max(0,[int]($TimeoutSeconds*1000-$watch.ElapsedMilliseconds))
        if ($remaining -eq 0) { throw "Owned process $($process.Id) timed out; it was NOT killed or automatically uninstalled." }
        if ($process.WaitForExit([Math]::Min(60000,$remaining))) { break }
    }
    $code=$process.ExitCode; $pidValue=$process.Id; $process.Dispose()
    if ($code -ne 0) { throw "Owned installer/uninstaller failed with exit code $code (PID $pidValue)." }
    return [pscustomobject]@{pid=$pidValue;exitCode=$code;elapsedMs=$watch.ElapsedMilliseconds}
}
function Assert-OwnedRegistration([string]$Directory,[string]$Version) {
    $registration=Get-ItemProperty -LiteralPath $uninstallKey
    if ([IO.Path]::GetFullPath($registration.InstallLocation.Trim('"')) -ne $Directory) { throw 'Installation registration no longer points to the owned fixture.' }
    if ((Read-Version $registration.DisplayVersion) -ne (Read-Version $Version)) { throw 'Registered version is incorrect.' }
    if (-not (Test-Path -LiteralPath $locationKey) -or (Get-Item -LiteralPath $locationKey).GetValue('') -ne $Directory) { throw 'Installer location registration is incorrect.' }
    $command=$registration.UninstallString
    if ($command -notmatch '^"([^"]+)"(?:\s.*)?$') { throw 'Unexpected uninstall command format; refusing any cleanup.' }
    if ([IO.Path]::GetFullPath($Matches[1]) -ne (Join-Path $Directory 'uninstall.exe')) { throw 'Uninstaller registration is outside the owned fixture.' }
    return [pscustomobject]@{installLocation=$Directory;displayVersion=$registration.DisplayVersion;uninstaller=(Join-Path $Directory 'uninstall.exe')}
}
function Verify-Binary($Release,[string]$Installed) {
    $null=Resolve-AppFile $Installed
    $output=& python (Join-Path $PSScriptRoot 'verify-bundled-binary.py') $Release.sourceBinary $Installed
    if ($LASTEXITCODE -ne 0) { throw 'Installed binary differs beyond the verified Tauri NSIS marker.' }
    $verified=($output -join "`n") | ConvertFrom-Json
    if (-not $verified.verified -or (Read-Version (Get-Item -LiteralPath $Installed).VersionInfo.FileVersion) -ne (Read-Version $Release.version)) { throw 'Installed binary/version verification failed.' }
    return $verified
}

# Reads only metadata/hashes from the real profile. No row, source, or credential
# value is printed. SQLite logical hashes tolerate WAL checkpoints, not changed data.
$fingerprintPython=@'
import hashlib,json,os,sqlite3,sys,uuid
from pathlib import Path
def digest(v): return hashlib.sha256(json.dumps(v,sort_keys=True,separators=(',',':'),ensure_ascii=True).encode()).hexdigest()
def cell(v):
    if isinstance(v,bytes): return ['blob',hashlib.sha256(v).hexdigest()]
    if isinstance(v,str):
        try: return ['json',json.loads(v)]
        except (ValueError,TypeError): return ['text',v]
    return [type(v).__name__,v]
def plain(p):
    s=p.lstat()
    if p.is_symlink() or getattr(s,'st_file_attributes',0)&0x400: raise ValueError('Reparse point in profile fingerprint scope')
def table_hash(db,name):
    quoted='"'+name.replace('"','""')+'"'
    rows=sorted(digest([cell(v) for v in row]) for row in db.execute('SELECT * FROM '+quoted))
    return {'rows':len(rows),'sha256':digest(rows)}
if sys.argv[1]=='--self-test':
    db=sqlite3.connect(':memory:');db.execute('CREATE TABLE t (value TEXT, secret BLOB)')
    db.execute('INSERT INTO t VALUES(?,?)',('{"a":1,"b":2}',b'SYNTHETIC'))
    first=table_hash(db,'t');db.execute('UPDATE t SET value=?',('{"b":2,"a":1}',));assert table_hash(db,'t')==first
    db.execute('UPDATE t SET secret=?',(b'CHANGED',));assert table_hash(db,'t')!=first
    assert 'SYNTHETIC' not in json.dumps(first);db.close();print('{"passed":true,"tests":3}');sys.exit(0)
root=Path(sys.argv[1]);plain(root)
directories=[root]; profiles=root/'profiles'
if profiles.exists():
    plain(profiles)
    for p in sorted(profiles.iterdir()):
        plain(p)
        if not p.is_dir() or str(uuid.UUID(p.name))!=p.name: raise ValueError('Unexpected profile directory')
        directories.append(p)
result={'physical':{},'logical':{},'pointers':{},'installationMarkers':{},'profileCount':len(directories)}
budget_marker=root/'ai-budget.installation.json'
if budget_marker.exists():
    plain(budget_marker)
    if not budget_marker.is_file(): raise ValueError('Unexpected budget marker type')
    result['installationMarkers'][budget_marker.name]=hashlib.sha256(budget_marker.read_bytes()).hexdigest()
active=root;pointer=root/'active-profile.json'
if pointer.exists():
    plain(pointer); raw=pointer.read_bytes();result['pointers']['active-profile.json']=hashlib.sha256(raw).hexdigest()
    ref=json.loads(raw)['profile']
    if ref['kind']=='profile':
        assert str(uuid.UUID(ref['id']))==ref['id'];active=root/'profiles'/ref['id'];plain(active)
    else: assert ref['kind']=='base'
if not (active/'openkoda.sqlite3').is_file() or not (active/'assistant.sqlite3').is_file(): raise ValueError('An initialized existing profile is required; this test does not create one')
for directory in directories:
    for path in sorted(directory.iterdir()):
        if not (path.name.endswith(('.sqlite3','.sqlite3-wal','.sqlite3-shm'))): continue
        plain(path)
        if not path.is_file(): raise ValueError('Unexpected database path type')
        key=path.relative_to(root).as_posix()
        if not path.name.endswith('.sqlite3'): continue
        db=sqlite3.connect(path.as_uri()+'?mode=ro',uri=True);db.execute('PRAGMA query_only=ON')
        if db.execute('PRAGMA quick_check').fetchone()!=('ok',): raise ValueError('Profile database failed integrity check')
        tables=[r[0] for r in db.execute("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
        result['logical'][key]={'userVersion':db.execute('PRAGMA user_version').fetchone()[0],'tables':{n:table_hash(db,n) for n in tables}}
        if directory==root and path.name=='activation.sqlite3' and 'activation_meta' in tables:
            if db.execute("SELECT count(*) FROM activation_meta WHERE key='pending'").fetchone()[0]: raise ValueError('Pending profile activation must be resolved before this read-only startup rehearsal')
        if directory==active:
            if 'app_state' in tables:
                state=json.loads(db.execute('SELECT json FROM app_state WHERE id=1').fetchone()[0])
                for group in ('assessments','plans','executions'):
                    if any(x.get('status') in {'preparing','planning','queued','running','pausing','validating','canceling','approved'} for x in state.get(group,[])): raise ValueError('Startup would reconcile active domain work')
            if 'requests' in tables:
                if db.execute("SELECT count(*) FROM requests WHERE status='pending'").fetchone()[0]: raise ValueError('Pending provider request must be resolved first')
                columns={r[1] for r in db.execute('PRAGMA table_info(requests)')}
                if 'details' in columns and db.execute("SELECT count(*) FROM requests WHERE json_extract(details,'$.operationProposal.status')='pending'").fetchone()[0]: raise ValueError('Startup would reconcile an incomplete operation proposal')
            for name in ('investigations','remediation_plans','retention_plans','project_purge_plans','bridge_runs'):
                if name in tables:
                    for (raw,) in db.execute('SELECT data FROM "'+name+'"'):
                        if json.loads(raw).get('status') in {'prepared','approved','running','canceling','applying','retesting','confirming','starting'}: raise ValueError('Startup would change an existing journal; review it first')
        db.close()
# Capture physical files after closing all readers. SHM is transient coordination
# state; startup comparisons use logical content and record schema additions.
for directory in directories:
    for path in sorted(directory.iterdir()):
        if not path.name.endswith(('.sqlite3','.sqlite3-wal')): continue
        plain(path)
        if not path.is_file(): raise ValueError('Unexpected database path type')
        result['physical'][path.relative_to(root).as_posix()]=hashlib.sha256(path.read_bytes()).hexdigest()
print(json.dumps(result,sort_keys=True,separators=(',',':')))
'@
function Get-Fingerprint {
    $output=& python -c $fingerprintPython (Join-Path $env:USERPROFILE '.openkoda')
    if ($LASTEXITCODE -ne 0) { throw 'Read-only profile fingerprint/preflight failed; inspect the bounded diagnostic above.' }
    return (($output -join "`n") | ConvertFrom-Json)
}
function Canonical($Value) { return (ConvertTo-Json -InputObject $Value -Depth 30 -Compress) }
function Assert-Fingerprint($Before,$After,[switch]$Startup) {
    if ((Canonical $Before.pointers) -ne (Canonical $After.pointers) -or $Before.profileCount -ne $After.profileCount) { throw 'Profile selection or profile inventory changed.' }
    if (-not $Startup -and (Canonical $Before.physical) -ne (Canonical $After.physical)) { throw 'Database files changed outside an application startup.' }
    $additions=[Collections.Generic.List[string]]::new()
    $versionChanges=[Collections.Generic.List[object]]::new()
    foreach ($marker in $Before.installationMarkers.PSObject.Properties) {
        if ($After.installationMarkers.PSObject.Properties[$marker.Name].Value -ne $marker.Value) { throw 'An existing installation budget marker changed or disappeared.' }
    }
    foreach ($marker in $After.installationMarkers.PSObject.Properties) {
        if (-not $Before.installationMarkers.PSObject.Properties[$marker.Name]) {
            if (-not $Startup) { throw 'Installation marker created outside application startup.' }
            $additions.Add($marker.Name)
        }
    }
    foreach ($db in $Before.logical.PSObject.Properties) {
        $next=$After.logical.PSObject.Properties[$db.Name]
        if (-not $next) { throw 'A preexisting profile database disappeared.' }
        if ($db.Value.userVersion -ne $next.Value.userVersion) {
            if (-not $Startup) { throw 'Database user_version changed outside application startup.' }
            $versionChanges.Add([pscustomobject]@{database=$db.Name;before=$db.Value.userVersion;after=$next.Value.userVersion})
        }
        foreach ($table in $db.Value.tables.PSObject.Properties) {
            $newTable=$next.Value.tables.PSObject.Properties[$table.Name]
            if (-not $newTable -or (Canonical $table.Value) -ne (Canonical $newTable.Value)) { throw 'Preexisting logical data changed; this rehearsal cannot certify that migration.' }
        }
    }
    foreach ($db in $After.logical.PSObject.Properties) {
        $previous=$Before.logical.PSObject.Properties[$db.Name]
        if (-not $previous) { $additions.Add($db.Name); continue }
        foreach ($table in $db.Value.tables.PSObject.Properties) { if (-not $previous.Value.tables.PSObject.Properties[$table.Name]) { $additions.Add($db.Name+'#'+$table.Name) } }
    }
    if (-not $Startup -and $additions.Count) { throw 'Database schema changed outside application startup.' }
    return [pscustomobject]@{preexistingRowsPreserved=$true;startupStructuralAdditions=@($additions);userVersionChanges=@($versionChanges);physicalEqualityRequired=(-not $Startup)}
}
function Save-Checkpoint([string]$Name) {
    Assert-NoApplication
    $fingerprint=Get-Fingerprint
    $fingerprint | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $testDirectory ($Name+'.json')) -Encoding utf8
    return $fingerprint
}
function Run-Startup([string]$Binary) {
    $output=& node (Join-Path $PSScriptRoot 'installed-startup-smoke.mjs') $Binary
    if ($LASTEXITCODE -ne 0) { throw 'Installed startup smoke failed; no automatic cleanup was performed.' }
    $receipt=($output -join "`n") | ConvertFrom-Json
    if (-not $receipt.passed -or @($receipt.starts).Count -ne 2 -or @($receipt.starts | Where-Object {$_.views -ne 6}).Count -ne 0) { throw 'Two successful six-view starts are required.' }
    return $receipt
}

if ($SelfTest) {
    $count=0
    Assert-Upgrade (Read-Version '0.1.0') (Read-Version '0.1.1');$count++
    foreach ($bad in @('0.1.0','0.0.9')) { $denied=$false;try {Assert-Upgrade (Read-Version '0.1.0') (Read-Version $bad)} catch {$denied=$true};if(-not $denied){throw 'Version rejection self-test failed'};$count++ }
    Assert-Upgrade (Read-Version '0.1.5') (Read-Version '0.1.5') $true;$count++
    foreach ($bad in @('0.1.4','0.1.6')) { $denied=$false;try {Assert-Upgrade (Read-Version '0.1.5') (Read-Version $bad) $true} catch {$denied=$true};if(-not $denied){throw 'Rebuild version rejection self-test failed'};$count++ }
    if ((Get-NsisArguments 'D:\fixture\installed')[-1] -ne '/D=D:\fixture\installed' -or (Get-NsisArguments 'D:\fixture\installed' -Uninstall)[-1] -ne '_?=D:\fixture\installed') { throw 'NSIS final argument self-test failed' };$count++
    foreach ($invalid in @('D:\fixture with space\installed','relative\installed')) {$denied=$false;try{$null=Get-NsisArguments $invalid}catch{$denied=$true};if(-not $denied){throw 'Unsafe NSIS path accepted'};$count++}
    $sample='{"pointers":{},"profileCount":1,"physical":{"db":"bytes"},"logical":{"db":{"userVersion":0,"tables":{"vault":{"rows":1,"sha256":"hash"}}}}}'|ConvertFrom-Json
    $same=($sample|ConvertTo-Json -Depth 20)|ConvertFrom-Json
    $null=Assert-Fingerprint $sample $same;$count++
    $same.physical.db='checkpoint';$null=Assert-Fingerprint $sample $same -Startup;$count++
    $denied=$false;try {$null=Assert-Fingerprint $sample $same} catch {$denied=$true};if(-not $denied){throw 'Physical change self-test failed'};$count++
    $same.logical.db.tables.vault.sha256='altered';$denied=$false;try {$null=Assert-Fingerprint $sample $same -Startup} catch {$denied=$true};if(-not $denied){throw 'Logical change self-test failed'};$count++
    $same=($sample|ConvertTo-Json -Depth 20)|ConvertFrom-Json
    $same.logical|Add-Member -NotePropertyName 'ai-budget.sqlite3' -NotePropertyValue ([pscustomobject]@{userVersion=0;tables=[pscustomobject]@{}})
    $change=Assert-Fingerprint $sample $same -Startup
    if (@($change.startupStructuralAdditions).Count -ne 1 -or $change.startupStructuralAdditions[0] -ne 'ai-budget.sqlite3') {throw 'New budget database must be explicitly recorded'};$count++
    $denied=$false;try{$null=Assert-Fingerprint $sample $same}catch{$denied=$true};if(-not $denied){throw 'New database outside startup was accepted'};$count++
    $same=($sample|ConvertTo-Json -Depth 20)|ConvertFrom-Json
    $same.pointers|Add-Member -NotePropertyName 'active-profile.json' -NotePropertyValue 'moved'
    $denied=$false;try{$null=Assert-Fingerprint $sample $same -Startup}catch{$denied=$true};if(-not $denied){throw 'Changed profile pointer was accepted'};$count++
    $sample|Add-Member -NotePropertyName 'installationMarkers' -NotePropertyValue ([pscustomobject]@{})
    $same=($sample|ConvertTo-Json -Depth 20)|ConvertFrom-Json
    $same.installationMarkers|Add-Member -NotePropertyName 'ai-budget.installation.json' -NotePropertyValue 'initial-marker'
    $change=Assert-Fingerprint $sample $same -Startup
    if ($change.startupStructuralAdditions -notcontains 'ai-budget.installation.json') {throw 'New marker was not recorded'};$count++
    $denied=$false;try{$null=Assert-Fingerprint $sample $same}catch{$denied=$true};if(-not $denied){throw 'Marker created outside startup was accepted'};$count++
    $sample.installationMarkers|Add-Member -NotePropertyName 'ai-budget.installation.json' -NotePropertyValue 'different-marker'
    $denied=$false;try{$null=Assert-Fingerprint $sample $same -Startup}catch{$denied=$true};if(-not $denied){throw 'Changed marker was accepted'};$count++
    $same.installationMarkers.PSObject.Properties.Remove('ai-budget.installation.json')
    $denied=$false;try{$null=Assert-Fingerprint $sample $same -Startup}catch{$denied=$true};if(-not $denied){throw 'Removed marker was accepted'};$count++
    $pythonTests=& python -c $fingerprintPython --self-test
    if ($LASTEXITCODE -ne 0) { throw 'Synthetic fingerprint self-test failed.' }
    [ordered]@{passed=$true;powershellTests=$count;python=($pythonTests|ConvertFrom-Json);installersStarted=0;profileRead=$false} | ConvertTo-Json -Depth 5
    exit 0
}

$result=$null; $testDirectory=$null
try {
    $old=Read-Release $OldReleaseManifest ''
    $oldManifest=Get-Content -LiteralPath $old.manifest -Raw|ConvertFrom-Json
    if (-not $oldManifest.sourceBinary -or -not $oldManifest.sourceBinarySha256) { throw 'Old manifest must pin its preserved source binary and SHA-256.' }
    $new=Read-Release $NewReleaseManifest $NewSourceBinary
    Assert-Upgrade (Read-Version $old.version) (Read-Version $new.version) ([bool]$AllowSameVersionRebuild)
    Assert-NoApplication;Assert-NoRegistration
    if ($ValidateOnly) { [ordered]@{validated=$true;old=$old;new=$new;installersStarted=0;profileRead=$false}|ConvertTo-Json -Depth 8;return }
    $null=Get-Command python,node -ErrorAction Stop
    $null=Get-NsisArguments $appDirectory
    $localDirectory=Get-Item -LiteralPath (Join-Path $appDirectory '.local')
    if (-not $localDirectory.PSIsContainer -or ($localDirectory.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'The fixture parent must be a plain local directory.' }
    $testDirectory=Join-Path $appDirectory ('.local/upgrade-smoke-'+[guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $testDirectory | Out-Null
    $installDirectory=[IO.Path]::GetFullPath((Join-Path $testDirectory 'installed'))
    if (-not $installDirectory.StartsWith($workspacePrefix,[StringComparison]::OrdinalIgnoreCase) -or (Test-Path -LiteralPath $installDirectory)) { throw 'Owned installation destination is invalid or already exists.' }
    $binary=Join-Path $installDirectory 'openkoda.exe'
    $testKind=if ($AllowSameVersionRebuild) { 'same_version_rebuild' } else { 'version_upgrade' }
    $testLimit=if ($AllowSameVersionRebuild) { 'Same-version package replacement on this host; no new schema-migration claim. Any earlier migration proof remains a separate receipt.' } else { 'Positive version upgrade on this host only; no interrupted-upgrade, rollback, clean-Windows or publisher-trust claim.' }
    $result=[ordered]@{passed=$false;directory=$testDirectory;oldRelease=$old;newRelease=$new;testKind=$testKind;stage='preflight';limitations=@($testLimit,'Real initialized profile is read and hash-checked; startup can create new schema, but preexisting table contents must remain identical.','Failure preserves installation and receipts for inspection; no process kill, broad deletion or automatic uninstall.')}
    $before=Save-Checkpoint '01-before-old-install'
    Assert-NoRegistration;Assert-NoApplication
    $result.stage='install-old';$result.oldInstall=Run-OwnedProcess $old.installer (Get-NsisArguments $installDirectory)
    $result.oldVerification=Verify-Binary $old $binary
    $result.oldRegistration=Assert-OwnedRegistration $installDirectory $old.version
    $afterInstall=Save-Checkpoint '02-after-old-install';$result.oldInstallPreservation=Assert-Fingerprint $before $afterInstall
    $result.stage='start-old';$result.oldStartup=Run-Startup $binary
    $afterOld=Save-Checkpoint '03-after-old-startup';$result.oldStartupPreservation=Assert-Fingerprint $afterInstall $afterOld -Startup
    $null=Assert-OwnedRegistration $installDirectory $old.version;Assert-NoApplication
    $result.stage='upgrade';$result.upgrade=Run-OwnedProcess $new.installer (Get-NsisArguments $installDirectory)
    $result.newVerification=Verify-Binary $new $binary
    $result.newRegistration=Assert-OwnedRegistration $installDirectory $new.version
    $afterUpgrade=Save-Checkpoint '04-after-upgrade';$result.upgradePreservation=Assert-Fingerprint $afterOld $afterUpgrade
    $result.stage='start-new';$result.newStartup=Run-Startup $binary
    $afterNew=Save-Checkpoint '05-after-new-startup';$result.newStartupPreservation=Assert-Fingerprint $afterUpgrade $afterNew -Startup
    $result.endToEndPreservation=Assert-Fingerprint $before $afterNew -Startup
    $registration=Assert-OwnedRegistration $installDirectory $new.version
    $null=Verify-Binary $new $binary
    $uninstaller=Resolve-AppFile $registration.uninstaller
    Assert-NoApplication
    $result.stage='uninstall-owned';$result.uninstall=Run-OwnedProcess $uninstaller (Get-NsisArguments $installDirectory -Uninstall) 60
    # _?= runs in place and waits for actual completion. Its own EXE may remain;
    # no recursive filesystem cleanup is needed or performed.
    if ((Test-Path -LiteralPath $binary) -or (Test-Path -LiteralPath $uninstallKey)) { throw 'Owned executable or registration remained after uninstallation.' }
    if (Test-Path -LiteralPath $locationKey) {
        $key=Get-Item -LiteralPath $locationKey
        if ($key.GetValue('') -ne $installDirectory -or $key.GetSubKeyNames().Count -ne 0 -or @($key.GetValueNames()|Where-Object {$_ -notin @('','Installer Language')}).Count -ne 0) { throw 'Unexpected location registry values; preserved without deletion.' }
        Remove-Item -LiteralPath $locationKey
    }
    $afterRemoval=Save-Checkpoint '06-after-uninstall';$result.uninstallPreservation=Assert-Fingerprint $afterNew $afterRemoval
    $result.existingDataPreserved=$true;$result.stage='completed';$result.passed=$true
} catch {
    if ($null -eq $result) { throw }
    $result.error=$_.Exception.Message
} finally {
    foreach ($lease in $artifactLeases) { $lease.Dispose() }
    if ($null -ne $result) {
        $result.generatedAt=[DateTime]::UtcNow.ToString('o')
        $result|ConvertTo-Json -Depth 30|Set-Content -LiteralPath (Join-Path $testDirectory 'result.json') -Encoding utf8
        $result|ConvertTo-Json -Depth 30
    }
}
if ($null -ne $result -and -not $result.passed) { exit 1 }
