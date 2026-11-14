import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { basename, dirname, join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { fileURLToPath } from 'node:url';
import { chromium, expect } from '@playwright/test';

// Sends a single bounded chat. Reopening and billing checks never repeat inference.
const [flag, keyFile, binaryPath, binarySha256, selectedModel] = process.argv.slice(2);
assert.equal(flag, '--live-openrouter');
assert.match(binarySha256 ?? '', /^[0-9a-f]{64}$/);
const binary = await realpath(binaryPath);
assert.equal(basename(binary), 'openkoda.exe');
assert.equal(basename(dirname(binary)), 'debug');
assert.equal(createHash('sha256').update(await readFile(binary)).digest('hex'), binarySha256);
const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = join(app, '.local', 'live-cancel-' + randomUUID());
const profile = join(root, 'profile');
const repo = join(root, 'fixture');
const provider = 'openrouter';
const model = selectedModel ?? 'mistralai/ministral-8b-2512';
assert.ok(['mistralai/ministral-8b-2512','openai/gpt-4.1-nano','qwen/qwen3-14b'].includes(model));
const capUsd = '0.01';
const question = 'Para este teste sintético de transmissão, escreva uma lista numerada de 150 pequenos exemplos de nomes de variáveis em Python. ' +
  'Escreva em cada linha o número, o nome e uma breve frase de explicação. Comece diretamente pelo primeiro exemplo e continue até o número 150.';
const catalog = await fetch('https://openrouter.ai/api/v1/models', {redirect:'error', signal:AbortSignal.timeout(15000)});
assert.equal(catalog.status, 200);
const entry = (await catalog.json()).data.find(m => m.id === model);
assert.ok(entry?.architecture.output_modalities.includes('text'));
const inputRate = Number(entry.pricing.prompt), outputRate = Number(entry.pricing.completion);
assert.ok(inputRate > 0 && inputRate <= 0.0000002 && outputRate > 0 && outputRate <= 0.0000005);
assert.equal(Number(entry.pricing.request ?? 0), 0);
assert.ok(Number(entry.pricing.internal_reasoning ?? 0) <= 0.0000005);
const keys = (await readFile(keyFile, 'utf8')).match(/sk-or-[\w-]{18,512}/g) ?? [];
assert.equal(keys.length, 1, 'Expected one OpenRouter credential');
async function keyUsage() {
  const response=await fetch('https://openrouter.ai/api/v1/key',{headers:{Authorization:'Bearer '+keys[0]},redirect:'error',signal:AbortSignal.timeout(15000)});
  assert.equal(response.status,200);
  const used=(await response.json()).data.usage;
  assert.ok(typeof used==='number'&&Number.isFinite(used));
  return used;
}
await mkdir(root);
await mkdir(profile);
await mkdir(repo);
const fixture = 'Synthetic cancellation fixture.\n';
await writeFile(join(repo, 'README.md'), fixture, {flag:'wx'});
const receipt = {model, binarySha256, capUsd, stage:'launch', passed:false, keyRemoved:false, keyUsageBeforeUsd:await keyUsage()};
const portProbe = createServer();
portProbe.listen(0, '127.0.0.1');
await once(portProbe, 'listening');
const {port} = portProbe.address();
await new Promise(done => portProbe.close(done));
const child = spawn(binary, [], {cwd:dirname(binary), windowsHide:true, stdio:'ignore', env:{
  ...process.env, OPENKODA_DATA_DIR:profile, WEBVIEW2_USER_DATA_FOLDER:join(root,'webview'),
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-address=127.0.0.1 --remote-debugging-port=' + port,
}});
const exited = once(child, 'exit');
let browser, page, activated = false, assessmentId;
const ipc = (action, payload={}) => page.evaluate(({action,payload}) =>
  window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}), {action,payload});

async function readGeneration() {
  const database = new DatabaseSync(join(profile,'assistant.sqlite3'),{readOnly:true});
  let id;
  try {
    const rows = database.prepare("SELECT json_extract(details,'$.responseId') AS generation FROM requests WHERE assessment=?").all(assessmentId);
    if (!rows.length) return;
    assert.equal(rows.length,1);
    id = rows[0].generation;
  } finally { database.close(); }
  assert.match(id,/^gen-[a-zA-Z0-9-]{1,124}$/);
  const url = new URL('https://openrouter.ai/api/v1/generation');
  url.searchParams.set('id',id);
  receipt.generation = {id};
  let data;
  await expect.poll(async () => {
    const res = await fetch(url,{redirect:'error',headers:{Authorization:'Bearer '+keys[0]},signal:AbortSignal.timeout(15000)});
    if (res.status === 404) return false;
    assert.equal(res.status,200);
    data = (await res.json()).data;
    return typeof data?.total_cost === 'number';
  },{timeout:65000,intervals:[500,1000,2500]}).toBe(true);
  assert.equal(data.id,id);
  assert.ok([entry.id,entry.canonical_slug].includes(data.model),'Unexpected generation model: '+data.model);
  assert.equal(data.streamed,true);
  receipt.generation = {id, model:data.model, costUsd:data.total_cost, cancelled:data.cancelled, streamed:data.streamed,
    finishReason:data.finish_reason, nativeFinishReason:data.native_finish_reason, inputTokens:data.native_tokens_prompt,
    outputTokens:data.native_tokens_completion};
  assert.ok(data.total_cost >= 0 && data.total_cost <= Number(capUsd));
}

try {
  const url = 'http://127.0.0.1:' + port;
  await expect.poll(async () => {
    try { return (await fetch(url+'/json/version')).ok; } catch { return false; }
  },{timeout:60000}).toBe(true);
  browser = await chromium.connectOverCDP(url);
  await expect.poll(() => browser.contexts()[0].pages().length,{timeout:30000}).toBe(1);
  page = browser.contexts()[0].pages()[0];
  await page.waitForFunction(() => location.hostname === 'tauri.localhost' && !!window.__TAURI_INTERNALS__?.invoke);
  assert.equal((await ipc('snapshot')).projects.length,0);
  assert.equal((await ipc('ai.status',{provider})).hasApiKey,false);
  activated = true;
  await ipc('ai.key.save',{provider,key:keys[0],prepareActivation:true});
  const listing = await ipc('ai.models',{provider});
  assert.ok(listing.models.includes(model));
  await ipc('ai.provider.configure',{provider,credentialRevision:listing.credentialRevision,models:[model],defaultModel:model});
  const budget = await ipc('ai.budget.status');
  await ipc('ai.budget.configure',{enabled:true,currency:'USD',expectedRevision:budget.revision,
    installationLimitUsd:capUsd,assessmentLimitUsd:capUsd,acknowledgeEstimates:true,acknowledgeLegacyExcluded:true,
    legacyReviewDigest:budget.legacyReviewDigest,prices:[{provider,model,inputUsdPerMillion:(inputRate*1e6).toFixed(6),
      outputUsdPerMillion:(outputRate*1e6).toFixed(6),sourceNote:'OpenRouter catalog for cancellation test'}]});
  ({assessmentId} = await ipc('project.launch',{kind:'code',root:repo,name:'Cancelamento sintético',title:'Cancelamento sintético',
    objective:'Verificar o cancelamento de uma conversa em fluxo.',ports:[],useRipgrep:false,aiSelection:{provider,model},idempotencyKey:randomUUID()}));
  await page.reload();
  const input = page.getByRole('textbox',{name:'Mensagem da conversa',exact:true});
  await input.fill(question);
  await page.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  const approval = page.getByRole('button',{name:'Autorizar envio',exact:true});
  const region = page.getByRole('region',{name:'Conversa do projeto'});
  const alert = region.getByRole('alert').first();
  await approval.or(alert).first().waitFor({state:'visible',timeout:15000});
  if (await alert.isVisible()) throw new Error(await alert.innerText());
  assert.equal((await ipc('ai.history',{assessmentId})).requests.length,0);
  receipt.stage = 'send';
  await approval.click();
  const partial = region.getByRole('article',{name:'Resposta em andamento'});
  await partial.or(alert).first().waitFor({state:'visible',timeout:90000});
  if (await alert.isVisible()) throw new Error(await alert.innerText());
  const partialText = await partial.innerText();
  receipt.visiblePartialCharacters = partialText.replace(/^OpenKoda\s*/,'').trim().length;
  assert.ok(receipt.visiblePartialCharacters > 0);
  receipt.requestId = await partial.getAttribute('data-live-answer');
  assert.ok(receipt.requestId);
  const started = Date.now();
  receipt.stage = 'cancel';
  await page.getByRole('button',{name:'Cancelar trabalho',exact:true}).click();
  await expect.poll(async () => (await ipc('ai.history',{assessmentId})).requests[0]?.status,{timeout:10000}).toBe('unknown');
  receipt.interruptionMs = Date.now()-started;
  assert.equal((await ipc('ai.chat.status',{assessmentId})).requestId,null);
  await expect(partial).toHaveCount(0);
  const history = await ipc('ai.history',{assessmentId});
  assert.equal(history.requests.length,1);
  const request = history.requests[0];
  assert.equal(request.id,receipt.requestId);
  assert.equal(request.inputTokens,null);
  assert.equal(request.outputTokens,null);
  assert.equal(request.dataReceiptStatus,'recorded');
  assert.equal(history.messages.length,1);
  assert.equal(history.messages[0].role,'user');
  assert.equal(history.messages[0].text,question);
  assert.deepEqual(request.chatMessageIds,[history.messages[0].id]);
  const charge = (await ipc('ai.budget.status',{assessmentId})).recentCharges.find(row => row.chargeId === request.id);
  assert.equal(charge?.state,'unknown');
  assert.equal(charge.pricedUsageUsd,null);
  receipt.local = {status:request.status,budgetState:charge.state,reservedUsd:charge.reservedUsd,requests:1,userTurns:1,assistantTurns:0};
  const rows = (await ipc('ai.budget.history')).rows.filter(row => row.assessmentId === assessmentId);
  assert.equal(rows.length,1);
  assert.equal(rows[0].requests,1);
  assert.equal(rows[0].partial,1);
  assert.equal(rows[0].input,null);
  assert.equal(rows[0].output,null);
  receipt.stage = 'reopen';
  await page.reload();
  await expect(page.getByText(question,{exact:true})).toHaveCount(1);
  const reopened = await ipc('ai.history',{assessmentId});
  assert.deepEqual(reopened,history);
  assert.equal(await readFile(join(repo,'README.md'),'utf8'),fixture);
  receipt.reopenedWithoutDuplicate = true;
  receipt.passed = true;
} catch (error) {
  receipt.error = String(error).replaceAll(keys[0],'[redacted]').slice(0,900);
} finally {
  try {
    if (page && activated) {
      await expect.poll(async () => {
        try { await ipc('ai.key.remove',{provider}); return !(await ipc('ai.status',{provider})).hasApiKey; }
        catch { return false; }
      },{timeout:100000,intervals:[200,1000]}).toBe(true);
      receipt.keyRemoved = true;
    }
    if (assessmentId) {
      try {
        await readGeneration();
        if(receipt.generation){
          await expect.poll(async()=>{
            receipt.keyUsageAfterUsd=await keyUsage();
            receipt.keyUsageDeltaUsd=Number((receipt.keyUsageAfterUsd-receipt.keyUsageBeforeUsd).toFixed(9));
            return receipt.keyUsageDeltaUsd+0.000000001>=receipt.generation.costUsd;
          },{timeout:65000,intervals:[1000,2000,3000]}).toBe(true);
        }
      }
      catch (error) { receipt.generationError = String(error).replaceAll(keys[0],'[redacted]').slice(0,900); }
    }
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await exited;
    await browser?.close().catch(() => {});
    await writeFile(join(root,'receipt.json'),JSON.stringify(receipt,null,2));
    console.log(JSON.stringify({root,...receipt},null,2));
  }
}
if (!receipt.passed || !receipt.keyRemoved || typeof receipt.generation?.costUsd !== 'number') process.exitCode = 1;
