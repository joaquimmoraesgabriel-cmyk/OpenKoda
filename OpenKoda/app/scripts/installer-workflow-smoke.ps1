# Artifact/installer rehearsal for 0.1.12. NEVER starts an application binary.
# Release builds ignore OPENKODA_DATA_DIR; schema migration is separate debug evidence.
# Restore the archived 0.1.11 and 0.1.12 packages, pinned binaries and migration fixture
# inside the workspace before replaying this historical rehearsal.
[CmdletBinding()]
param(
    [string]$OldReleaseManifest,
    [string]$NewReleaseManifest,
    [string]$NewSourceBinary,
    [string]$NativeMigrationReceipt,
    [string]$ExpectedNativeSha256,
    [ValidateSet('ExtractOnly','Install')][string]$Mode = 'ExtractOnly',
    [string]$ExtractorPath,
    [string]$ExpectedExtractorSha256,
    [string]$ExpectedExtractorLibrarySha256,
    [switch]$ValidateOnly,
    [switch]$SelfTest
)
$ErrorActionPreference='Stop'
$appRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$appPrefix=$appRoot.TrimEnd('\')+'\'
$leases=[Collections.Generic.List[IO.FileStream]]::new()
$uninstallKey='HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda'
$locationKey='HKCU:\Software\openkoda\OpenKoda'
$registrationKeys=@($uninstallKey,$locationKey,'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda','HKLM:\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall\OpenKoda','HKLM:\Software\openkoda\OpenKoda')
function Sha([string]$Path){(Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()}
function Plain-Owned([string]$Path,[switch]$Directory){
    if(-not [IO.Path]::IsPathRooted($Path)){$Path=Join-Path $appRoot $Path}
    $full=[IO.Path]::GetFullPath($Path)
    if($full -notmatch '^[a-zA-Z]:\\' -or -not $full.StartsWith($appPrefix,[StringComparison]::OrdinalIgnoreCase) -or $full.Substring(3).Contains(':')){throw 'Artifact must be inside the app workspace without alternate streams.'}
    $item=Get-Item -LiteralPath $full
    if([bool]$item.PSIsContainer -ne [bool]$Directory){throw 'Unexpected artifact type.'}
    $node=$item
    while($node.FullName -ne $appRoot){
        if(($node.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0){throw 'Redirected artifact refused.'}
        $node=if($node -is [IO.FileInfo]){$node.Directory}else{$node.Parent}
        if($null -eq $node){throw 'Artifact ancestry escaped the workspace.'}
    }
    return $full
}
function Hold([string]$Path){$leases.Add([IO.File]::Open($Path,[IO.FileMode]::Open,[IO.FileAccess]::Read,[IO.FileShare]::Read))}
function Read-Json([string]$Path){$file=Plain-Owned $Path;if((Get-Item -LiteralPath $file).Length -gt 2MB){throw 'Receipt exceeds the bounded metadata limit.'};Get-Content -LiteralPath $file -Raw|ConvertFrom-Json}
function Version([string]$Value){if($Value -notmatch '^\d+\.\d+\.\d+(\.\d+)?$'){throw 'Invalid numeric version.'};$v=[version]$Value;[version]::new($v.Major,$v.Minor,$v.Build,[Math]::Max(0,$v.Revision))}
function Read-Release([string]$Path,[string]$Override,[string]$ExpectedVersion){
    $manifest=Plain-Owned $Path;Hold $manifest;$data=Read-Json $manifest
    if($data.profile -and $data.profile -ne 'release'){throw 'Only release installers are accepted.'}
    if($data.version -ne $ExpectedVersion){throw 'Unexpected manifest version.'}
    $source=$Override;if(-not $source){$source=$data.sourceBinary};if(-not $source){$source=Join-Path $appRoot 'src-tauri/target/release/openkoda.exe'}
    $source=Plain-Owned $source;Hold $source;$sourceHash=Sha $source
    if($data.sourceBinarySha256 -and $sourceHash -ne $data.sourceBinarySha256){throw 'Source binary hash differs from its manifest.'}
    if((Version (Get-Item -LiteralPath $source).VersionInfo.FileVersion) -ne (Version $ExpectedVersion)){throw 'Source FileVersion differs.'}
    $entries=if($data.installer){@([pscustomobject]@{path=$data.installer;sha256=$data.sha256;bytes=$data.bytes})}else{@($data.files)}
    if($entries.Count -ne 1){throw 'Manifest must identify one installer.'}
    $entry=$entries[0];$installer=Plain-Owned $entry.path;Hold $installer
    if($entry.sha256 -notmatch '^[a-f0-9]{64}$' -or (Sha $installer) -ne $entry.sha256 -or (Get-Item -LiteralPath $installer).Length -ne $entry.bytes){throw 'Installer hash/size differs.'}
    if((Version (Get-Item -LiteralPath $installer).VersionInfo.FileVersion) -ne (Version $ExpectedVersion)){throw 'Installer FileVersion differs.'}
    [pscustomobject]@{manifest=$manifest;manifestSha256=(Sha $manifest);version=$ExpectedVersion;installer=$installer;installerSha256=$entry.sha256;sourceBinary=$source;sourceBinarySha256=$sourceHash;authenticode=(Get-AuthenticodeSignature -LiteralPath $installer).Status.ToString()}
}
function Read-Migration([string]$Path,[string]$ExpectedHash,$Old){
    if($ExpectedHash -notmatch '^[a-f0-9]{64}$'){throw 'An exact reviewed debug SHA-256 is required.'}
    $file=Plain-Owned $Path;Hold $file;$receipt=Read-Json $file
    if(-not $receipt.passed -or -not $receipt.ownedProcessClosed -or $receipt.artifact.version -ne '0.1.12' -or $receipt.artifact.sha256 -ne $ExpectedHash){throw 'Native compatibility receipt is not a completed matching debug rehearsal.'}
    $binary=Plain-Owned $receipt.artifact.binary;Hold $binary;if((Sha $binary) -ne $ExpectedHash){throw 'Reviewed debug binary changed.'}
    if((Sha (Join-Path $PSScriptRoot 'native-workflow-smoke.mjs')) -ne $receipt.artifact.scriptSha256){throw 'Migration harness differs from the receipt.'}
    $ids=@('preserved-previous-release-creates-fixture','new-release-preserves-exact-project-history','reviewed-backup-restore-keeps-capture-bytes-and-pauses-work')
    foreach($id in $ids){$records=@($receipt.records|Where-Object{$_.id -eq $id});if($records.Count -ne 1 -or $records[0].status -ne 'passed'){throw 'Required migration/backup evidence is missing.'}}
    $seed=@($receipt.records|Where-Object{$_.id -eq $ids[0]})[0].detail
    $migration=@($receipt.records|Where-Object{$_.id -eq $ids[1]})[0].detail
    $oldManifest=Read-Json $Old.manifest
    if($seed.version -ne '0.1.11' -or $seed.schema -ne 8 -or $seed.sha256 -ne $oldManifest.nativeTestBinarySha256 -or ($migration.schema -join ',') -ne '8,8' -or $seed.environmentSchema -ne 2 -or ($migration.environmentSchema -join ',') -ne '2,2' -or -not $migration.exactEnvironmentPreserved -or -not $migration.exactJsonPreserved -or -not $migration.noAutomaticDispatch){throw 'Compatibility was not the reviewed 0.1.11-to-0.1.12 schema-eight transition.'}
    $oldDebug=Plain-Owned $oldManifest.nativeTestBinary;Hold $oldDebug;if((Sha $oldDebug) -ne $seed.sha256){throw 'Preserved old debug artifact changed.'}
    $directory=Plain-Owned $receipt.directory -Directory
    if((Split-Path -Leaf $directory) -notmatch '^native-workflow-[a-f0-9-]{36}$'){throw 'Expected an explicitly owned native-workflow fixture.'}
    $data=Plain-Owned (Join-Path $directory 'data') -Directory
    [pscustomobject]@{path=$file;sha256=(Sha $file);debugBinary=$binary;debugSha256=$ExpectedHash;oldDebugSha256=$seed.sha256;schemaBefore=8;schemaAfter=8;environmentBefore=2;environmentAfter=2;dataDirectory=$data;installedReleaseExecuted=$false;proofKind='separate_preserved_debug_artifacts'}
}
function Synthetic-Fingerprint([string]$Directory){
    $base=Plain-Owned $Directory -Directory;$pending=[Collections.Generic.Stack[string]]::new();$pending.Push($base);$hashes=[ordered]@{};$count=0;$total=0L
    while($pending.Count){foreach($item in (Get-ChildItem -LiteralPath ($pending.Pop()) -Force|Sort-Object Name)){
        $path=Plain-Owned $item.FullName -Directory:$item.PSIsContainer
        if($item.PSIsContainer){$pending.Push($path)}else{$count++;$total+=$item.Length;if($count -gt 10000 -or $total -gt 512MB){throw 'Synthetic fingerprint exceeds its bound.'};$hashes[$path.Substring($base.Length+1)]=Sha $path}
    }}
    return ($hashes|ConvertTo-Json -Depth 2 -Compress)
}
function Shared-State {
    $keys=[ordered]@{};foreach($key in $registrationKeys){$keys[$key]=[bool](Test-Path -LiteralPath $key)}
    $links=[ordered]@{};foreach($name in @('Desktop','CommonDesktopDirectory','Programs','CommonPrograms')){$folder=[Environment]::GetFolderPath($name);if(-not $folder){throw 'Cannot resolve a shared shortcut folder.'};$links[$name]=[bool](Test-Path -LiteralPath (Join-Path $folder 'OpenKoda.lnk'))}
    $runKey='HKCU:\Software\Microsoft\Windows\CurrentVersion\Run';$autostart=$false
    if(Test-Path -LiteralPath $runKey){$autostart=@((Get-Item -LiteralPath $runKey).GetValueNames()) -contains 'OpenKoda'}
    [pscustomobject]@{registration=$keys;shortcuts=$links;autostart=$autostart}
}
function Canonical($Value){ConvertTo-Json -InputObject $Value -Depth 8 -Compress}
function Assert-NoApp {if(Get-Process -Name openkoda,openkoda-debug -ErrorAction SilentlyContinue){throw 'A pre-existing application is running; it is preserved.'}}
function Assert-Clear($State){if(@($State.registration.Values|Where-Object{$_}).Count -or @($State.shortcuts.Values|Where-Object{$_}).Count -or $State.autostart){throw 'Existing OpenKoda registration/shortcut/autostart blocks installation. Use ExtractOnly; no existing installation will be touched.'}}
function Assert-NoShortcutChanges($Before){$after=Shared-State;if((Canonical $Before.shortcuts) -ne (Canonical $after.shortcuts) -or $Before.autostart -ne $after.autostart){throw 'Shared shortcut/autostart state changed; preserved for inspection.'};Assert-NoApp}
function Assert-WebViewPresent {
    $guid='{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
    foreach($key in @("HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\$guid","HKLM:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$guid","HKCU:\SOFTWARE\Microsoft\EdgeUpdate\Clients\$guid")){
        if(Test-Path -LiteralPath $key){$value=(Get-ItemProperty -LiteralPath $key -Name pv -ErrorAction Stop).pv;if($value -match '^\d+\.\d+\.\d+\.\d+$' -and [version]$value -gt [version]'0.0.0.0'){return $value}}
    };throw 'Existing WebView2 could not be verified; no runtime downloader will be started.'
}
function Nsis-Args([string]$Directory,[switch]$Uninstall){if($Directory -match '[\s"]' -or -not $Directory.StartsWith($appPrefix,[StringComparison]::OrdinalIgnoreCase)){throw 'NSIS requires an owned absolute whitespace-free destination.'};if($Uninstall){@('/S','/UPDATE',"_?=$Directory")}else{@('/S','/NS','/UPDATE',"/D=$Directory")}}
function Assert-Launch([string]$Executable,[string[]]$Arguments,[string[]]$Allowed){
    if($Executable -notin $Allowed -or (Split-Path -Leaf $Executable) -in @('openkoda.exe','openkoda-debug.exe') -or $Arguments -contains '/R' -or $Arguments -contains '/ARGS'){throw 'Application execution is forbidden in this installer harness.'}
}
function Run-Nsis([string]$Executable,[string[]]$Arguments,[string[]]$Allowed){
    Assert-Launch $Executable $Arguments $Allowed;Assert-NoApp
    $process=Start-Process -FilePath $Executable -ArgumentList $Arguments -WindowStyle Hidden -PassThru
    $watch=[Diagnostics.Stopwatch]::StartNew()
    while(-not $process.WaitForExit(1000)){if($watch.Elapsed.TotalSeconds -gt 120){throw "Owned installer PID $($process.Id) timed out; no process was killed or installation removed."}}
    $code=$process.ExitCode;$pidValue=$process.Id;$process.Dispose();if($code -ne 0){throw "Owned NSIS process failed with exit $code."};Assert-NoApp
    [pscustomobject]@{pid=$pidValue;exitCode=$code;elapsedMs=$watch.ElapsedMilliseconds;arguments=$Arguments}
}
function Verify-Binary($Release,[string]$File){$file=Plain-Owned $File;$output=& python (Join-Path $PSScriptRoot 'verify-bundled-binary.py') $Release.sourceBinary $file;if($LASTEXITCODE -ne 0){throw 'Payload differs beyond the NSIS bundle marker.'};($output -join "`n")|ConvertFrom-Json}
function Assert-OwnedRegistration([string]$Directory,[string]$Version){
    $r=Get-ItemProperty -LiteralPath $uninstallKey
    if([IO.Path]::GetFullPath($r.InstallLocation.Trim('"')) -ne $Directory -or (Version $r.DisplayVersion) -ne (Version $Version)){throw 'Registration is not this owned fixture/version.'}
    if((Get-Item -LiteralPath $locationKey).GetValue('') -ne $Directory -or $r.UninstallString -ne ('"'+(Join-Path $Directory 'uninstall.exe')+'"')){throw 'Uninstall registration is not the exact owned executable.'}
}
function Extract-Entry([string]$Listing){
    $paths=@();foreach($line in ($Listing -split "`r?`n")){if($line.StartsWith('Path = ')){$name=$line.Substring(7);if($name -match '(^|[\\/])openkoda\.exe$'){$paths+=$name}}}
    if($paths.Count -ne 1){throw 'Archive must contain exactly one application payload.'};$entry=$paths[0]
    if($entry -match '^[\\/\-]' -or $entry.Contains(':') -or $entry -match '(^|[\\/])\.\.([\\/]|$)' -or $entry -notmatch '^[a-zA-Z0-9_$ .\\/-]+$'){throw 'Unsafe archive entry refused.'};$entry
}
function Run-Extractor([string]$Tool,[string[]]$Arguments,[string]$LogBase){
    foreach($argument in $Arguments){if($argument.Contains('"') -or $argument.EndsWith('\')){throw 'Unsupported extractor argument quoting.'}}
    $quoted=@($Arguments|ForEach-Object{'"'+$_+'"'})
    $process=Start-Process -FilePath $Tool -ArgumentList $quoted -WindowStyle Hidden -PassThru -RedirectStandardOutput ($LogBase+'.stdout.log') -RedirectStandardError ($LogBase+'.stderr.log')
    if(-not $process.WaitForExit(60000)){throw "Owned extractor PID $($process.Id) timed out; artifacts/process preserved for inspection."}
    $code=$process.ExitCode;$process.Dispose()
    if((Get-Item -LiteralPath ($LogBase+'.stdout.log')).Length -gt 1MB -or (Get-Item -LiteralPath ($LogBase+'.stderr.log')).Length -gt 1MB){throw 'Extractor output exceeds its bound.'}
    if($code -ne 0){throw "Pinned extractor failed with exit $code; no installer was executed."}
    Get-Content -LiteralPath ($LogBase+'.stdout.log') -Raw
}
function Extract-Payload($Release,[string]$Destination,[string]$Tool){
    New-Item -ItemType Directory -Path $Destination | Out-Null;$null=Plain-Owned $Destination -Directory
    $text=Run-Extractor $Tool @('l','-slt','-ba','--',$Release.installer) ($Destination+'-list')
    $entry=Extract-Entry $text
    $null=Run-Extractor $Tool @('e','-y','-bd','-bb0',('-o'+$Destination),'--',$Release.installer,$entry) ($Destination+'-extract')
    $items=@(Get-ChildItem -LiteralPath $Destination -Force);if($items.Count -ne 1 -or $items[0].Name -ne 'openkoda.exe' -or $items[0].PSIsContainer){throw 'Unexpected extraction output preserved for inspection.'}
    [pscustomobject]@{entry=$entry;verification=(Verify-Binary $Release $items[0].FullName)}
}

function Test-BundleResources([string]$Destination, [string]$Installer, [string]$Extractor) {
    $configPath = Plain-Owned (Join-Path $appRoot 'src-tauri/tauri.conf.json')
    $resources = (Read-Json $configPath).bundle.resources
    $expected = [ordered]@{}
    foreach ($resource in $resources.PSObject.Properties) {
        $relative = [string]$resource.Value
        if ($relative -cnotmatch '^(avisos|recuperacao)/[A-Za-z0-9_-]+\.[A-Za-z0-9]+$') {
            throw 'Unexpected destination for a bundled resource.'
        }
        $source = Plain-Owned (Join-Path (Split-Path $configPath) $resource.Name)
        $expected[$relative] = Sha $source
    }
    if ($Extractor) {
        New-Item -ItemType Directory -Path $Destination | Out-Null
        $entries = @($expected.Keys | ForEach-Object { $_.Replace('/', '\') })
        $null = Run-Extractor $Extractor (@('x','-y','-bd','-bb0',('-o'+$Destination),'--',$Installer) + $entries) ($Destination+'-extract')
    }
    foreach ($relative in $expected.Keys) {
        $file = Plain-Owned (Join-Path $Destination $relative)
        if ((Sha $file) -ne $expected[$relative]) {throw ('Bundled resource differs: '+$relative)}
    }
    return $expected
}

if($SelfTest){
    $checks=0
    if((Version '0.1.12') -le (Version '0.1.11')){throw 'Version comparison failed.'};$checks++
    foreach($bad in @('..\openkoda.exe','/openkoda.exe','C:\openkoda.exe','-openkoda.exe','a:stream/openkoda.exe','x[1]/openkoda.exe')){$failed=$false;try{$null=Extract-Entry ("Path = "+$bad)}catch{$failed=$true};if(-not $failed){throw 'Unsafe archive path accepted.'};$checks++}
    if((Extract-Entry 'Path = $INSTDIR\openkoda.exe') -ne '$INSTDIR\openkoda.exe'){throw 'Valid archive entry refused.'};$checks++
    $failed=$false;try{$null=Extract-Entry "Path = openkoda.exe`nPath = a/openkoda.exe"}catch{$failed=$true};if(-not $failed){throw 'Duplicate payload accepted.'};$checks++
    foreach($name in @('openkoda.exe','openkoda-debug.exe')){$path=Join-Path $appRoot $name;$failed=$false;try{Assert-Launch $path @('/S') @($path)}catch{$failed=$true};if(-not $failed){throw 'Application launch accepted.'};$checks++}
    $fake=Join-Path $appRoot 'fixture-setup.exe';$failed=$false;try{Assert-Launch $fake @('/S','/R') @($fake)}catch{$failed=$true};if(-not $failed){throw 'Auto-launch flag accepted.'};$checks++
    if((Nsis-Args (Join-Path $appRoot '.local/owned/installed') -Uninstall) -notcontains '/UPDATE'){throw 'Uninstall must preserve shared jump lists/autostart.'};$checks++
    [ordered]@{passed=$true;checks=$checks;processesStarted=0;registryRead=$false;profileRead=$false}|ConvertTo-Json;return
}
$result=$null
try{
    if(-not $OldReleaseManifest -or -not $NewReleaseManifest){throw 'Supply -OldReleaseManifest for archived 0.1.11 and -NewReleaseManifest for archived 0.1.12, or use -SelfTest for synthetic checks.'}
    $old=Read-Release $OldReleaseManifest '' '0.1.11';$new=Read-Release $NewReleaseManifest $NewSourceBinary '0.1.12'
    $migration=Read-Migration $NativeMigrationReceipt $ExpectedNativeSha256 $old
    Assert-NoApp;$before=Shared-State
    if($ValidateOnly){[ordered]@{validated=$true;mode=$Mode;old=$old;new=$new;migration=$migration;sharedState=$before;installationBlocked=(@($before.registration.Values|Where-Object{$_}).Count -gt 0 -or @($before.shortcuts.Values|Where-Object{$_}).Count -gt 0 -or $before.autostart);processesStarted=0;profileRead=$false}|ConvertTo-Json -Depth 10;return}
    $null=Get-Command python -ErrorAction Stop
    if($Mode -eq 'Install') {Assert-Clear $before;$webView=Assert-WebViewPresent}
    else{
        if($ExpectedExtractorSha256 -notmatch '^[a-f0-9]{64}$'){throw 'ExtractOnly requires a coordinator-reviewed extractor SHA-256.'}
        $extractor=Plain-Owned $ExtractorPath;Hold $extractor
        if((Sha $extractor) -ne $ExpectedExtractorSha256 -or (Split-Path -Leaf $extractor) -notin @('7z.exe','7zz.exe')){throw 'Unexpected extractor identity; reduced 7zr/7za do not establish NSIS support.'}
        if((Split-Path -Leaf $extractor) -eq '7z.exe'){
            $extractorLibrary=Plain-Owned (Join-Path (Split-Path -Parent $extractor) '7z.dll');Hold $extractorLibrary
            if($ExpectedExtractorLibrarySha256 -notmatch '^[a-f0-9]{64}$' -or (Sha $extractorLibrary) -ne $ExpectedExtractorLibrarySha256){throw 'The full-format extractor DLL must also be pinned.'}
        }
    }
    $null=Plain-Owned (Join-Path $appRoot '.local') -Directory
    $directory=Join-Path $appRoot ('.local/installer-workflow-'+[guid]::NewGuid().ToString());New-Item -ItemType Directory -Path $directory|Out-Null
    $result=[ordered]@{passed=$false;directory=$directory;mode=$Mode;oldRelease=$old;newRelease=$new;migrationEvidence=$migration;sharedStateBefore=$before;installedReleaseExecuted=$false;personalProfileOpened=$false;stage='preflight';limitations=@('No release application is launched: OPENKODA_DATA_DIR is debug-only.','Schema-eight compatibility from 0.1.11 to 0.1.12 and backup/restore are separate immutable debug receipts, not installed-release startup proof.','ExtractOnly verifies package payloads; it does not execute installer, replacement, registry writes, WebView2 install or uninstaller.','Install mode refuses any existing registration/shortcut/autostart and uses /UPDATE to preserve jump lists; it is one positive local rehearsal, not clean Windows, interruption or rollback.','Failure preserves this fixture and any owned installation; no automatic kill, recursive delete or removal of an existing installation.')}
    $fingerprint=Synthetic-Fingerprint $migration.dataDirectory;$fingerprint|Set-Content -LiteralPath (Join-Path $directory 'synthetic-data-before.json') -Encoding utf8
    if($Mode -eq 'ExtractOnly'){
        $result.extractor=[ordered]@{path=$extractor;sha256=$ExpectedExtractorSha256;library=$extractorLibrary;librarySha256=$ExpectedExtractorLibrarySha256};$result.stage='extract-old';$result.oldPayload=Extract-Payload $old (Join-Path $directory 'old') $extractor
        $result.stage='extract-new';$result.newPayload=Extract-Payload $new (Join-Path $directory 'new') $extractor
        $result.bundledResources=Test-BundleResources (Join-Path $directory 'resources') $new.installer $extractor
    }else{
        $install=Join-Path $directory 'installed';$binary=Join-Path $install 'openkoda.exe';$uninstaller=Join-Path $install 'uninstall.exe';$allowed=@($old.installer,$new.installer,$uninstaller)
        $result.webViewVersion=$webView;Assert-Clear (Shared-State)
        $result.stage='install-old';$result.oldInstall=Run-Nsis $old.installer (Nsis-Args $install) $allowed;Assert-OwnedRegistration $install $old.version;Assert-NoShortcutChanges $before;$result.oldPayload=Verify-Binary $old $binary
        if((Synthetic-Fingerprint $migration.dataDirectory) -ne $fingerprint){throw 'Synthetic fixture changed during installation.'}
        $result.stage='upgrade';$result.upgrade=Run-Nsis $new.installer (Nsis-Args $install) $allowed;Assert-OwnedRegistration $install $new.version;Assert-NoShortcutChanges $before;$result.newPayload=Verify-Binary $new $binary
        $result.bundledResources=Test-BundleResources $install
        if((Synthetic-Fingerprint $migration.dataDirectory) -ne $fingerprint){throw 'Synthetic fixture changed during replacement.'}
        $result.stage='uninstall-owned';$null=Plain-Owned $uninstaller;Assert-OwnedRegistration $install $new.version;$result.uninstall=Run-Nsis $uninstaller (Nsis-Args $install -Uninstall) $allowed
        if((Test-Path -LiteralPath $binary) -or (Test-Path -LiteralPath $uninstallKey)){throw 'Owned executable/registration remained.'}
        if(Test-Path -LiteralPath $locationKey){$key=Get-Item -LiteralPath $locationKey;if($key.GetValue('') -ne $install -or $key.GetSubKeyNames().Count -or @($key.GetValueNames()|Where-Object{$_ -notin @('','Installer Language')}).Count){throw 'Unexpected registration is preserved.'};Remove-Item -LiteralPath $locationKey}
    }
    Assert-NoApp;$after=Shared-State;$result.sharedStateAfter=$after;if((Canonical $after) -ne (Canonical $before)){throw 'Shared installation metadata changed.'}
    $afterFingerprint=Synthetic-Fingerprint $migration.dataDirectory;$afterFingerprint|Set-Content -LiteralPath (Join-Path $directory 'synthetic-data-after.json') -Encoding utf8
    if($afterFingerprint -ne $fingerprint){throw 'Synthetic migration/backup fixture changed.'};$result.syntheticDataPreserved=$true;$result.stage='completed';$result.passed=$true
}catch{if($null -eq $result){throw};$result.error=$_.Exception.Message}
finally{foreach($lease in $leases){$lease.Dispose()};if($null -ne $result){$result.generatedAt=[DateTime]::UtcNow.ToString('o');$result|ConvertTo-Json -Depth 12|Set-Content -LiteralPath (Join-Path $directory 'result.json') -Encoding utf8;$result|ConvertTo-Json -Depth 12}}
if($null -ne $result -and -not $result.passed){exit 1}
