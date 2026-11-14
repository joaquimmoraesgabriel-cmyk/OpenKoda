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
requests=ai.execute('SELECT * FROM requests ORDER BY id').fetchall()
messages=ai.execute('SELECT id,assessment,role,text,created FROM messages ORDER BY id').fetchall()
hash_rows=lambda rows: hashlib.sha256(json.dumps(rows,separators=(',',':'),ensure_ascii=True).encode()).hexdigest()
ai.close()
print(json.dumps({'coreSha256':digest,'projects':len(state['projects']),'assessments':len(state['assessments']),'findings':len(state['findings']),'providerRequests':len(requests),'providerRequestsSha256':hash_rows(requests),'chatMessages':len(messages),'chatMessagesSha256':hash_rows(messages)}))
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
  const errors=[];page.on('pageerror',error=>errors.push(error.name));
  const localChoice=page.getByRole('button',{name:'Continuar com minha chave de API',exact:true});
  const navigation=page.getByRole('navigation',{name:'Navegação principal'});
  await expect(localChoice.or(navigation)).toBeVisible({timeout:60000});
  const welcomed=await localChoice.isVisible();
  if(welcomed)await localChoice.click();
  await expect(navigation).toBeVisible();
  await expect(page.getByRole('main')).toHaveAttribute('aria-busy','false',{timeout:60000});
  const request=(action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}),{action,payload});
  const snapshot=await request('snapshot');const ai=await request('ai.status');
  expect(snapshot.projects.length).toBe(receipt.before.projects);expect(snapshot.assessments.length).toBe(receipt.before.assessments);
  // Start after bootstrap and the explicitly authorized local-mode choice.
  // Hash values without putting preferences or profile details in the receipt.
  const preferencesFingerprint=async()=>createHash('sha256').update(await page.evaluate(()=>JSON.stringify(
   Object.keys(localStorage).filter(key=>key.startsWith('openkoda.')).sort().map(key=>[key,localStorage.getItem(key)])
  ))).digest('hex');
  const preferencesBefore=await preferencesFingerprint();
  const screenshots=[],views=[];
  const capture=async name=>{const path=join(directory,`start-${index+1}-${name}.png`);await page.screenshot({path,animations:'disabled'});screenshots.push(path);};
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Perfil',exact:true,level:1})).toBeVisible();views.push('Perfil');await capture('profile');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  for(const name of ['Conta e segurança','Plano e assinatura','Aparência','IA e modelos','Histórico de uso','Atualizações']){
   await page.getByRole('button',{name,exact:true}).click();
   await expect(page.getByRole('region',{name:'Conteúdo de '+name,exact:true})).toBeVisible();views.push(name);
   if(name==='Aparência')await capture('appearance');
   if(name==='Atualizações')await capture('updates');
  }
  await page.getByRole('button',{name:'Início',exact:true}).click();
  await expect(page.getByRole('region',{name:'Área de projetos',exact:true})).toBeVisible();views.push('Início');
  const tree=page.getByRole('complementary',{name:'Projetos e conversas'});
  const showTree=async()=>{
   if(await tree.count()&&!await tree.isVisible()){
    const expand=page.getByRole('button',{name:'Mostrar lista de projetos',exact:true});
    if(await expand.isVisible())await expand.click();
    else await page.getByRole('button',{name:'Projetos',exact:true}).click();
   }
  };
  let projectsVisited=0,conversationsVisited=0;
  const visibleProjects=await tree.locator('.tree-project').count();
  for(let projectIndex=0;projectIndex<Math.min(3,visibleProjects);projectIndex++){
   await showTree();await tree.locator('.tree-project').nth(projectIndex).click();projectsVisited++;
   await showTree();const chats=tree.locator('.tree-group').nth(projectIndex).locator('.tree-chat');
   if(await chats.count()){
    await chats.first().click();await expect(page.getByRole('region',{name:'Histórico da conversa',exact:true})).toBeVisible();conversationsVisited++;
   }
  }
  await capture('workspace');
  // Read a bounded sample through the native history path. The full SQLite
  // fingerprint below checks every stored message, including hidden projects.
  const histories=[];
  for(const assessment of snapshot.assessments.slice(0,3)){
   const history=await request('ai.history',{assessmentId:assessment.id});
   expect(Array.isArray(history.messages)).toBe(true);
   histories.push({assessmentSha256:createHash('sha256').update(assessment.id).digest('hex'),messages:history.messages.length,
    messagesSha256:createHash('sha256').update(JSON.stringify(history.messages.map(({id,role,text,createdAt})=>({id,role,text,createdAt})))).digest('hex')});
  }
  expect(errors).toEqual([]);
  const preferencesAfter=await preferencesFingerprint();expect(preferencesAfter).toBe(preferencesBefore);
  receipt.starts.push({iteration:index+1,welcomed,views,projectsVisited,conversationsVisited,histories,screenshots,
   preferencesBeforeSha256:preferencesBefore,preferencesAfterSha256:preferencesAfter,
   credentialConfigured:ai.hasApiKey===true,connectionVerified:ai.verified===true});
  await close();
 }
 receipt.after=fingerprint();expect(receipt.after).toEqual(receipt.before);receipt.passed=true;
}catch(error){receipt.error=String(error);process.exitCode=1;}
finally{await close();receipt.generatedAt=new Date().toISOString();await writeFile(join(directory,'result.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify(receipt,null,2));}
