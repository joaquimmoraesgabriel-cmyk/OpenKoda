import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomInt, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { basename, dirname, join, resolve } from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { fileURLToPath } from 'node:url';
import { chromium, expect } from '@playwright/test';

// Explicit opt-in. One paid chat; the image, question and project are synthetic.
const [flag, keyFile, binaryPath, binarySha256] = process.argv.slice(2);
assert.equal(flag, '--live-openrouter');
assert.match(binarySha256 ?? '', /^[0-9a-f]{64}$/);
const binary = await realpath(binaryPath);
assert.equal(basename(binary), 'openkoda.exe');
assert.equal(basename(dirname(binary)), 'debug');
assert.equal(createHash('sha256').update(await readFile(binary)).digest('hex'), binarySha256);
const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const root = join(app, '.local', 'live-image-' + randomUUID());
const profile = join(root, 'profile');
const repo = join(root, 'fixture');
const provider = 'openrouter';
const model = 'google/gemini-2.5-flash-lite';
const question = 'Transcreva somente o número de seis dígitos mostrado na imagem anexada.';
const digits = String(randomInt(100000, 1000000));
const keys = (await readFile(keyFile, 'utf8')).match(/sk-or-[\w-]{18,512}/g) ?? [];
assert.equal(keys.length, 1, 'Expected one OpenRouter credential');
const catalogResponse = await fetch('https://openrouter.ai/api/v1/models', {redirect:'error', signal:AbortSignal.timeout(15000)});
assert.equal(catalogResponse.status, 200);
const entry = (await catalogResponse.json()).data.find(item => item.id === model);
assert.ok(entry?.architecture.input_modalities.includes('image') && entry.architecture.output_modalities.includes('text'));
const inputRate = Number(entry.pricing.prompt), outputRate = Number(entry.pricing.completion);
assert.ok(inputRate > 0 && inputRate <= 0.0000002 && outputRate > 0 && outputRate <= 0.0000005);
assert.ok(Number(entry.pricing.request ?? 0) === 0 && Number(entry.pricing.image ?? 0) <= 0.005);
assert.ok(Number(entry.pricing.internal_reasoning ?? 0) <= 0.0000005);
await mkdir(root);
await mkdir(profile);
await mkdir(repo);
await writeFile(join(repo, 'README.md'), 'Synthetic image attachment test.\n', {flag:'wx'});
const receipt = {model, binarySha256, stage:'launch', passed:false, keyRemoved:false};
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
let browser, page, activated = false;
const ipc = (action, payload={}) => page.evaluate(({action,payload}) =>
  window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}), {action,payload});
try {
  const url = 'http://127.0.0.1:' + port;
  await expect.poll(async () => {
    try { return (await fetch(url + '/json/version')).ok; } catch { return false; }
  }, {timeout:60000}).toBe(true);
  browser = await chromium.connectOverCDP(url);
  await expect.poll(() => browser.contexts()[0].pages().length, {timeout:30000}).toBe(1);
  page = browser.contexts()[0].pages()[0];
  await page.waitForFunction(() => location.hostname === 'tauri.localhost' && !!window.__TAURI_INTERNALS__?.invoke);
  const state = await ipc('snapshot');
  assert.deepEqual(state.projects, []);
  assert.equal((await ipc('ai.status', {provider})).hasApiKey, false);
  activated = true;
  await ipc('ai.key.save', {provider, key:keys[0], prepareActivation:true});
  const listing = await ipc('ai.models', {provider});
  assert.ok(listing.models.includes(model));
  await ipc('ai.provider.configure', {provider, credentialRevision:listing.credentialRevision, models:[model], defaultModel:model});
  const budget = await ipc('ai.budget.status');
  await ipc('ai.budget.configure', {enabled:true, currency:'USD', expectedRevision:budget.revision,
    installationLimitUsd:'0.05', assessmentLimitUsd:'0.05', acknowledgeEstimates:true, acknowledgeLegacyExcluded:true,
    legacyReviewDigest:budget.legacyReviewDigest, prices:[{provider,model,inputUsdPerMillion:(inputRate*1e6).toFixed(6),
      outputUsdPerMillion:(outputRate*1e6).toFixed(6),sourceNote:'OpenRouter catalog for live image smoke'}]});
  const {assessmentId} = await ipc('project.launch', {kind:'code', root:repo, name:'Imagem sintética', title:'Imagem sintética',
    objective:'Ler uma imagem sintética enviada como anexo.', ports:[], useRipgrep:false, aiSelection:{provider,model}, idempotencyKey:randomUUID()});
  await page.reload();
  receipt.stage = 'attach';
  const png = await page.evaluate(value => {
    const canvas = document.createElement('canvas');
    canvas.width = 400;
    canvas.height = 120;
    const ctx = canvas.getContext('2d');
    ctx.fillStyle = '#ffffff';
    ctx.fillRect(0,0,400,120);
    ctx.fillStyle = '#111111';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.font = 'bold 70px Consolas, monospace';
    ctx.fillText(value,200,60);
    return canvas.toDataURL('image/png').split(',')[1];
  }, digits);
  await page.getByRole('button',{name:'Adicionar à mensagem'}).click();
  const chooserPromise = page.waitForEvent('filechooser');
  await page.getByRole('menuitem',{name:'Anexar foto ou arquivo'}).click();
  await (await chooserPromise).setFiles({name:'amostra.png', mimeType:'image/png', buffer:Buffer.from(png,'base64')});
  const thumb = page.getByRole('img',{name:'Anexo selecionado'});
  await expect(thumb).toBeVisible();
  const jpeg = await thumb.getAttribute('src');
  assert.match(jpeg, /^data:image\/jpeg;base64,/);
  const input = page.getByRole('textbox',{name:'Mensagem da conversa',exact:true});
  await input.fill(question);
  const send = page.getByRole('button',{name:'Enviar mensagem',exact:true});
  await expect(send).toBeEnabled();
  await send.click();
  const approval = page.getByRole('button',{name:'Autorizar envio',exact:true});
  const alert = page.getByRole('region',{name:'Conversa do projeto'}).getByRole('alert').first();
  await approval.or(alert).first().waitFor({state:'visible',timeout:15000});
  if (await alert.isVisible()) throw new Error(await alert.innerText());
  await expect(page.getByRole('img',{name:'Imagem selecionada para envio'})).toHaveAttribute('src',jpeg);
  assert.equal((await ipc('ai.history',{assessmentId})).requests.length,0);
  receipt.stage = 'send';
  await approval.click();
  await expect.poll(async () => {
    if (await alert.isVisible()) return await alert.innerText();
    const {requests} = await ipc('ai.history',{assessmentId});
    assert.ok(requests.length <= 1);
    return requests[0]?.status;
  }, {timeout:95000}).toBe('completed');
  const {requests,messages} = await ipc('ai.history',{assessmentId});
  assert.equal(requests.length,1);
  const request = requests[0];
  assert.equal(request.model,model);
  assert.ok(request.inputTokens > 0 && request.outputTokens > 0);
  assert.equal(request.dataReceiptStatus,'recorded');
  const answer = messages.find(m => m.role === 'assistant')?.text;
  receipt.answer = answer;
  receipt.expectedDigits = digits;
  receipt.tokens = {input:request.inputTokens, output:request.outputTokens};
  receipt.stage = 'generation-cost';
  const database = new DatabaseSync(join(profile,'assistant.sqlite3'),{readOnly:true});
  let generationId;
  try {
    generationId = database.prepare("SELECT json_extract(details,'$.responseId') AS generation FROM requests WHERE id=? AND assessment=?").get(request.id,assessmentId).generation;
  } finally {database.close();}
  assert.match(generationId,/^gen-[A-Za-z0-9-]+$/);
  const lookup = new URL('https://openrouter.ai/api/v1/generation');
  lookup.searchParams.set('id',generationId);
  let generation;
  await expect.poll(async () => {
    const response = await fetch(lookup,{headers:{Authorization:'Bearer '+keys[0]},redirect:'error',signal:AbortSignal.timeout(15000)});
    if (response.status === 404) return false;
    assert.equal(response.status,200);
    generation = (await response.json()).data;
    return typeof generation?.total_cost === 'number';
  },{timeout:45000,intervals:[500,1000,2000]}).toBe(true);
  assert.equal(generation.id,generationId);
  assert.ok(generation.total_cost >= 0 && generation.total_cost <= 0.05);
  receipt.generation = {id:generation.id,costUsd:generation.total_cost,inputMedia:generation.num_media_prompt,streamed:generation.streamed};
  assert.equal(generation.num_media_prompt,1,'OpenRouter must have received one image');
  assert.equal(generation.streamed,true);
  receipt.stage = 'reopen-history';
  assert.ok(typeof answer === 'string' && answer.match(/\b\d{6}\b/g)?.join(' ') === digits,
    'Response did not uniquely identify the number in the pixels');
  const rows = (await ipc('ai.budget.history')).rows.filter(row => row.assessmentId === assessmentId && row.model === model);
  assert.ok(rows.some(row => row.requests === 1 && row.input === request.inputTokens && row.output === request.outputTokens));
  await page.reload();
  await expect(page.getByText(answer,{exact:true})).toBeVisible();
  const reopened = await ipc('ai.history',{assessmentId});
  assert.deepEqual(reopened.messages,messages);
  assert.deepEqual(reopened.requests,requests);
  receipt.passed = true;
} catch (error) {
  receipt.error = String(error).replaceAll(keys[0],'[redacted]').slice(0,900);
} finally {
  try {
    if (page && activated) {
      await ipc('ai.key.remove',{provider});
      receipt.keyRemoved = !(await ipc('ai.status',{provider})).hasApiKey;
    }
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await exited;
    await browser?.close().catch(() => {});
    await writeFile(join(root,'receipt.json'),JSON.stringify(receipt,null,2));
    console.log(JSON.stringify({root,...receipt},null,2));
  }
}
if (!receipt.passed || !receipt.keyRemoved) process.exitCode = 1;
