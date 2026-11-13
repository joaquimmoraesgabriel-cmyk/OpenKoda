import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { createHash, randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { access, mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, expect } from '@playwright/test';

const [flag, keyFile, exe, expectedHash] = process.argv.slice(2);
assert.equal(flag, '--live-openrouter-multibatch');
assert.match(expectedHash ?? '', /^[a-f0-9]{64}$/);
const binary = await realpath(exe);
assert.equal(basename(binary), 'openkoda.exe');
assert.equal(basename(dirname(binary)), 'debug', 'Only debug builds honor the disposable profile');
const sha = content => createHash('sha256').update(content).digest('hex');
assert.equal(sha(await readFile(binary)), expectedHash);
const model = 'google/gemini-2.5-flash-lite';
const capUsd = 0.02;
const app = fileURLToPath(new URL('../', import.meta.url));
const root = join(app, '.local', 'live-multibatch-' + randomUUID());
const profile = join(root, 'profile');
const repository = join(root, 'synthetic');
await mkdir(root);
await mkdir(profile);
await mkdir(repository);

// The two files sort into distinct 20 KiB batches. Each on its own fits in a
// single batch. The padding conveys no threat evidence and compresses in tokens.
const padding = Array.from({length: 96}, () => '#' + '-'.repeat(120) + '\n').join('');
const files = {
  'api.py': [
    'from flask import Flask, request',
    'from math_worker import compute_formula',
    'app = Flask(__name__)',
    '@app.get("/compute")',
    'def run_calculation():',
    '    expression = request.args["formula"]',
    '    return str(compute_formula(expression))',
    '',
  ].join('\n') + padding,
  'math_worker.py': [
    'def compute_formula(user_expression):',
    '    return eval(user_expression)',
    '',
  ].join('\n') + padding,
};
for (const [path, content] of Object.entries(files)) {
  assert.ok(Buffer.byteLength(content) < 20 * 1024);
  await writeFile(join(repository, path), content, {flag:'wx'});
}
assert.ok(Object.values(files).reduce((sum, content) => sum + Buffer.byteLength(content), 0) > 20 * 1024);
const goal = 'Verifique se a entrada HTTP da rota /compute chega a uma operação perigosa em outro arquivo. ' +
  'Se um lote mostrar só o chamador ou o chamado, consulte a cópia autorizada do outro arquivo antes de concluir. ' +
  'Para cada candidato, identifique a origem HTTP, a operação final, os nomes dos dois arquivos, e cite a linha literal que sustenta a conclusão.';

const catalog = await fetch('https://openrouter.ai/api/v1/models', {redirect:'error', signal:AbortSignal.timeout(15000)});
assert.equal(catalog.status, 200);
const entry = (await catalog.json()).data.find(row => row.id === model);
assert.ok(entry?.architecture.output_modalities.includes('text'));
for (const [name, max] of [['prompt',2e-7], ['completion',5e-7], ['internal_reasoning',5e-7], ['request',0]]) {
  const price = Number(entry.pricing[name] ?? (['prompt','completion'].includes(name) ? NaN : 0));
  assert.ok(Number.isFinite(price) && price >= 0 && price <= max, 'Unexpected OpenRouter rate: ' + name);
}
const candidates = (await readFile(keyFile, 'utf8')).match(/sk-or-[\w-]{18,512}/g) ?? [];
assert.equal(candidates.length, 1, 'Cannot identify one authorized OpenRouter key');
const [key] = candidates;
const server = createServer();
server.listen(0, '127.0.0.1');
await once(server, 'listening');
const port = server.address().port;
await new Promise(done => server.close(done));
const child = spawn(binary, [], {cwd:dirname(binary), stdio:'ignore', windowsHide:true, env:{
  ...process.env, OPENKODA_DATA_DIR:profile, WEBVIEW2_USER_DATA_FOLDER:join(root,'webview'),
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-address=127.0.0.1 --remote-debugging-port=' + port,
}});
const closed = once(child, 'exit');
const receipt = {model, binarySha256:expectedHash, maxCostUsd:capUsd, passed:false, keyRemoved:false};
let browser, page, activationAttempted = false;
const ipc = (action, payload = {}) => page.evaluate(({action,payload}) =>
  window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}), {action,payload});
try {
  const url = 'http://127.0.0.1:' + port;
  await expect.poll(async () => {
    try { return (await fetch(url + '/json/version')).ok; } catch { return false; }
  }, {timeout:60000}).toBe(true);
  browser = await chromium.connectOverCDP(url);
  await expect.poll(() => browser.contexts()[0].pages().length).toBe(1);
  page = browser.contexts()[0].pages()[0];
  await page.waitForFunction(() => location.hostname === 'tauri.localhost' && !!window.__TAURI_INTERNALS__?.invoke);
  await access(join(profile, 'openkoda.sqlite3'));
  assert.equal((await ipc('snapshot')).projects.length, 0);
  activationAttempted = true;
  await ipc('ai.key.save',{provider:'openrouter',key,prepareActivation:true});
  const listing = await ipc('ai.models',{provider:'openrouter'});
  assert.ok(listing.models.includes(model));
  await ipc('ai.provider.configure',{provider:'openrouter',credentialRevision:listing.credentialRevision,models:[model],defaultModel:model});
  await page.reload();
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  const form = page.locator('.launch-page');
  await form.getByLabel('Pasta do projeto',{exact:true}).fill(repository);
  await form.getByLabel('Descrição',{exact:true}).fill(goal);
  await form.getByRole('button',{name:/^Escolher modelo/}).click();
  await page.getByRole('dialog',{name:'Escolher provedor e modelo'}).getByRole('button',{name:model,exact:true}).click();
  await form.getByRole('switch',{name:'Opções avançadas'}).check();
  await form.getByRole('checkbox',{name:'Personalizar limite de custo'}).check();
  await form.getByLabel('Limite da análise em USD',{exact:true}).fill(String(capUsd));
  await form.getByRole('button',{name:'Criar projeto',exact:true}).click();
  let scan;
  await expect.poll(async () => {
    if (await form.getByRole('alert').isVisible()) throw new Error(await form.getByRole('alert').innerText());
    const state = await ipc('snapshot');
    if (!state.assessments.length) return false;
    assert.equal(state.assessments.length, 1);
    receipt.assessmentId = state.assessments[0].id;
    scan = (await ipc('security.scan.get',{assessmentId:receipt.assessmentId})).scan;
    return !!scan && !['starting','running','cancelling'].includes(scan.status);
  }, {timeout:420000,intervals:[250,750]}).toBe(true);
  receipt.scanId = scan.id;
  receipt.status = scan.status;
  receipt.error = scan.error;
  receipt.events = scan.events;
  receipt.coverage = scan.result?.coverage;
  receipt.findings = scan.result?.findings;
  const ledger = await ipc('ai.budget.status',{assessmentId:receipt.assessmentId});
  const charge = ledger.recentCharges.find(row => row.chargeId === scan.id && row.model === model);
  receipt.ledgerUsd = charge?.assumedChargeUsd == null ? null : Number(charge.assumedChargeUsd);
  receipt.reportedScanUsd = scan.result?.cost?.estimatedUsd ?? receipt.ledgerUsd;
  const history = (await ipc('ai.budget.history')).rows.filter(row => row.assessmentId === receipt.assessmentId && row.model === model);
  receipt.requests = history.reduce((sum,r) => sum + r.requests, 0);
  receipt.inputTokens = history.reduce((sum,r) => sum + r.input, 0);
  receipt.outputTokens = history.reduce((sum,r) => sum + r.output, 0);
  assert.equal(scan.model, model);
  assert.equal(scan.status, 'completed', scan.error ?? 'Expected completed scan');
  assert.match(receipt.coverage.summary, /2 de 2 lotes revisados/);
  assert.deepEqual(receipt.coverage.excludedPaths, []);
  for (const [name, content] of Object.entries(files)) {
    assert.ok(receipt.coverage.reviewedRanges.includes(name + ':1–' + content.trimEnd().split('\n').length), name);
    assert.equal(await readFile(join(repository,name),'utf8'), content);
  }
  const queries = scan.events.filter(event => event.stage === 'source_query');
  const calls = scan.events.filter(event => event.stage === 'review');
  assert.ok(queries.some(event => /[1-9]\d* trecho\(s\) retornado/.test(event.message)), 'Missing observed context lookup');
  assert.equal(calls.length, receipt.requests);
  assert.ok(receipt.requests >= 3 && receipt.requests <= 6);
  assert.ok(receipt.findings.some(finding => {
    const evidence = (finding.title + ' ' + finding.description).toLowerCase();
    const citesSource = ['api.py:6','api.py:7'].includes(finding.location);
    const citesSink = finding.location === 'math_worker.py:2';
    return (citesSource || citesSink) && /eval\b/.test(evidence) &&
      /api\.py|\/compute|request\.args/.test(evidence) && /math_worker\.py|compute_formula/.test(evidence);
  }), 'Missing a line-anchored finding connecting the HTTP source to the evaluator');
  assert.ok(receipt.inputTokens > 0 && receipt.outputTokens > 0);
  assert.ok(Number.isFinite(receipt.reportedScanUsd) && receipt.reportedScanUsd > 0 && receipt.reportedScanUsd <= capUsd);
  receipt.passed = true;
} catch (error) {
  receipt.failure = String(error).replaceAll(key,'[redigido]').slice(0,1000);
} finally {
  try {
    if (activationAttempted) {
      await ipc('ai.key.remove',{provider:'openrouter'});
      receipt.keyRemoved = (await ipc('ai.status',{provider:'openrouter'})).hasApiKey === false;
    }
  } finally {
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await closed;
    await browser?.close().catch(() => {});
    await writeFile(join(root,'receipt.json'), JSON.stringify(receipt,null,2).replaceAll(key,'[redigido]'),{flag:'wx'});
    console.log(JSON.stringify({root,passed:receipt.passed,keyRemoved:receipt.keyRemoved,model,
      requests:receipt.requests,input:receipt.inputTokens,output:receipt.outputTokens,
      reportedScanUsd:receipt.reportedScanUsd,failure:receipt.failure}));
  }
}
if (!receipt.passed || !receipt.keyRemoved) process.exitCode = 1;
