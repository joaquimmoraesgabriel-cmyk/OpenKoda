import assert from 'node:assert/strict';
import { execFile,spawn } from 'node:child_process';
import { createHash,randomUUID } from 'node:crypto';
import { once } from 'node:events';
import { access,mkdir,readFile,realpath,writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { homedir } from 'node:os';
import { basename,dirname,join,resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { chromium,expect } from '@playwright/test';

const appRoot=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const [flag,expectedHash,...extra]=process.argv.slice(2);
assert.equal(flag,'--expected-sha256');
assert.match(expectedHash??'',/^[a-f0-9]{64}$/);
assert.equal(extra.length,0);
const binary=await realpath(resolve(appRoot,process.env.OPENKODA_SMOKE_BINARY||'src-tauri/target/debug/openkoda.exe'));
assert.equal(basename(binary),'openkoda.exe');
assert.equal(basename(dirname(binary)),'debug');
const digest=bytes=>createHash('sha256').update(bytes).digest('hex');
const sha256=digest(await readFile(binary));
assert.equal(sha256,expectedHash);
const version=JSON.parse(await readFile(join(appRoot,'package.json'),'utf8')).version;
const {stdout}=await promisify(execFile)('powershell.exe',['-NoLogo','-NoProfile','-NonInteractive','-Command',
  '(Get-Item -LiteralPath $env:OPENKODA_SMOKE_BINARY).VersionInfo.ProductVersion'],{env:{...process.env,OPENKODA_SMOKE_BINARY:binary},windowsHide:true});
assert.equal(stdout.trim().split('+')[0],version);

const id=randomUUID(),folder=join(appRoot,'.local','native-source-edition-'+id);
const data=join(await realpath(homedir()),'.openkoda-source-smoke-'+id);
const repository=join(folder,'source-fixture'),source=join(repository,'sample.ts');
await mkdir(folder);
await mkdir(data);
await mkdir(repository);
await writeFile(source,'export const example = 1;\n',{flag:'wx'});
const sourceSha256=digest(await readFile(source));
const result={passed:false,id,version,sha256,profile:data,sourceSha256,checks:[],denials:[],screenshots:[],closed:[]};
let active;

async function start(){
  const reserve=createServer();reserve.listen(0,'127.0.0.1');await once(reserve,'listening');
  const port=reserve.address().port;await new Promise(done=>reserve.close(done));
  const child=spawn(binary,[],{cwd:dirname(binary),stdio:'ignore',windowsHide:true,env:{...process.env,OPENKODA_DATA_DIR:data,
    WEBVIEW2_USER_DATA_FOLDER:join(folder,'webview'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:'--remote-debugging-address=127.0.0.1 --remote-debugging-port='+port}});
  active={child,closed:once(child,'exit'),browser:null};
  const url='http://127.0.0.1:'+port;
  await expect.poll(async()=>{try{return (await fetch(url+'/json/version')).ok;}catch{return false;}},{timeout:45000}).toBe(true);
  active.browser=await chromium.connectOverCDP(url);
  const ctx=active.browser.contexts()[0];
  await expect.poll(()=>ctx.pages().length,{timeout:30000}).toBe(1);
  const page=ctx.pages()[0];page.setDefaultTimeout(15000);
  await page.waitForFunction(()=>location.hostname==='tauri.localhost'&&typeof window.__TAURI_INTERNALS__?.invoke==='function');
  return page;
}
async function close(){
  if(!active)return;
  const {child,closed,browser}=active;
  if(child.exitCode===null&&child.signalCode===null)child.kill();
  await closed;
  await browser?.close().catch(()=>{});
  result.closed.push(child.pid);active=undefined;
}
const invoke=(page,action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}),{action,payload});
async function deny(page,action,payload,text){
  await assert.rejects(()=>invoke(page,action,payload),error=>String(error).includes(text),action);
  result.denials.push(action);
}
try{
  let page=await start();
  await access(join(data,'openkoda.sqlite3'));
  const before=await invoke(page,'snapshot');assert.equal(before.projects.length,0);
  await expect(page.getByRole('heading',{name:'Seu primeiro projeto'})).toBeVisible();
  await expect(page.getByRole('radio',{name:/Pasta local/})).toBeChecked();
  await expect(page.getByRole('radio',{name:/Web|Rede/})).toHaveCount(0);
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await page.getByLabel('Pasta do projeto',{exact:true}).fill(repository);
  await expect(page.getByRole('button',{name:'Criar projeto',exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:/Análise Web|Análise de rede|Usar só a análise nativa/})).toHaveCount(0);
  await page.screenshot({path:join(folder,'01-projeto.png')});result.screenshots.push('01-projeto.png');
  result.checks.push('Criação de projeto local exige modelo e omite operações aposentadas.');

  // Confirm the native edition guard before attempting any of the retired routes.
  await deny(page,'project.create',{kind:'probe-edition'},'Esta edição aceita projetos');
  for(const kind of ['web','network'])await deny(page,'project.launch',{kind},'Esta edição aceita projetos');
  const retired='motor anterior está indisponível';
  await deny(page,'project.launch',{kind:'code',useRipgrep:true},retired);
  for(const action of ['assessment.launchPlan','assessment.networkPlan','assessment.sourcePlan','plan.run','work.operation.enqueue','mcp.request','packages.apply','kali.mcp.scanEvidence','ai.operation.propose','evidence.sourceCapture'])await deny(page,action,{},retired);
  assert.deepEqual(await invoke(page,'snapshot'),before);
  result.checks.push('Guarda IPC recusa modalidades, motores e aliases sem alterar o domínio.');

  const body={kind:'code',root:repository,relativeTarget:null,projectId:null,name:'Código sintético',title:'Conversa sintética',objective:'Conferir um projeto local sem chamada paga.',
    minutes:null,ports:[],useRipgrep:false,aiSelection:{provider:'openai',model:'fixture-unpaid'},idempotencyKey:randomUUID()};
  const created=await invoke(page,'project.launch',body);
  const replay=await invoke(page,'project.launch',body);assert.equal(replay.assessmentId,created.assessmentId);assert.equal(replay.replayed,true);
  const status=await invoke(page,'security.status',{assessmentId:created.assessmentId});
  result.runtime={...status.runtime,provider:status.provider,hasApiKey:status.hasApiKey,reason:status.reason};
  assert.equal(status.runtime?.available,true,'Diagnóstico local: '+JSON.stringify(status.runtime?.missing));
  assert.equal(status.available,false);assert.equal(status.hasApiKey,false);
  assert.deepEqual(await invoke(page,'security.scan.get',{assessmentId:created.assessmentId}),{scan:null});
  result.checks.push('SDK disponível no host, perfil sintético sem credencial e nenhuma análise iniciada.');
  const next=await invoke(page,'project.newConversation',{projectId:created.projectId,sourceAssessmentId:created.assessmentId,
    title:'Segunda sessão sintética',idempotencyKey:randomUUID()});
  assert.notEqual(next.id,created.assessmentId);
  await close();page=await start();
  const reopened=await invoke(page,'snapshot');assert.equal(reopened.projects.length,1);
  assert.deepEqual(reopened.assessments.map(a=>a.id).sort(),[created.assessmentId,next.id].sort());
  assert.equal(reopened.executions?.length??0,0);assert.equal(reopened.findings.length,0);
  const target=new URL(page.url());target.searchParams.set('area','tecnica');await page.goto(target.href);
  await expect(page.getByRole('region',{name:'Conversa do projeto'})).toBeVisible();
  await expect(page.getByRole('button',{name:/Área técnica|Ferramentas do Kali|^Avaliações$/})).toHaveCount(0);
  await page.getByRole('button',{name:'Painel do projeto'}).click();
  const panel=page.getByRole('complementary',{name:'Projeto',exact:true});
  await expect(panel.getByRole('button',{name:'Conversas 2'})).toBeVisible();
  await expect(panel.getByRole('button',{name:'Execuções 0'})).toBeVisible();
  await page.screenshot({path:join(folder,'02-painel.png')});result.screenshots.push('02-painel.png');
  await panel.getByText('Comparar avaliações',{exact:true}).click();
  await panel.getByLabel('Avaliação A',{exact:true}).selectOption(created.assessmentId);
  await panel.getByLabel('Avaliação B',{exact:true}).selectOption(next.id);
  await panel.getByRole('button',{name:'Comparar registros'}).click();
  const comparison=panel.getByRole('region',{name:'Resultado da comparação'});
  await expect(comparison.getByRole('region',{name:'Varreduras do Codex Security'})).toContainText('Mesmos valores exibidos');
  await expect(comparison.getByText('0',{exact:true})).toHaveCount(2);
  await expect(comparison).not.toContainText('Kali');
  await panel.getByRole('tab',{name:'Configurações',exact:true}).click();
  await expect(panel.getByRole('heading',{name:'Pasta do projeto'})).toBeVisible();
  await expect(panel.getByText('Alvos e escopo',{exact:true})).toHaveCount(0);
  result.checks.push('Comparação de duas conversas usa registros do SDK e a configuração conserva a pasta local.');
  assert.equal(digest(await readFile(source)),sourceSha256);
  result.checks.push('Reabertura preserva projeto, conversa e fonte; URL técnica continua no workspace de código.');
  result.passed=true;
}catch(error){result.error=String(error);throw error;}
finally{
  try{await close();}finally{await writeFile(join(folder,'result.json'),JSON.stringify(result,null,2));console.log(JSON.stringify({folder,...result}));}
}
