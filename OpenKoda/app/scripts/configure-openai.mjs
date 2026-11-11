// Explicit local credential import through real Tauri IPC. Never records key or provider text.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { readFile, mkdir, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID } from 'node:crypto';

const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const keyFile=process.argv[2];
const verifyOnly=keyFile==='--verify';
if(!keyFile) throw new Error('Usage: node scripts/configure-openai.mjs <explicit-key-file> [model]');
const binary=resolve(root,'src-tauri/target/debug/openkoda.exe');
const directory=join(root,'.local',`openai-connection-${randomUUID()}`);
await mkdir(directory,{recursive:true});
const server=createServer();
await new Promise((ok,fail)=>{server.once('error',fail);server.listen(0,'127.0.0.1',ok);});
const port=server.address().port;
await new Promise(ok=>server.close(ok));
let browser, child;
let key;
const metadata={passed:false,configured:false,model:null,endpoint:'https://api.openai.com/v1',dataDirectory:join(process.env.USERPROFILE,'.openkoda')};
try {
  if(!verifyOnly) {
    const bytes=await readFile(resolve(keyFile));
    const matches=[...new Set(bytes.toString('utf8').match(/\bsk-[A-Za-z0-9_-]{16,}/g)||[])];
    bytes.fill(0);
    if(matches.length!==1) throw new Error('O arquivo deve conter exatamente uma chave OpenAI reconhecível. Nenhum conteúdo foi exibido.');
    key=matches[0];matches.fill('');
  }
  // This invocation deliberately configures the real application profile. Debugging is process-local.
  const env={...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`};
  delete env.OPENKODA_DATA_DIR;
  child=spawn(binary,[],{cwd:root,windowsHide:true,stdio:'ignore',env});
  child.on('error',()=>{});
  await expect.poll(async()=>{try{return (await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:30000}).toBe(true);
  browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context=browser.contexts()[0];
  await expect.poll(()=>context.pages().length,{timeout:15000}).toBeGreaterThan(0);
  const page=context.pages()[0];
  await page.waitForFunction(()=>typeof window.__TAURI_INTERNALS__?.invoke==='function');
  const request=(action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}),{action,payload});
  await expect(page.locator('main')).toHaveAttribute('aria-busy','false');
  if(!verifyOnly) await request('ai.key.save',{key});key=undefined;
  metadata.configured=true;
  const snapshot=await request('snapshot');
  let model=snapshot.settings.model;
  let result;
  if(verifyOnly) {
    result=await request('ai.status');
    expect(result.hasApiKey).toBe(true);
    metadata.verificationOnly=true;
  } else {
  if(snapshot.settings.endpoint!=='https://api.openai.com/v1'||snapshot.settings.provider!=='openai') {
    await request('settings.save',{provider:'openai',endpoint:'https://api.openai.com/v1'});
  }
  const catalog=await request('ai.models');
  const requested=process.argv[3]||snapshot.settings.model;
  model=requested||['gpt-5-mini','gpt-4.1-mini','gpt-4o-mini'].find(id=>catalog.models.includes(id));
  if(!model||!catalog.models.includes(model)) throw new Error('Selecione um modelo disponível nas Configurações; a chave já foi cadastrada.');
  await request('settings.save',{model});metadata.model=model;
  result=await request('ai.test');
  }
  metadata.model=model;
  expect(result.verified).toBe(true);
  metadata.passed=true;metadata.checkedAt=result.checkedAt;metadata.usage=result.usage;
  await page.reload();
  await expect(page.locator('main')).toHaveAttribute('aria-busy','false');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await expect(page.getByLabel('Modelo',{exact:true})).toHaveValue(model);
  await expect(page.getByText(/Conexão verificada/)).toBeVisible();
  await page.screenshot({path:join(directory,'connection-configured.png'),fullPage:true});
} catch(error) {
  // Deliberately generic: no exception text from a provider, page or file is retained.
  const reason=String(error);
  metadata.error=reason.includes('Limite ou saldo')?'A API informou limite ou saldo insuficiente. A credencial cadastrada foi preservada.':reason.includes('recusou a credencial')?'A OpenAI recusou a credencial cadastrada.':reason.includes('Falha de conexão')?'Não foi possível concluir a conexão HTTPS com a OpenAI.':reason.includes('incompleta')?'A resposta do teste ficou incompleta; o consumo informado foi preservado no registro local.':'O cadastro ou teste não concluiu. Consulte a tela Configurações; nenhuma chave foi registrada no diagnóstico.';
  process.exitCode=1;
} finally {
  key=undefined;
  await browser?.close().catch(()=>{});
  if(child&&child.exitCode===null) child.kill();
  await writeFile(join(directory,'result.json'),JSON.stringify(metadata,null,2));
  console.log(JSON.stringify({...metadata,resultDirectory:directory},null,2));
}
