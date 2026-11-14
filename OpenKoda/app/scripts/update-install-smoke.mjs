// Real packaged updater rehearsal. Only the separately named, compiled fixture
// is installed; the production profile is fingerprinted but never opened by it.
import {chromium,expect} from '@playwright/test';
import {readFile,writeFile,mkdir} from 'node:fs/promises';
import {spawn,execFileSync} from 'node:child_process';
import {resolve,dirname,join} from 'node:path';
import {createServer} from 'node:net';
import {randomUUID} from 'node:crypto';
import {fileURLToPath} from 'node:url';
const input=process.argv[2];if(!input)throw new Error('Pass the rehearsal.json emitted by build-update-rehearsal.ps1.');
const resume=process.argv.includes('--resume');
const rehearsal=JSON.parse((await readFile(resolve(input),'utf8')).replace(/^\uFEFF/,''));
if(rehearsal.schema!=='openkoda.update-rehearsal.v1'||!rehearsal.testSigningOnly)throw new Error('Not an isolated updater rehearsal.');
const base=resolve(rehearsal.directory),data=resolve(rehearsal.dataDirectory),install=resolve(rehearsal.installDirectory);
const appRoot=resolve(dirname(fileURLToPath(import.meta.url)),'..');
if(!base.toLowerCase().startsWith(join(appRoot,'.local','update-rehearsal-').toLowerCase())||!data.startsWith(base+'\\')||!install.startsWith(base+'\\'))throw new Error('Fixture paths must stay in the isolated workspace.');
const runPs=script=>execFileSync('powershell.exe',['-NoProfile','-NonInteractive','-Command',script],{encoding:'utf8',windowsHide:true});
const psQuote=text=>`'${text.replaceAll("'","''")}'`;
const registration='HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\OpenKodaUpdateTest';
runPs(resume?`$registered=Get-ItemProperty -LiteralPath ${psQuote(registration)} -ErrorAction Stop;if($registered.DisplayVersion-ne '1.0.9' -or $registered.InstallLocation.Trim('"')-ne ${psQuote(install)}){throw 'Resume requires the same isolated 1.0.9 installation.'}`:`if(Test-Path -LiteralPath ${psQuote(registration)}){throw 'A previous test registration exists. Inspect it before another rehearsal.'}`);
const requireNoOtherOpenKoda=()=>runPs(`$others=@(Get-CimInstance Win32_Process -Filter "Name='openkoda.exe'" | Where-Object {$_.ExecutablePath-ne ${psQuote(join(install,'openkoda.exe'))}});if($others.Count){throw 'Close other OpenKoda installations before testing NSIS. They will not be stopped by this test.'}`);
requireNoOtherOpenKoda();
const fingerprint=path=>JSON.parse(execFileSync('python',['-c',String.raw`
import pathlib,sqlite3,hashlib,json,sys,stat
root=pathlib.Path(sys.argv[1]);result={}
if root.exists():
 for dbpath in sorted(list(root.glob('*.sqlite3'))+list(root.glob('profiles/*/*.sqlite3'))):
  for node in (dbpath,*dbpath.parents):
   metadata=node.lstat()
   if node.is_symlink() or getattr(metadata,'st_file_attributes',0)&0x400:raise ValueError('Profile contains a reparse point; nothing was changed')
   if node==root:break
  db=sqlite3.connect(dbpath.as_uri()+'?mode=ro',uri=True)
  h=hashlib.sha256()
  for line in db.iterdump():h.update(line.encode())
  db.close();result[str(dbpath.relative_to(root))]=h.hexdigest()
print(json.dumps(result))
`,path],{encoding:'utf8',windowsHide:true}));
const realRoot=join(process.env.USERPROFILE,'.openkoda');
const receipt={passed:false,resumed:resume,directory:base,dataDirectory:data,productionBefore:fingerprint(realRoot),checks:[]};
const note=step=>console.log(JSON.stringify({step,at:new Date().toISOString()}));
const verifyInstalled=release=>JSON.parse(execFileSync('python',[join(appRoot,'scripts','verify-bundled-binary.py'),release.binary,join(install,'openkoda.exe')],{encoding:'utf8',windowsHide:true}));
let child,browser,page,port,preserveDownload=false;
const invoke=(action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}),{action,payload});
const update=action=>page.evaluate(action=>window.__TAURI_INTERNALS__.invoke('desktop_update',{input:{action}}),action);
async function disconnect(){await browser?.close().catch(()=>{});browser=undefined;page=undefined;}
async function closeOwned(){await disconnect();if(child&&child.exitCode===null){child.kill();await new Promise(done=>{child.once('exit',done);setTimeout(done,5000);});}child=undefined;}
async function connect(){
 await expect.poll(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:60000}).toBe(true);
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);const context=browser.contexts()[0];await expect.poll(()=>context.pages().length).toBeGreaterThan(0);page=context.pages()[0];
 await expect.poll(async()=>{try{return(await update('status')).currentVersion;}catch{return '';}},{timeout:60000}).not.toBe('');
 await page.locator('.app-shell, .welcome-entry').first().waitFor({state:'visible',timeout:60000});
 const local=page.getByRole('button',{name:'Continuar com minha chave de API'});
 if(await local.isVisible().catch(()=>false))await local.click();
 await expect(page.locator('.app-shell')).toBeVisible({timeout:60000});
}
async function start(){
 const server=createServer();await new Promise((done,fail)=>{server.once('error',fail);server.listen(0,'127.0.0.1',done);});port=server.address().port;await new Promise(done=>server.close(done));
 child=spawn(join(install,'openkoda.exe'),[],{cwd:install,windowsHide:true,stdio:'ignore',env:{...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`}});child.on('error',()=>{});await connect();
}
try{
 await mkdir(install,{recursive:true});
 const old=rehearsal.releases.find(item=>item.version==='1.0.9');if(!old)throw new Error('Old rehearsal package missing.');
 if(!resume){note('installing isolated 1.0.9');runPs(`$p=Start-Process -FilePath ${psQuote(old.installer)} -ArgumentList @('/S','/NS',${psQuote('/D='+install)}) -WindowStyle Hidden -PassThru; if(!$p.WaitForExit(300000)){throw 'Test installer timed out'}; if($p.ExitCode-ne 0){throw 'Test installer failed'}`);}
 receipt.oldBinary=verifyInstalled(old);receipt.oldFileVersion=runPs(`(Get-Item -LiteralPath ${psQuote(join(install,'openkoda.exe'))}).VersionInfo.FileVersion`).trim();expect(receipt.oldFileVersion).toMatch(/^1\.0\.9(?:\.0)?$/);
 await start();expect((await update('status')).currentVersion).toBe('1.0.9');receipt.checks.push('old installed executable reports 1.0.9');note('installed 1.0.9 verified');
 const source=join(base,'source');let project,conversation;
 if(resume){const snapshot=await invoke('snapshot');expect(snapshot.projects).toHaveLength(1);expect(snapshot.assessments).toHaveLength(1);project=snapshot.projects[0];conversation=snapshot.assessments[0];expect(project.name).toBe('Project preserved across update');expect(conversation.title).toBe('Conversation preserved across update');expect(await readFile(join(source,'example.txt'),'utf8')).toBe('Synthetic update rehearsal.');}
 else {await mkdir(source,{recursive:true});await writeFile(join(source,'example.txt'),'Synthetic update rehearsal.');project=await invoke('project.create',{name:'Project preserved across update',kind:'code',target:source,description:'Synthetic fixture only'});conversation=await invoke('project.newConversation',{projectId:project.id,title:'Conversation preserved across update',idempotencyKey:randomUUID()});}
 if(!resume)await closeOwned();
 if(!resume)execFileSync('python',['-c',String.raw`
import sqlite3,pathlib,sys,uuid,datetime
root=pathlib.Path(sys.argv[1]);aid=sys.argv[2]
db=sqlite3.connect(root/'assistant.sqlite3');db.execute('INSERT INTO messages(id,assessment,role,text,created) VALUES(?,?,?,?,?)',(str(uuid.uuid4()),aid,'user','A synthetic conversation must survive this update.',datetime.datetime.now(datetime.timezone.utc).isoformat()));db.commit();db.close()
inactive=root/'profiles'/str(uuid.uuid4());inactive.mkdir(parents=True)
for path in root.glob('*.sqlite3'):
 source=sqlite3.connect(path);target=sqlite3.connect(inactive/path.name);source.backup(target);target.close();source.close()
`,data,conversation.id],{windowsHide:true});
 receipt.before=fingerprint(data);note(resume?'existing synthetic data preserved for retry':'synthetic projects, conversation and inactive profile seeded');if(!resume)await start();
 let available;receipt.checkAttempts=[];
 for(let attempt=1;attempt<=3;attempt++){
  await update('check');
  await expect.poll(async()=>{available=await update('status');return ['checking','idle'].includes(available.phase)?'pending':available.phase;},{timeout:30000}).not.toBe('pending');
  receipt.checkAttempts.push({attempt,at:new Date().toISOString(),status:available});
  if(available.phase==='available')break;
  if(attempt===3||available.phase!=='error')throw new Error(`Native GitHub check failed: ${available.message}`);
  note('transient GitHub manifest failure; bounded retry after cooldown');await new Promise(done=>setTimeout(done,16000));
 }
 expect(available.version).toBe('1.1.0');
 note('GitHub update 1.1.0 available');
 await page.locator('.desktop-update-notice').getByRole('button',{name:'Detalhes'}).click();
 await expect(page.locator('.desktop-update-settings')).toBeVisible();
 await page.getByRole('button',{name:'Adiar atualização'}).click();
 expect((await update('status')).deferred).toBe(true);await expect(page.locator('.desktop-update-notice')).toHaveCount(0);receipt.checks.push('GitHub channel finds 1.1.0; native notice, settings and defer work');
 receipt.downloadAttempts=[];
 for(let attempt=1;attempt<=2;attempt++){
  const beforeAttempt=await update('status');let began=false;
  await page.locator('.desktop-update-settings').getByRole('button',{name:'Baixar atualização'}).click();
  note(`downloading update through native UI; attempt ${attempt}`);
  await expect(page.locator('.desktop-update-settings progress')).toBeVisible();
  // Each real request has its own deadline, longer than the native 600s limit.
  // A failed request becomes available again and is observed immediately.
  let settled;
  await expect.poll(async()=>{settled=await update('status');if(settled.phase==='downloading')began=true;return settled.phase==='ready'||(['available','error'].includes(settled.phase)&&(began||settled.message!==beforeAttempt.message))?settled.phase:'pending';},{timeout:660000,intervals:[1000,2000,3000]}).not.toBe('pending');
  receipt.downloadAttempts.push({attempt,at:new Date().toISOString(),status:settled});
  if(settled.phase==='ready')break;
  if(attempt===2||!/Download interrompido|GitHub não disponibilizou/.test(settled.message))throw new Error(`Native download failed: ${settled.message}`);
  note('transient GitHub download failure; retrying once after 20s');await new Promise(done=>setTimeout(done,20000));
 }
 const downloaded=await update('status');expect(downloaded.downloadedBytes).toBeGreaterThan(1000000);receipt.downloadedBytes=downloaded.downloadedBytes;await page.screenshot({path:join(base,'ready.png'),fullPage:true});receipt.checks.push('real GitHub installer download, UI progress and signature verification');
 note('download and signature verified');
 // The successful Windows install exits the old process, so the IPC promise
 // disconnects. That disconnect itself is not treated as proof of success.
 requireNoOtherOpenKoda();
 note('installing and restarting from native UI');
 const installing=page.locator('.desktop-update-settings').getByRole('button',{name:'Instalar e reiniciar'}).click().catch(()=>undefined);
 await expect.poll(()=>child.exitCode,{timeout:120000}).not.toBe(null);await installing;await disconnect();child=undefined;
 await expect.poll(()=>runPs(`(Get-ItemProperty -LiteralPath ${psQuote(registration)} -ErrorAction Stop).DisplayVersion`).trim(),{timeout:480000,intervals:[1000,2000,5000]}).toBe('1.1.0');
 // /R restarts the app inheriting the WebView debugging endpoint.
 await connect();expect((await update('status')).currentVersion).toBe('1.1.0');
 note('restarted 1.1.0 verified');
 receipt.newBinary=verifyInstalled(rehearsal.releases.find(item=>item.version==='1.1.0'));receipt.newFileVersion=runPs(`(Get-Item -LiteralPath ${psQuote(join(install,'openkoda.exe'))}).VersionInfo.FileVersion`).trim();expect(receipt.newFileVersion).toMatch(/^1\.1\.0(?:\.0)?$/);
 const snapshot=await invoke('snapshot');expect(snapshot.projects.some(item=>item.id===project.id)).toBe(true);expect(snapshot.assessments.some(item=>item.id===conversation.id)).toBe(true);expect(await readFile(join(source,'example.txt'),'utf8')).toBe('Synthetic update rehearsal.');
 receipt.checks.push('NSIS updates, restarts and reports 1.1.0');
 receipt.after=fingerprint(data);expect(receipt.after).toEqual(receipt.before);receipt.checks.push('all active and inactive SQLite logical hashes preserved');
 receipt.backup=JSON.parse(execFileSync('python',['-c',String.raw`
import pathlib,json,hashlib,sqlite3,sys
root=pathlib.Path(sys.argv[1]);expected=json.loads(sys.argv[2]);manifests=list((root/'update-backups').glob('*/manifest.json'))
assert len(manifests)==1,'Expected one complete pre-install backup'
manifest=json.loads(manifests[0].read_text(encoding='utf-8'));assert manifest['complete'] and manifest['fromVersion']=='1.0.9' and manifest['toVersion']=='1.1.0'
checked=[]
for entry in manifest['files']:
 path=manifests[0].parent/entry['area']/entry['name'];assert hashlib.sha256(path.read_bytes()).hexdigest()==entry['sha256']
 db=sqlite3.connect(path.as_uri()+'?mode=ro',uri=True);assert db.execute('PRAGMA quick_check').fetchone()[0]=='ok'
 digest=hashlib.sha256()
 for line in db.iterdump():digest.update(line.encode())
 db.close();assert digest.hexdigest()==expected[entry['name']];checked.append(entry['name'])
assert 'openkoda.sqlite3' in checked and 'assistant.sqlite3' in checked
print(json.dumps({'fromVersion':manifest['fromVersion'],'toVersion':manifest['toVersion'],'verifiedDatabases':checked}))
`,data,JSON.stringify(receipt.before)],{encoding:'utf8',windowsHide:true}));receipt.checks.push('pre-install backup versions, integrity, SHA-256 and logical contents verified');
 receipt.productionAfter=fingerprint(realRoot);expect(receipt.productionAfter).toEqual(receipt.productionBefore);await page.screenshot({path:join(base,'updated.png'),fullPage:true});receipt.checks.push('production profile unchanged');receipt.passed=true;
}catch(error){receipt.error=String(error);if(page)receipt.lastUpdateStatus=await update('status').catch(()=>undefined);preserveDownload=receipt.lastUpdateStatus?.phase==='downloading';await page?.screenshot({path:join(base,'failure.png'),fullPage:true}).catch(()=>{});process.exitCode=1;}
finally{
 if(preserveDownload){receipt.leftRunning={pid:child?.pid,port,reason:'Harness deadline; native download still running'};child?.unref();child=undefined;await disconnect();}else await closeOwned();
 // Relaunched process belongs to fixture; never stop another OpenKoda path.
 if(!preserveDownload)try {runPs(`$owned=@(Get-CimInstance Win32_Process -Filter "Name='openkoda.exe'" | Where-Object {$_.ExecutablePath-eq ${psQuote(join(install,'openkoda.exe'))}});foreach($process in $owned){Stop-Process -Id $process.ProcessId -ErrorAction Stop};exit 0`);}catch(error){receipt.cleanupError=String(error);receipt.passed=false;process.exitCode=1;}
 receipt.generatedAt=new Date().toISOString();await writeFile(join(base,'result.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt,null,2));
}
