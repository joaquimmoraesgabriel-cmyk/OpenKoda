// Deliberate, opt-in production-installation upgrade check. Never imports test
// fixtures, seeds projects, signs in, invokes paid providers or bypasses rollout.
// Run only after reviewing/publishing the exact candidate on the official feed.
import {chromium,expect} from '@playwright/test';
import {spawn,execFileSync} from 'node:child_process';
import {readFile,writeFile,mkdir,lstat,realpath} from 'node:fs/promises';
import {resolve,dirname,join,basename,toNamespacedPath} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash,randomUUID} from 'node:crypto';
import {createServer} from 'node:net';

const appRoot=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2),option=name=>{const index=args.indexOf(name);return index<0?undefined:args[index+1];};
if(!args.includes('--execute-production-upgrade')||!option('--receipt'))throw new Error('Review first; then pass --receipt <1.1.1 receipt> --execute-production-upgrade.');
const FROM='1.1.0',TO='1.1.1';
const OLD_INSTALLED_SHA256='73e8a5a739ab80a3f3f338fe0eba0d8d2750cde0561a7ffbe484ef4097b8146b';
const endpoint='https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/desktop-updates/latest.json';
// LOCALAPPDATA can be virtualized by the packaged host; use the verified
// per-user NSIS location rather than inheriting that virtualization.
const install=join(process.env.USERPROFILE,'AppData','Local','OpenKoda'),executable=join(install,'openkoda.exe');
const data=join(process.env.USERPROFILE,'.openkoda');
const registration='HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\OpenKoda';
const directory=join(appRoot,'.local',`production-update-${randomUUID()}`);
const pauseAfterDiscovery=args.includes('--pause-after-discovery');
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const psQuote=value=>`'${value.replaceAll("'","''")}'`;
const runPs=script=>execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',script],{encoding:'utf8',windowsHide:true,timeout:30000});
const python=(script,...parameters)=>JSON.parse(execFileSync('python',['-c',script,...parameters],{encoding:'utf8',windowsHide:true,timeout:90000,maxBuffer:8*1024*1024}));
const note=step=>console.log(JSON.stringify({step,at:new Date().toISOString()}));
const processList=()=>JSON.parse(runPs(`$items=@(Get-CimInstance Win32_Process -Filter "Name='openkoda.exe'" | ForEach-Object {[pscustomobject]@{pid=$_.ProcessId;parentPid=$_.ParentProcessId;path=$_.ExecutablePath;created=$_.CreationDate.ToUniversalTime().ToString('o')}});ConvertTo-Json -InputObject $items -Compress`));
const assertNoOpenKoda=()=>{if(processList().length)throw new Error('An OpenKoda process is already open. It will not be stopped or reused by this test.');};
const plainPath=async path=>{for(let cursor=resolve(path);;cursor=dirname(cursor)){const info=await lstat(cursor);if(info.isSymbolicLink())throw new Error('Reparse/symlink installation paths are not supported by this test.');if(dirname(cursor)===cursor)break;}};
const installedVersion=()=>runPs(`(Get-Item -LiteralPath ${psQuote(executable)}).VersionInfo.FileVersion`).trim();

// Every SQLite is fingerprinted, including inactive profiles. Only the active
// commercial-account vault is allowed to rotate ciphertext during normal session
// restoration. That exception is recorded, and its user identity must stay equal.
const fingerprint=()=>python(String.raw`
import pathlib,sqlite3,hashlib,json,sys,uuid
root=pathlib.Path(sys.argv[1]);active=root;pointer=root/'active-profile.json'
def plain(path):
 for node in (path,*path.parents):
  item=node.lstat()
  assert not node.is_symlink() and not getattr(item,'st_file_attributes',0)&0x400,'Reparse point rejected'
  if node==root:break
plain(root)
if pointer.exists():
 plain(pointer);ref=json.loads(pointer.read_text(encoding='utf-8'))['profile']
 if ref['kind']=='profile':
  identity=ref['id'];assert str(uuid.UUID(identity))==identity;active=root/'profiles'/identity
 else:assert ref['kind']=='base'
plain(active)
def digest(value):return hashlib.sha256(json.dumps(value,sort_keys=True,separators=(',',':'),ensure_ascii=True).encode()).hexdigest()
databases={}
for path in sorted(list(root.glob('*.sqlite3'))+list(root.glob('profiles/*/*.sqlite3'))):
 plain(path);db=sqlite3.connect(path.as_uri()+'?mode=ro',uri=True);db.execute('BEGIN')
 assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
 h=hashlib.sha256()
 for line in db.iterdump():h.update(line.encode())
 db.close();databases[path.relative_to(root).as_posix()]=h.hexdigest()
db=sqlite3.connect((active/'openkoda.sqlite3').as_uri()+'?mode=ro',uri=True)
state=json.loads(db.execute('SELECT json FROM app_state WHERE id=1').fetchone()[0]);db.close()
ai=sqlite3.connect((active/'assistant.sqlite3').as_uri()+'?mode=ro',uri=True)
messages=ai.execute('SELECT id,assessment,role,text,created FROM messages ORDER BY id').fetchall()
requests=ai.execute('SELECT * FROM requests ORDER BY id').fetchall();ai.close()
metadata={}
for parent in set([root,active]):
 for name in ['active-profile.json','ai-budget.installation.json','window-behavior.json']:
  path=parent/name
  if path.exists():plain(path);metadata[path.relative_to(root).as_posix()]=hashlib.sha256(path.read_bytes()).hexdigest()
print(json.dumps({'activeDirectory':str(active),'databases':databases,'metadata':metadata,'activeCommercialVault':(active/'commercial-account.sqlite3').relative_to(root).as_posix(),
 'coreSha256':digest(state),'projects':len(state['projects']),'assessments':len(state['assessments']),'messages':len(messages),'messagesSha256':digest(messages),'providerRequests':len(requests),'providerRequestsSha256':digest(requests)}))
`,data);
const compareData=(before,after)=>{
 const changed=Object.keys({...before.databases,...after.databases}).filter(path=>before.databases[path]!==after.databases[path]);
 expect(changed.filter(path=>path!==before.activeCommercialVault)).toEqual([]);
 for(const key of ['activeDirectory','activeCommercialVault','metadata','coreSha256','projects','assessments','messages','messagesSha256','providerRequests','providerRequestsSha256'])expect(after[key],key).toEqual(before[key]);
 return {allSqliteUnchanged:changed.length===0,changedDatabases:changed,allowedReason:changed.length?'Active account vault can re-encrypt the same verified identity during session restoration.':null};
};
const verifyBackup=before=>python(String.raw`
import pathlib,sqlite3,hashlib,json,sys
root=pathlib.Path(sys.argv[1]);expected=json.loads(sys.argv[2]);existing=set(json.loads(sys.argv[3]));active=pathlib.Path(expected['activeDirectory'])
found=[p for p in (root/'update-backups').glob('*/manifest.json') if str(p) not in existing]
assert len(found)==1,'Expected exactly one newly created native backup'
manifest_path=found[0];manifest=json.loads(manifest_path.read_text(encoding='utf-8'))
assert manifest['schema']=='openkoda.update-backup.v1' and manifest['complete'] and manifest['fromVersion']=='1.1.0' and manifest['toVersion']=='1.1.1'
assert pathlib.Path(manifest['profileDirectory']).samefile(active)
expected_entries={(('profile' if parent==active else 'installation'),p.name) for parent in set([root,active]) for p in parent.glob('*.sqlite3')}
actual_entries={(e['area'],e['name']) for e in manifest['files']}
assert len(actual_entries)==len(manifest['files']) and actual_entries==expected_entries,'Incomplete or duplicate database inventory'
checked=[];rotated=[]
for entry in manifest['files']:
 area,name=entry['area'],entry['name'];assert area in ['profile','installation'] and pathlib.Path(name).name==name and name.endswith('.sqlite3')
 path=manifest_path.parent/area/name
 assert not path.is_symlink() and not getattr(path.lstat(),'st_file_attributes',0)&0x400
 assert hashlib.sha256(path.read_bytes()).hexdigest()==entry['sha256']
 db=sqlite3.connect(path.as_uri()+'?mode=ro',uri=True);assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
 h=hashlib.sha256()
 for line in db.iterdump():h.update(line.encode())
 db.close();original=(active if area=='profile' else root)/name;relative=original.relative_to(root).as_posix()
 if h.hexdigest()!=expected['databases'][relative]:
  assert relative==expected['activeCommercialVault'],'Backup contents differ from pre-install state';rotated.append(relative)
 checked.append({'area':area,'name':name,'sha256':entry['sha256'],'logicalSha256':h.hexdigest()})
print(json.dumps({'manifest':str(manifest_path),'fromVersion':manifest['fromVersion'],'toVersion':manifest['toVersion'],'verifiedDatabases':checked,'rotatedCommercialVault':rotated}))
`,data,JSON.stringify(before),JSON.stringify(receipt.existingBackupManifests));
const backupManifests=()=>python(String.raw`import pathlib,json,sys;print(json.dumps([str(p) for p in (pathlib.Path(sys.argv[1])/'update-backups').glob('*/manifest.json')]))`,data);

const receipt={schema:'openkoda.production-update-smoke.v1',passed:false,fromVersion:FROM,toVersion:TO,directory,checks:[]};
let child,browser,page,port,installationStarted=false,downloadInFlight=false,restartVerified=false,preserveProcess=false;
const update=action=>page.evaluate(action=>window.__TAURI_INTERNALS__.invoke('desktop_update',{input:{action}}),action);
const accountIdentity=async()=>sha(JSON.stringify(await page.evaluate(async()=>{const session=await window.__TAURI_INTERNALS__.invoke('request',{request:{action:'account.session',payload:{}}});return session.user?.id??null;})));
const preferences=async()=>sha(await page.evaluate(()=>JSON.stringify(Object.keys(localStorage).filter(key=>key.startsWith('openkoda.')).sort().map(key=>[key,localStorage.getItem(key)]))));
async function disconnect(){await browser?.close().catch(()=>{});browser=undefined;page=undefined;}
async function connect(){
 await expect.poll(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:60000}).toBe(true);
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);const context=browser.contexts()[0];await expect.poll(()=>context.pages().length).toBeGreaterThan(0);page=context.pages()[0];
 await expect.poll(async()=>{try{return(await update('status')).currentVersion;}catch{return '';}},{timeout:60000}).not.toBe('');
 // Do not alter onboarding or switch accounts/profiles in a production trial.
 await expect(page.locator('.app-shell')).toBeVisible({timeout:60000});
 await expect(page.getByRole('main')).toHaveAttribute('aria-busy','false',{timeout:60000});
}
try{
 const candidate=JSON.parse((await readFile(resolve(option('--receipt')),'utf8')).replace(/^\uFEFF/,''));
 expect(candidate.version).toBe(TO);expect(candidate.profile).toBe('release');expect(candidate.isolatedUpdateTest).toBe(false);expect(candidate.updaterSigned).toBe(true);expect(candidate.files).toHaveLength(1);
 const artifact=candidate.files[0],expectedArtifact=join(appRoot,'src-tauri','target','release','bundle','nsis',`OpenKoda_${TO}_x64-setup.exe`);
 expect(resolve(artifact.path)).toBe(expectedArtifact);expect(resolve(artifact.updaterSignature)).toBe(expectedArtifact+'.sig');expect(resolve(candidate.releaseRoot)).toBe(join(appRoot,'src-tauri','target','release'));
 await plainPath(executable);await plainPath(data);await plainPath(expectedArtifact);
 const artifactBytes=await readFile(expectedArtifact);expect(artifactBytes.length).toBe(artifact.bytes);expect(sha(artifactBytes)).toBe(artifact.sha256);
 const signature=(await readFile(artifact.updaterSignature,'utf8')).trim();expect(signature.length).toBeGreaterThan(100);
 const config=JSON.parse(await readFile(join(appRoot,'src-tauri','tauri.conf.json'),'utf8'));
 expect(config.identifier).toBe('com.openkoda.desktop');expect(config.productName).toBe('OpenKoda');expect(config.plugins.updater.endpoints).toEqual([endpoint]);expect(config.plugins.updater.requireSignedVersion).toBe(true);
 expect(sha(await readFile(executable))).toBe(OLD_INSTALLED_SHA256);expect(installedVersion()).toMatch(/^1\.1\.0(?:\.0)?$/);
 runPs(`$r=Get-ItemProperty -LiteralPath ${psQuote(registration)} -ErrorAction Stop;if($r.DisplayVersion -ne ${psQuote(FROM)} -or $r.InstallLocation.Trim('"') -ne ${psQuote(install)}){throw 'Unexpected installed registration.'}`);
 assertNoOpenKoda();
 // This read verifies the artifact currently offered. The application still
 // checks/downloads it itself; no cached bytes or injected feed are used.
 const response=await fetch(endpoint,{signal:AbortSignal.timeout(25000)});if(!response.ok)throw new Error('Official update feed is unavailable.');
 const manifest=await response.json();expect(manifest.version).toBe(TO);expect(manifest.platforms['windows-x86_64'].signature.trim()).toBe(signature);
 expect(manifest.platforms['windows-x86_64'].url).toBe(`https://github.com/joaquimmoraesgabriel-cmyk/OpenKoda/releases/download/desktop-v${TO}/${basename(expectedArtifact)}`);
 // Rust canonicalize() uses the Windows verbatim path. Hash that exact spelling,
 // including its namespace prefix, rather than the USERPROFILE input spelling.
 const canonicalBase=toNamespacedPath(await realpath(data));
 const digest=createHash('sha256').update(canonicalBase).digest(),bucket=digest.readUInt16LE(0)%100;
 receipt.canonicalBase=canonicalBase;receipt.rolloutBucket=bucket;receipt.rolloutPercent=manifest.rolloutPercent;
 if(!Number.isInteger(manifest.rolloutPercent)||bucket>=manifest.rolloutPercent)throw new Error('This installation is not in the published rollout. The test will not bypass it.');
 receipt.candidate={version:TO,bytes:artifact.bytes,sha256:artifact.sha256,authenticode:artifact.authenticode};
 receipt.before=fingerprint();receipt.existingBackupManifests=backupManifests();await mkdir(directory,{recursive:true});
 const server=createServer();await new Promise((ok,fail)=>{server.once('error',fail);server.listen(0,'127.0.0.1',ok);});port=server.address().port;await new Promise(ok=>server.close(ok));
 const env={...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`};delete env.OPENKODA_DATA_DIR;delete env.OPENKODA_BUILD_TEST_DATA_DIR;
 assertNoOpenKoda();child=spawn(executable,[],{cwd:install,windowsHide:true,stdio:'ignore',env});child.on('error',()=>{});receipt.ownedPid=child.pid;
 await connect();expect((await update('status')).currentVersion).toBe(FROM);receipt.accountBefore=await accountIdentity();receipt.preferencesBefore=await preferences();
 note('production 1.1.0 opened; no projects or account modified');
 await page.getByRole('button',{name:'Configurações',exact:true}).click();await page.getByRole('button',{name:'Atualizações',exact:true}).click();
 const settings=()=>page.locator('.desktop-update-settings');await expect(settings()).toBeVisible();receipt.checkAttempts=[];
 let available;
 for(let attempt=1;attempt<=3;attempt++){
  available=await update('status');
  if(available.phase!=='available'){
   // The automatic startup check is attempt one, even when it already failed
   // before Playwright attached. At most two further checks are issued here.
   if(attempt>1||available.phase==='idle'){
    if(attempt>1)await new Promise(done=>setTimeout(done,16000));
    await settings().getByRole('button',{name:'Verificar atualizações',exact:true}).click();
   }
   await expect.poll(async()=>{available=await update('status');return ['checking','idle'].includes(available.phase)?'pending':available.phase;},{timeout:30000}).not.toBe('pending');
  }
  receipt.checkAttempts.push({attempt,at:new Date().toISOString(),status:available});
  if(available.phase==='available')break;
  if(attempt===3||available.phase!=='error')throw new Error('Official native update check did not offer the expected candidate.');
 }
 expect(available.version).toBe(TO);
 if(pauseAfterDiscovery){
  const releaseFile=join(directory,'resume-after-channel-paused.json');
  await writeFile(join(directory,'discovery.json'),JSON.stringify({version:available.version,phase:available.phase,releaseFile,at:new Date().toISOString()},null,2));
  console.log(JSON.stringify({step:'official candidate discovered; pause public rollout before releasing download',directory,releaseFile,at:new Date().toISOString()}));
  await expect.poll(async()=>{try{const value=JSON.parse(await readFile(releaseFile,'utf8'));return value.version===TO&&value.rolloutPercent===0;}catch{return false;}},{timeout:300000,intervals:[1000]}).toBe(true);
  // The coordinator confirms its public-channel write through the gate file.
  // Keep the native Update object already discovered; do not replace its URL,
  // signature or bytes and do not trigger another application check.
  receipt.channelPausedBeforeDownload=true;
 }
 note('official candidate retained in native updater; ready for download');
 await expect(page.locator('.desktop-update-notice')).toBeVisible();await page.getByRole('button',{name:'Adiar atualização',exact:true}).click();
 expect((await update('status')).deferred).toBe(true);await expect(page.locator('.desktop-update-notice')).toHaveCount(0);receipt.checks.push('official update offer, settings and defer');
 receipt.downloadAttempts=[];
 for(let attempt=1;attempt<=2;attempt++){
  const previous=await update('status');let began=false,settled;
  downloadInFlight=true;await settings().getByRole('button',{name:'Baixar atualização',exact:true}).click();note(`native production download attempt ${attempt}`);
  await expect(settings().getByRole('progressbar',{name:'Download da atualização'})).toBeVisible();
  await expect.poll(async()=>{settled=await update('status');if(settled.phase==='downloading')began=true;return settled.phase==='ready'||(['available','error'].includes(settled.phase)&&(began||settled.message!==previous.message))?settled.phase:'pending';},{timeout:660000,intervals:[1000,2000,3000]}).not.toBe('pending');
  downloadInFlight=false;
  receipt.downloadAttempts.push({attempt,at:new Date().toISOString(),status:settled});
  if(settled.phase==='ready')break;
  if(attempt===2||!/Download interrompido|GitHub não disponibilizou/.test(settled.message))throw new Error(`Native download failed: ${settled.message}`);
  await new Promise(done=>setTimeout(done,20000));
 }
 const ready=await update('status');expect(ready.phase).toBe('ready');expect(ready.version).toBe(TO);expect(ready.downloadedBytes).toBe(artifact.bytes);
 receipt.preInstall=fingerprint();receipt.preInstallComparison=compareData(receipt.before,receipt.preInstall);expect(await accountIdentity()).toBe(receipt.accountBefore);
 const others=processList().filter(process=>process.pid!==child.pid);if(others.length)throw new Error('Another OpenKoda process opened during the trial; installation has not started.');
 await page.screenshot({path:join(directory,'ready.png'),fullPage:true});receipt.checks.push('native real download, progress and signature validation');
 installationStarted=true;receipt.installStartedAt=new Date().toISOString();note('native backup and production installation beginning');
 const click=settings().getByRole('button',{name:'Instalar e reiniciar',exact:true}).click().catch(()=>undefined);
 await expect.poll(()=>child.exitCode,{timeout:120000}).not.toBe(null);await click;await disconnect();child=undefined;
 await expect.poll(()=>runPs(`(Get-ItemProperty -LiteralPath ${psQuote(registration)} -ErrorAction Stop).DisplayVersion`).trim(),{timeout:480000,intervals:[1000,2000,5000]}).toBe(TO);
 // Never fake a successful restart by launching another executable here.
 await connect();expect((await update('status')).currentVersion).toBe(TO);expect(installedVersion()).toMatch(/^1\.1\.1(?:\.0)?$/);
 receipt.installedBinary=python(await readFile(join(appRoot,'scripts','verify-bundled-binary.py'),'utf8'),join(candidate.releaseRoot,'openkoda.exe'),executable);
 const restarted=processList().filter(process=>process.path?.toLowerCase()===executable.toLowerCase()&&Date.parse(process.created)>=Date.parse(receipt.installStartedAt));
 expect(restarted).toHaveLength(1);receipt.restartedProcess=restarted[0];restartVerified=true;
 receipt.accountAfter=await accountIdentity();expect(receipt.accountAfter).toBe(receipt.accountBefore);receipt.preferencesAfter=await preferences();expect(receipt.preferencesAfter).toBe(receipt.preferencesBefore);
 receipt.after=fingerprint();receipt.preservation=compareData(receipt.before,receipt.after);receipt.backup=verifyBackup(receipt.preInstall);
 await page.getByRole('button',{name:'Configurações',exact:true}).click();await page.getByRole('button',{name:'IA e modelos',exact:true}).click();
 await expect(page.getByRole('complementary',{name:'Provedores configurados',exact:true}).getByRole('button',{name:/^Koda /})).toBeVisible();
 await page.screenshot({path:join(directory,'updated-koda.png'),fullPage:true});receipt.checks.push('NSIS installed and automatically restarted 1.1.1','all project/chat/budget/inactive-profile SQLite contents and preferences preserved','same commercial account identity','native backup inventory, hashes, quick_check and contents','Koda provider visible in installed application');receipt.passed=true;note('production upgrade verified');
}catch(error){receipt.error=String(error);if(page)receipt.lastUpdateStatus=await update('status').catch(()=>undefined);preserveProcess=installationStarted||downloadInFlight||['downloading','installing'].includes(receipt.lastUpdateStatus?.phase);await page?.screenshot({path:join(directory,'failure.png'),fullPage:true}).catch(()=>{});process.exitCode=1;}
finally{
 // Keep the verified new application open. On any installation uncertainty or
 // active download, preserve the app/installer rather than interrupting it.
 if(preserveProcess||restartVerified){receipt.leftRunning={pid:child?.pid??receipt.restartedProcess?.pid,port,reason:restartVerified?'Verified automatically restarted app left open':'Download or installation may still be in progress; no process stopped'};child?.unref();await disconnect();}
 else {await disconnect();if(child&&child.exitCode===null){const owned=processList().find(process=>process.pid===child.pid&&process.path?.toLowerCase()===executable.toLowerCase());if(owned){child.kill();await new Promise(done=>{child.once('exit',done);setTimeout(done,5000);});}}}
 receipt.finishedAt=new Date().toISOString();await mkdir(directory,{recursive:true});await writeFile(join(directory,'result.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt,null,2));
}
