// Controlled native fixture for an observed Windows screen-reader walkthrough.
// IPC is used only to prepare synthetic data. UI inputs belong to the reviewer.
import {chromium,expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import {createServer} from 'node:net';
import {mkdir,readFile,writeFile,access} from 'node:fs/promises';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID,createHash} from 'node:crypto';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const release=JSON.parse(await readFile(join(root,'releases/0.1.1-local-20261107/release.json'),'utf8'));
const executable=release.nativeTestBinary;
const directory=join(root,'.local',`screen-reader-${randomUUID()}`);
await mkdir(join(directory,'source'),{recursive:true});
await writeFile(join(directory,'source','sample.txt'),'Synthetic accessibility fixture. No personal data.\n');
const binarySha256=createHash('sha256').update(await readFile(executable)).digest('hex');
if(binarySha256!==release.nativeTestBinarySha256)throw new Error('Preserved native binary changed.');
const receipt={passed:false,directory,binarySha256,version:release.version,observations:[]};
let child,browser;
try{
 const listener=createServer();await new Promise((ok,fail)=>{listener.once('error',fail);listener.listen(0,'127.0.0.1',ok);});
 const port=listener.address().port;await new Promise(ok=>listener.close(ok));
 child=spawn(executable,[],{cwd:root,windowsHide:true,stdio:'ignore',env:{...process.env,OPENKODA_DATA_DIR:join(directory,'data'),WEBVIEW2_USER_DATA_FOLDER:join(directory,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`}});
 child.on('error',error=>{receipt.spawnError=String(error);});
 await expect.poll(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:45000}).toBe(true);
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);const context=browser.contexts()[0];await expect.poll(()=>context.pages().length).toBeGreaterThan(0);const page=context.pages()[0];
 await expect(page.locator('main')).toHaveAttribute('aria-busy','false',{timeout:45000});
 const request=(action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}),{action,payload});
 const project=await request('project.create',{name:'Laboratório de acessibilidade',kind:'files',target:join(directory,'source'),description:'Somente dados sintéticos do teste de leitor de tela.'});
 const assessment=await request('assessment.create',{projectId:project.id,title:'Revisão com leitor de tela',kind:project.kind,target:project.target,objective:'Verificar nomes, estados e navegação acessíveis.'});
 const evidence=await request('evidence.import',{assessmentId:assessment.id,name:'Observação sintética',sourceDescription:'Fixture local, não verificada.',content:'Exemplo sintético para navegação. Não corresponde a uma vulnerabilidade real.',acknowledgeUnverifiedSource:true});
 const finding=await request('candidate.record',{assessmentId:assessment.id,title:'Candidato sintético para leitura',description:'Somente um exemplo de interface, sem conclusão de segurança.',severity:'low',recommendation:'Revisar o exemplo e manter os limites explícitos.',evidenceIds:[evidence.id]});
 await page.reload();await expect(page.locator('main')).toHaveAttribute('aria-busy','false',{timeout:45000});
 const before=await request('snapshot');
 const metrics=()=>page.evaluate(()=>({title:document.querySelector('main h1')?.textContent??'',focus:document.activeElement?.getAttribute('aria-label')??document.activeElement?.textContent?.slice(0,100),width:innerWidth,scrollWidth:document.documentElement.scrollWidth}));
 const ready={directory,pid:child.pid,port,projectId:project.id,assessmentId:assessment.id,findingId:finding.id,baseline:await metrics()};
 await writeFile(join(directory,'ready.json'),JSON.stringify(ready,null,2));await writeFile(join(root,'.local/screen-reader-current.json'),JSON.stringify(ready,null,2));console.log(JSON.stringify({ready:true,...ready}));
 const deadline=Date.now()+25*60*1000;let finish;
 while(Date.now()<deadline){
  const sample=await metrics();if(receipt.observations.at(-1)?.title!==sample.title)receipt.observations.push(sample);
  try{await access(join(directory,'finish.json'));finish=JSON.parse(await readFile(join(directory,'finish.json'),'utf8'));break;}catch{}
  await new Promise(ok=>setTimeout(ok,250));
 }
 if(!finish)throw new Error('No completed screen-reader observation receipt was provided.');
 receipt.review=finish;expect(finish.reader).toBe('NVDA 2026.3');expect(finish.observations.length).toBeGreaterThan(0);
 expect(await request('snapshot')).toEqual(before);receipt.domainPreserved=true;
 expect((await request('ai.status')).hasApiKey).toBe(false);
 receipt.passed=finish.passed===true;
 if(!receipt.passed)process.exitCode=1;
}catch(error){receipt.error=String(error);process.exitCode=1;}
finally{
 await browser?.close().catch(()=>{});
 if(child&&child.exitCode===null){child.kill();await new Promise(ok=>{child.once('exit',ok);setTimeout(ok,5000);});}
 receipt.generatedAt=new Date().toISOString();await writeFile(join(directory,'result.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify({passed:receipt.passed,error:receipt.error,directory}));
}
