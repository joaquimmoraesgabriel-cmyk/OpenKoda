// Execute the product's reviewed dedicated-environment flow through real Tauri IPC.
// Explicit --apply is required; without it this script only retains a reviewable plan.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const directory=join(root,'.local',`kali-integration-${randomUUID()}`);
await mkdir(directory,{recursive:true});
const server=createServer();
await new Promise((ok,fail)=>{server.once('error',fail);server.listen(0,'127.0.0.1',ok);});
const port=server.address().port;await new Promise(ok=>server.close(ok));
let browser,child;
const result={passed:false,applied:false};
try {
 const env={...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`};delete env.OPENKODA_DATA_DIR;
 child=spawn(resolve(root,'src-tauri/target/debug/openkoda.exe'),[],{cwd:root,windowsHide:true,stdio:'ignore',env});child.on('error',()=>{});
 await expect.poll(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:30000}).toBe(true);
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);const context=browser.contexts()[0];
 await expect.poll(()=>context.pages().length,{timeout:15000}).toBeGreaterThan(0);const page=context.pages()[0];
 await page.waitForFunction(()=>typeof window.__TAURI_INTERNALS__?.invoke==='function');
 const request=(action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}),{action,payload});
 await expect(page.locator('main')).toHaveAttribute('aria-busy','false',{timeout:60000});
 result.before=await request('environment.diagnose');
 const imageIndex=process.argv.indexOf('--image');
 if(result.before.phase==='absent'||imageIndex>=0) {
   const localFile=imageIndex>=0&&process.argv[imageIndex+1]&&!process.argv[imageIndex+1].startsWith('--')?process.argv[imageIndex+1]:undefined;
   const plan=await request(imageIndex>=0?'environment.image.plan':'environment.plan',localFile?{localFile}:{});result.plan=plan;
   await writeFile(join(directory,'reviewed-plan.json'),JSON.stringify(plan,null,2));
   console.log(JSON.stringify({stage:'plan',planDirectory:directory,plan},null,2));
   if(process.argv.includes('--apply')) {result.after=await request(imageIndex>=0?'environment.image.provision':'environment.provision',{planId:plan.id,digest:plan.digest});result.applied=true;}
 } else if(result.before.status!=='ready'&&process.argv.includes('--resume')) {
   result.after=await request('environment.resume');result.applied=true;
 } else result.after=result.before;
 if(result.after) {
   result.catalog=await request('packages.catalog');
   result.passed=result.after.status==='ready';
   await page.getByRole('button',{name:'Kali e ferramentas',exact:true}).click();
   await page.screenshot({path:join(directory,'managed-kali.png'),fullPage:true});
 }
} catch(error) {result.error=String(error);process.exitCode=1;}
finally {
 await browser?.close().catch(()=>{});if(child&&child.exitCode===null)child.kill();
 await writeFile(join(directory,'result.json'),JSON.stringify(result,null,2));
 console.log(JSON.stringify({passed:result.passed,applied:result.applied,error:result.error,resultDirectory:directory},null,2));
}
