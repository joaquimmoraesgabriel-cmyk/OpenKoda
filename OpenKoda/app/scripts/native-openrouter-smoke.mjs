import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { randomUUID, createHash } from 'node:crypto';
import { once } from 'node:events';
import { access, mkdir, readFile, readdir, realpath, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { dirname, basename, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium, expect } from '@playwright/test';

// Explicit opt-in. Two one-file analyses at $0.01 each and one short chat.
// A clean debug profile prevents the installed application or its projects from
// participating. Secrets never enter argv, screenshots, traces, or the receipt.
const [flag, keyFile, binaryPath, expectedDigest, run = 'all', priorId] = process.argv.slice(2);
assert.equal(flag, '--live-openrouter');
assert.ok(['all','google','mistral','chat','chat-google','resume-chat'].includes(run),'Unknown smoke case');
const resume=run==='resume-chat';
if(resume)assert.match(priorId??'',/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
else assert.equal(priorId,undefined);
assert.match(expectedDigest ?? '', /^[0-9a-f]{64}$/);
const binary = await realpath(binaryPath);
assert.equal(basename(binary), 'openkoda.exe');
assert.equal(basename(dirname(binary)), 'debug');
assert.equal(createHash('sha256').update(await readFile(binary)).digest('hex'), expectedDigest);
const app = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const version = JSON.parse(await readFile(join(app, 'package.json'), 'utf8')).version;
const root = join(app, '.local', 'live-router-' + randomUUID());
const profileRoot=resume?join(app,'.local','live-router-'+priorId):root;
const profile = join(profileRoot, 'profile');
await mkdir(root);
if(!resume)await mkdir(profile);
else for(const path of [profileRoot,profile,join(profileRoot,'receipt.json'),join(profile,'openkoda.sqlite3')]){
  assert.equal((await realpath(path)).toLowerCase(),path.toLowerCase(),'Unexpected profile path');
}
const models = ['google/gemini-2.5-flash-lite', 'mistralai/ministral-8b-2512'];
const chatOnly = run==='chat'||run==='chat-google';
const chatModel = run==='chat-google'?models[0]:models[1];
const scanModels = chatOnly ? [] : models.filter((_,i)=>run==='all'||run===['google','mistral'][i]);
const catalog = await (await fetch('https://openrouter.ai/api/v1/models', {redirect: 'error'})).json();
for (const id of models) {
  const model = catalog.data.find(model => model.id === id);
  assert.ok(model && model.architecture.output_modalities.includes('text'));
  for (const [field, max] of [['prompt', 0.0000002], ['completion', 0.0000005]]) {
    assert.ok(Number(model.pricing[field]) >= 0 && Number(model.pricing[field]) <= max, 'Price changed');
  }
  assert.equal(Number(model.pricing.request ?? 0), 0);
  assert.ok(Number(model.pricing.internal_reasoning ?? 0) <= 0.0000005);
}
const fixture = [
  'from flask import Flask, request',
  'app = Flask(__name__)',
  '@app.get("/calculate")',
  'def calculate():',
  '    expression = request.args["expression"]',
  '    return str(eval(expression))',
].join('\n')+'\n';
const goal = 'Revise o fluxo de dados de calculate e localize o risco em calculator.py.';
let prior;
if(resume){
  prior=JSON.parse(await readFile(join(profileRoot,'receipt.json'),'utf8'));
  assert.equal(prior.version,version);assert.match(prior.binarySha256,/^[0-9a-f]{64}$/);
  assert.equal(prior.stage,'chat');assert.equal(prior.chat,null);assert.equal(prior.passed,false);assert.equal(prior.keyRemoved,true);
  assert.deepEqual(prior.scans.map(s=>s.model),models);
  for(const [i,scan] of prior.scans.entries()){
    assert.equal(scan.status,'completed');assert.ok(scan.requests>=1&&scan.requests<=3&&scan.input>0&&scan.output>0);
    assert.ok(Number.isFinite(scan.costUsd)&&scan.costUsd>=0&&scan.costUsd<=0.01);
    assert.ok(scan.findings.some(f=>/^calculator\.py:[56]$/.test(f.location)));
    const repo=join(profileRoot,'synthetic-'+(i+1)), file=join(repo,'calculator.py');
    assert.equal((await realpath(file)).toLowerCase(),file.toLowerCase());
    assert.deepEqual(await readdir(repo),['calculator.py']);assert.equal(await readFile(file,'utf8'),fixture);
  }
}
const keys = (await readFile(keyFile, 'utf8')).match(/sk-or-[\w-]{18,512}/g) || [];
assert.ok(keys.length === 1, 'Cannot identify one authorized OpenRouter key');
async function keyUsage() {
  const response = await fetch('https://openrouter.ai/api/v1/key', {
    headers: { Authorization: 'Bearer ' + keys[0] }, redirect:'error', signal:AbortSignal.timeout(15000)
  });
  assert.equal(response.status, 200, 'OpenRouter key status');
  const usage = (await response.json()).data.usage;
  assert.ok(typeof usage === 'number' && Number.isFinite(usage));
  return usage;
}
const baseline = await keyUsage();
const receipt = { version, binarySha256:expectedDigest, scans:[], chat:null, keyUsageDeltaUsd:null, keyRemoved:false, passed:false };
if(resume)receipt.resumedFrom=priorId;
const server = createServer();
server.listen(0, '127.0.0.1'); await once(server, 'listening');
const port = server.address().port;
await new Promise(done => server.close(done));
const child = spawn(binary, [], {cwd:dirname(binary), windowsHide:true, stdio:'ignore', env:{
  ...process.env, OPENKODA_DATA_DIR:profile, WEBVIEW2_USER_DATA_FOLDER:join(root, 'webview'),
  WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-address=127.0.0.1 --remote-debugging-port=' + port,
}});
const closed = once(child, 'exit');
let browser, page, stage='open-debug',activationAttempted=false;
const ipc = (action, payload={}) => page.evaluate(({action,payload}) =>
  window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}), {action,payload});
const pause = ms => new Promise(done => setTimeout(done,ms));
const persist = () => writeFile(join(root, 'receipt.json'), JSON.stringify({...receipt,stage}, null, 2));
try {
  const url='http://127.0.0.1:'+port;
  await expect.poll(async()=>{try{return (await fetch(url+'/json/version')).ok;}catch{return false;}},{timeout:60000}).toBe(true);
  browser=await chromium.connectOverCDP(url);
  await expect.poll(()=>browser.contexts()[0].pages().length,{timeout:30000}).toBe(1);
  page=browser.contexts()[0].pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForFunction(()=>location.hostname==='tauri.localhost' && !!window.__TAURI_INTERNALS__?.invoke);
  await access(join(profile,'openkoda.sqlite3'));
  const initial=await ipc('snapshot'),reviewed=[];
  let assessmentId;
  if(resume){
    assert.equal(initial.projects.length,2);assert.equal(initial.assessments.length,2);
    assert.equal((await ipc('ai.status',{provider:'openrouter'})).hasApiKey,false);
    for(const [i,model] of models.entries()){
      const repo=join(profileRoot,'synthetic-'+(i+1));
      const project=initial.projects.find(p=>p.target.toLowerCase()===repo.toLowerCase());
      assert.ok(project&&!project.archived);
      const chat=initial.assessments.find(a=>a.projectId===project.id);
      assert.equal(chat.target.toLowerCase(),repo.toLowerCase());assert.equal(chat.aiSelection.model,model);
      const {scan}=await ipc('security.scan.get',{assessmentId:chat.id});
      assert.equal(scan.status,'completed');assert.equal(scan.provider,'openrouter');assert.equal(scan.model,model);
      assert.deepEqual(scan.result.findings.map(f=>({location:f.location,severity:f.severity})),prior.scans[i].findings);
      assert.ok(scan.result.report.trim());assert.equal(scan.result.cost.estimatedUsd,prior.scans[i].costUsd);
      assert.equal((await ipc('ai.history',{assessmentId:chat.id})).requests.length,0,'A chat has already been dispatched');
      reviewed.push({assessmentId:chat.id,model,scanId:scan.id,projectName:project.name,title:chat.title});
    }
    assessmentId=reviewed[1].assessmentId;
  }else assert.equal(initial.projects.length,0);
  stage='provider-activation';
  activationAttempted=true;
  await ipc('ai.key.save',{provider:'openrouter',key:keys[0],prepareActivation:true});
  const listing=await ipc('ai.models',{provider:'openrouter'});
  assert.ok(models.every(id=>listing.models.includes(id)));
  await ipc('ai.provider.configure',{provider:'openrouter',credentialRevision:listing.credentialRevision,models,defaultModel:models[0]});
  await page.reload();
  if(resume){
    const entry=reviewed[1];
    const group=page.locator('.tree-group').filter({has:page.getByRole('button',{name:entry.projectName,exact:true})});
    await group.getByRole('button',{name:entry.title,exact:true}).click();
    await expect(group.getByRole('button',{name:entry.title,exact:true})).toHaveAttribute('aria-current','page');
  }
  for (const [index,model] of scanModels.entries()) {
    stage='scan-'+(index+1);
    assert.ok((await keyUsage())-baseline<0.02,'Stop on unexpected usage');
    const repository=join(root,'synthetic-'+(index+1));
    await mkdir(repository);await writeFile(join(repository,'calculator.py'),fixture,{flag:'wx'});
    const create = page.getByRole('button',{name:'Criar projeto',exact:true});
    if(index===0)await create.click();else await page.getByRole('button',{name:'Novo projeto',exact:true}).click();
    const form=page.locator('.launch-page');
    await form.getByLabel('Pasta do projeto',{exact:true}).fill(repository);
    await form.getByLabel('Descrição',{exact:true}).fill(goal);
    const picker=form.getByRole('button',{name:/^Escolher modelo/});
    await picker.click();
    const dialog=page.getByRole('dialog',{name:'Escolher provedor e modelo'});
    await dialog.getByRole('button',{name:model,exact:true}).click();
    await form.getByRole('switch',{name:'Opções avançadas'}).check();
    await form.getByRole('checkbox',{name:'Personalizar limite de custo'}).check();
    await form.getByLabel('Limite da análise em USD',{exact:true}).fill('0.01');
    await form.getByRole('button',{name:'Criar projeto',exact:true}).click();
    const start=Date.now(); let scan;
    while(Date.now()-start<130000) {
      const state=await ipc('snapshot');
      const project=state.projects.find(p=>p.target.toLowerCase()===repository.toLowerCase());
      assessmentId=state.assessments.find(a=>a.projectId===project?.id)?.id;
      if(assessmentId)scan=(await ipc('security.scan.get',{assessmentId})).scan;
      if(scan && !['starting','running','cancelling'].includes(scan.status))break;
      if(await form.getByRole('alert').isVisible())throw new Error(await form.getByRole('alert').innerText());
      await pause(200);
    }
    const record={model,status:scan?.status??'missing',events:scan?.events.map(e=>e.stage),coverage:scan?.result?.coverage?.summary,
      findings:scan?.result?.findings.map(f=>({location:f.location,severity:f.severity})),costUsd:scan?.result?.cost?.estimatedUsd,error:scan?.error};
    receipt.scans.push(record);await persist();
    assert.equal(record.status,'completed');
    assert.ok(scan.result.findings.some(f=>/^calculator\.py:[56]$/.test(f.location) && /eval\s*\(/i.test(f.description)),
      'Expected the fixture finding with line-level evidence for eval');
    assert.ok(typeof record.costUsd==='number' && record.costUsd>=0 && record.costUsd<=0.01);
    assert.equal(await readFile(join(repository,'calculator.py'),'utf8'),fixture);
    const rows=(await ipc('ai.budget.history')).rows.filter(row=>row.assessmentId===assessmentId && row.model===model);
    record.requests=rows.reduce((sum,row)=>sum+row.requests,0);
    record.input=rows.reduce((sum,row)=>sum+row.input,0);
    record.output=rows.reduce((sum,row)=>sum+row.output,0);
    assert.ok(record.requests>=1 && record.requests<=3);
    assert.ok(rows.some(row=>row.input>0&&row.output>0));
    reviewed.push({assessmentId,model,scanId:scan.id});
  }
  if (chatOnly) {
    const repository=join(root,'synthetic-chat');
    await mkdir(repository);await writeFile(join(repository,'calculator.py'),fixture,{flag:'wx'});
    const created=await ipc('project.launch',{kind:'code',root:repository,name:'Chat sintético',title:'Chat sintético',
      objective:goal,ports:[],useRipgrep:false,aiSelection:{provider:'openrouter',model:chatModel},idempotencyKey:randomUUID()});
    assessmentId=created.assessmentId;await page.reload();
  }
  if (chatOnly || resume || run==='all') {
  stage='chat';
  assert.ok((await keyUsage())-baseline<0.022,'Stop on unexpected usage');
  const input=page.getByRole('textbox',{name:'Mensagem da conversa',exact:true});
  await input.fill('Resuma em uma frase por que eval pode ser perigosa em Python. Inclua o nome da função.');
  const send=page.getByRole('button',{name:'Enviar mensagem',exact:true});
  await expect(send).toBeEnabled({timeout:15000});
  await send.click();
  const start=Date.now(); let sawDelta=false;
  while(Date.now()-start<90000) {
    const error=page.getByRole('region',{name:'Conversa do projeto'}).getByRole('alert').first();
    if(await error.isVisible())throw new Error('Chat UI: '+await error.innerText());
    const approval=page.getByRole('button',{name:'Autorizar envio',exact:true});
    if(await approval.isVisible())await approval.click();
    const live=await ipc('ai.chat.status',{assessmentId});
    if(typeof live.partialText==='string' && live.partialText.length)sawDelta=true;
    const history=await ipc('ai.history',{assessmentId});
    if(history.requests.length){
      assert.equal(history.requests.length,1);
      const r=history.requests[0];
      if(r.status==='completed'){
        assert.equal(r.model,chatModel);
        const rows=(await ipc('ai.budget.history')).rows.filter(row=>row.assessmentId===assessmentId && row.model===r.model);
        assert.ok(rows.some(row=>row.input===r.inputTokens && row.output===r.outputTokens && row.requests===1));
        assert.ok(history.messages.some(m=>m.role==='assistant' && /eval/i.test(m.text)));
        assert.equal(await input.inputValue(),'');
        receipt.chat={model:r.model,status:r.status,input:r.inputTokens,output:r.outputTokens,sawDelta};
        break;
      }
      assert.ok(!['unknown','failed','incomplete'].includes(r.status),'Chat '+r.status);
    }
    await pause(70);
  }
  assert.ok(receipt.chat,'No completed chat before deadline. Composer length='+String((await input.inputValue()).length)+'; submitting button visible='+await send.isVisible());
  }
  stage='reopen-history';
  await page.reload();
  for(const entry of reviewed){
    const {scan}=await ipc('security.scan.get',{assessmentId:entry.assessmentId});
    assert.equal(scan.id,entry.scanId);
    assert.equal(scan.status,'completed');
    assert.ok(scan.result.findings.length>0);
  }
  if(resume)receipt.reopenedScans=reviewed.map(({assessmentId,model,scanId})=>({assessmentId,model,scanId}));
  if(receipt.chat){
    const history=await ipc('ai.history',{assessmentId});
    assert.equal(history.requests.length,1);
    assert.ok(history.messages.some(m=>m.role==='assistant' && /eval/i.test(m.text)));
  }
  receipt.passed=true;
} catch(error) {
  receipt.error=String(error).replaceAll(keys[0],'[redigido]').slice(0,800);
} finally {
  try {
    if(page&&activationAttempted){await ipc('ai.key.remove',{provider:'openrouter'});receipt.keyRemoved=(await ipc('ai.status',{provider:'openrouter'})).hasApiKey===false;}
    // Key totals can lag completed requests. Poll read-only usage; never retry inference.
    receipt.keyUsageBeforeUsd=baseline;
    const start=Date.now();
    const scanned=receipt.scans.reduce((sum,s)=>sum+(s.costUsd??0),0);
    const minimum=scanned-Math.max(1,receipt.scans.length*3)*0.000001;
    do {
      const total=await keyUsage();
      receipt.keyUsageAfterUsd=total;
      receipt.keyUsageDeltaUsd=Number((total-baseline).toFixed(9));
      if(receipt.keyUsageDeltaUsd>0 && receipt.keyUsageDeltaUsd>=minimum || Date.now()-start>=60000)break;
      await pause(3000);
    } while(true);
  } finally {
    if(child.exitCode===null&&child.signalCode===null)child.kill();
    await closed; await browser?.close().catch(()=>{});
    await persist(); console.log(JSON.stringify({root,...receipt},null,2));
  }
}
if(!receipt.passed||!receipt.keyRemoved)process.exitCode=1;
