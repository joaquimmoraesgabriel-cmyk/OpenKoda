// Companion to a human/computer-use keyboard check of the packaged WebView.
// Starts one owned process; no provider, project or settings mutation is sent.
import {chromium,expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import {createServer} from 'node:net';
import {mkdir,readFile,writeFile,access} from 'node:fs/promises';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID,createHash} from 'node:crypto';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const executable=join(root,'src-tauri/target/release/openkoda.exe');
const directory=join(root,'.local',`native-display-${randomUUID()}`);await mkdir(directory,{recursive:true});
const receipt={passed:false,directory,binarySha256:createHash('sha256').update(await readFile(executable)).digest('hex'),observations:[]};
let child,browser;
try{
 const server=createServer();await new Promise((ok,fail)=>{server.once('error',fail);server.listen(0,'127.0.0.1',ok);});const port=server.address().port;await new Promise(ok=>server.close(ok));
 const env={...process.env,WEBVIEW2_USER_DATA_FOLDER:join(directory,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`};delete env.OPENKODA_DATA_DIR;
 child=spawn(executable,[],{cwd:root,windowsHide:true,stdio:'ignore',env});child.on('error',()=>{});
 await expect.poll(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:45000}).toBe(true);
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);const context=browser.contexts()[0];await expect.poll(()=>context.pages().length).toBeGreaterThan(0);const page=context.pages()[0];
 await expect(page.locator('main')).toHaveAttribute('aria-busy','false',{timeout:60000});
 const state=()=>page.evaluate(()=>window.__TAURI_INTERNALS__.invoke('request',{request:{action:'snapshot',payload:{}}}));
 const metrics=()=>page.evaluate(()=>({dpr:devicePixelRatio,width:innerWidth,height:innerHeight,scrollWidth:document.documentElement.scrollWidth,title:document.querySelector('main h1')?.textContent??'',focus:document.activeElement?.getAttribute('aria-label')??document.activeElement?.textContent?.slice(0,100)}));
 const before=await state();receipt.baseline=await metrics();receipt.observations.push(receipt.baseline);
 const ready={directory,pid:child.pid,port,baseline:receipt.baseline};await writeFile(join(directory,'ready.json'),JSON.stringify(ready,null,2));await writeFile(join(root,'.local/native-display-current.json'),JSON.stringify(ready,null,2));console.log(JSON.stringify({ready:true,...ready}));
 const deadline=Date.now()+300000;let finish;
 while(Date.now()<deadline){
  const sample=await metrics();const last=receipt.observations.at(-1);if(sample.dpr!==last.dpr||sample.title!==last.title||sample.width!==last.width)receipt.observations.push(sample);
  try{await access(join(directory,'finish.json'));finish=JSON.parse(await readFile(join(directory,'finish.json'),'utf8'));break;}catch{}
  await new Promise(ok=>setTimeout(ok,250));
 }
 if(!finish)throw new Error('Keyboard check did not provide a completion receipt within five minutes.');
 receipt.keyboardReview=finish;receipt.final=await metrics();receipt.maximumZoomRatio=Math.max(...receipt.observations.map(s=>s.dpr))/receipt.baseline.dpr;
 expect(receipt.maximumZoomRatio).toBeGreaterThanOrEqual(1.9);expect(receipt.final.dpr).toBe(receipt.baseline.dpr);
 expect(receipt.observations.filter(s=>s.dpr/receipt.baseline.dpr>=1.9).every(s=>s.scrollWidth<=s.width+1)).toBe(true);
 expect(await state()).toEqual(before);receipt.domainPreserved=true;receipt.passed=true;
}catch(error){receipt.error=String(error);process.exitCode=1;}
finally{await browser?.close().catch(()=>{});if(child&&child.exitCode===null)child.kill();receipt.generatedAt=new Date().toISOString();await writeFile(join(directory,'result.json'),JSON.stringify(receipt,null,2));console.log(JSON.stringify({passed:receipt.passed,error:receipt.error,directory,maximumZoomRatio:receipt.maximumZoomRatio}));}
