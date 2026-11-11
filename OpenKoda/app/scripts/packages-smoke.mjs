// Real managed-package flow. Plan first; --apply-plan consumes the exact saved review.
import {chromium,expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import {createServer} from 'node:net';
import {mkdir,readFile,writeFile} from 'node:fs/promises';
import {dirname,resolve,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID} from 'node:crypto';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const directory=join(root,'.local',`packages-smoke-${randomUUID()}`);await mkdir(directory,{recursive:true});
const net=createServer();await new Promise((ok,fail)=>{net.once('error',fail);net.listen(0,'127.0.0.1',ok);});const port=net.address().port;await new Promise(ok=>net.close(ok));
const result={passed:false,directory};let browser,child;
try {
 const env={...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`};delete env.OPENKODA_DATA_DIR;
 child=spawn(join(root,'src-tauri/target/debug/openkoda.exe'),[],{cwd:root,windowsHide:true,stdio:'ignore',env});child.on('error',()=>{});
 await expect.poll(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:45000}).toBe(true);
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);const context=browser.contexts()[0];await expect.poll(()=>context.pages().length,{timeout:15000}).toBeGreaterThan(0);const page=context.pages()[0];
 await page.waitForFunction(()=>typeof window.__TAURI_INTERNALS__?.invoke==='function');await expect(page.locator('main')).toHaveAttribute('aria-busy','false',{timeout:60000});
 const request=(action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}),{action,payload});
 result.before=await request('environment.diagnose');
 const repairIndex=process.argv.indexOf('--repair-apply');
 if(process.argv.includes('--repair-plan')) {
  expect(result.before.status).toBe('degraded');
  result.plan=await request('environment.repair.plan');expect(result.plan.kind).toBe('environment-repair');
  result.passed=true;
 } else if(repairIndex>=0) {
  const receipt=JSON.parse(await readFile(resolve(process.argv[repairIndex+1]),'utf8'));
  if(!receipt.plan?.id||!receipt.plan?.digest||receipt.plan.kind!=='environment-repair')throw new Error('A recovery review receipt is required');
  result.plan=receipt.plan;result.applied=await request('environment.repair.apply',{planId:receipt.plan.id,digest:receipt.plan.digest});
  expect(result.applied.status).toBe('ready');expect(result.applied.repair.status).toBe('completed');
  result.after=await request('environment.diagnose');expect(result.after.status).toBe('ready');
  result.passed=true;
 } else {
 expect(result.before.status).toBe('ready');
 const applyIndex=process.argv.indexOf('--apply-plan');
 if(applyIndex>=0) {
  const receipt=JSON.parse(await readFile(resolve(process.argv[applyIndex+1]),'utf8'));
  if(!receipt.plan?.id||!receipt.plan?.digest||receipt.plan.kind!=='packages')throw new Error('A package review receipt is required');
  result.plan=receipt.plan;result.applied=await request('packages.apply',{planId:receipt.plan.id,digest:receipt.plan.digest});
 }else{
  if(process.argv.includes('--refresh')){result.refresh=await request('packages.refresh');console.log(JSON.stringify({stage:'catalog-refreshed'}));}
  result.plan=await request('packages.plan',{packages:['ripgrep']});
 }
 result.after=await request('environment.diagnose');expect(result.after.status).toBe('ready');
 result.catalog=await request('packages.catalog');expect(result.catalog.inventoryVerified).toBe(true);
 if(applyIndex>=0)expect(result.catalog.tools.find(tool=>tool.package==='ripgrep').installed).toBe(true);
 result.passed=true;
 }
}catch(error){result.error=String(error);process.exitCode=1;}
finally{await browser?.close().catch(()=>{});if(child&&child.exitCode===null)child.kill();await writeFile(join(directory,'result.json'),JSON.stringify(result,null,2));console.log(JSON.stringify({passed:result.passed,plan:result.plan,error:result.error,directory},null,2));}
