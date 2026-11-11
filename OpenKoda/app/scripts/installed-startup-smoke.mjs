// Opens a specified packaged executable using the real profile. No provider or
// evaluation action is dispatched; receipts contain only hashes and counts.
import {chromium,expect} from '@playwright/test';
import {spawn,execFileSync} from 'node:child_process';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {createServer} from 'node:net';
import {dirname,resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID,createHash} from 'node:crypto';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
if(!process.argv[2])throw new Error('Pass the exact installed executable path.');
const executable=resolve(process.argv[2]);
const directory=join(root,'.local',`installed-startup-${randomUUID()}`);await mkdir(directory,{recursive:true});
const fingerprint=()=>JSON.parse(execFileSync('python',['-c',String.raw`
import json,sqlite3,hashlib,sys,uuid
from pathlib import Path
base=Path(sys.argv[1]); pointer=base/'active-profile.json'; active=base
if pointer.exists():
 p=json.loads(pointer.read_text(encoding='utf-8'))
 # Resolve the same bounded profile convention used by storage.rs.
 ref=p['profile']
 if ref['kind']=='profile':
  identity=ref['id']; assert str(uuid.UUID(identity))==identity
  active=base/'profiles'/identity
 else: assert ref['kind']=='base'
db=sqlite3.connect((active/'openkoda.sqlite3').as_uri()+'?mode=ro',uri=True)
state=json.loads(db.execute('SELECT json FROM app_state WHERE id=1').fetchone()[0]); db.close()
digest=hashlib.sha256(json.dumps(state,sort_keys=True,separators=(',',':'),ensure_ascii=True).encode()).hexdigest()
ai=sqlite3.connect((active/'assistant.sqlite3').as_uri()+'?mode=ro',uri=True)
requests=ai.execute('SELECT count(*) FROM requests').fetchone()[0]
ai.close()
print(json.dumps({'coreSha256':digest,'projects':len(state['projects']),'assessments':len(state['assessments']),'findings':len(state['findings']),'providerRequests':requests}))
`,join(process.env.USERPROFILE,'.openkoda')],{encoding:'utf8',windowsHide:true}));
const receipt={passed:false,executable,binarySha256:createHash('sha256').update(await readFile(executable)).digest('hex'),directory};
let child,browser;
async function close(){await browser?.close().catch(()=>{});browser=undefined;if(child&&child.exitCode===null){child.kill();await new Promise(ok=>{child.once('exit',ok);setTimeout(ok,5000);});}child=undefined;}
try{
 receipt.before=fingerprint();receipt.starts=[];
 for(let index=0;index<2;index++){
  const server=createServer();await new Promise((ok,fail)=>{server.once('error',fail);server.listen(0,'127.0.0.1',ok);});const port=server.address().port;await new Promise(ok=>server.close(ok));
  const env={...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`};delete env.OPENKODA_DATA_DIR;
  child=spawn(executable,[],{cwd:dirname(executable),windowsHide:true,stdio:'ignore',env});child.on('error',()=>{});
  await expect.poll(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:45000}).toBe(true);
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);const context=browser.contexts()[0];await expect.poll(()=>context.pages().length).toBeGreaterThan(0);const page=context.pages()[0];
  await expect(page.locator('main')).toHaveAttribute('aria-busy','false',{timeout:60000});
  const request=(action)=>page.evaluate(action=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload:{}}}),action);
  const snapshot=await request('snapshot');const ai=await request('ai.status');
  expect(snapshot.projects.length).toBe(receipt.before.projects);expect(snapshot.assessments.length).toBe(receipt.before.assessments);
  for(const name of ['Avaliações','Achados','Projetos','Kali e ferramentas','Configurações']){
   await page.getByRole('button',{name,exact:true}).click();await expect(page.locator('main')).toHaveAttribute('aria-busy','false');await expect(page.locator('main h1').first()).toBeVisible();
  }
  await page.getByRole('button',{name:'Nova avaliação',exact:true}).click();
  const hasActiveProject=snapshot.projects.some(project=>!project.archived);
  await expect(page.getByRole('heading',{name:hasActiveProject?'Nova avaliação':'Adicione um projeto primeiro',exact:true})).toBeVisible();
  receipt.starts.push({iteration:index+1,views:6,newAssessmentView:hasActiveProject?'form':'requires-project',credentialConfigured:ai.hasApiKey===true,connectionVerified:ai.verified===true,model:snapshot.settings.model});
  await close();
 }
 receipt.after=fingerprint();expect(receipt.after).toEqual(receipt.before);receipt.passed=true;
}catch(error){receipt.error=String(error);process.exitCode=1;}
finally{await close();receipt.generatedAt=new Date().toISOString();await writeFile(join(directory,'result.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt,null,2));}
