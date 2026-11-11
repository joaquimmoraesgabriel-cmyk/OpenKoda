import { type Page } from '@playwright/test';
import { expect, test } from './fixtures';
import { installScopeFixture } from './scope-fixture';
import { reviewKaliData } from './data-fixture';
import { emptySnapshot } from '../src/preview';

async function launchFixture(page:Page) {
  await page.addInitScript(({snapshot})=>{
    const calls: {action:string;payload:Record<string,unknown>}[]=[];
    const tools = [
      {package:'ripgrep',group:'Código',installed:true,adapter:'kali.rg.signals.v1',adapterImplemented:true},
      {package:'file',group:'Arquivos',installed:false,adapter:'offline-file'},
      {package:'curl',group:'Web e API',installed:true,adapter:'typed-http'},
      {package:'nmap',group:'Rede',installed:true,adapter:'not-integrated'},
    ];
    Object.assign(window,{isTauri:true,__launchCalls:calls,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request?:typeof calls[number]})=>{
      const req=args?.request;
      if (!req) return {};
      calls.push(req);
      if(req.action==='snapshot')return snapshot;
      if(req.action==='packages.catalog')return {inventoryVerified:true,tools};
      if(req.action==='project.pickTarget')return 'D:/Fixture/Sample';
      if(req.action==='project.launch')throw new Error('Falha de criação simulada.');
      if(req.action==='environment.status')return {tools:[],status:'unavailable',message:'Ambiente de teste'};
      return {};
    }}});
  },{snapshot:emptySnapshot()});
  await page.goto('/');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
}

async function submissions(page:Page) {
  return page.evaluate(()=>((window as unknown as {__launchCalls:{action:string;payload:Record<string,unknown>}[]}).__launchCalls)
    .filter(call=>call.action==='project.launch').map(call=>call.payload));
}

test('o assistente restringe a pasta selecionada e mantém o mesmo pedido ao tentar novamente',async({page})=>{
  await launchFixture(page);
  await page.getByRole('button',{name:'Escolher pasta'}).click();
  await expect(page.locator('.launch-name')).toContainText('Sample');
  await page.getByLabel('Alvo',{exact:true}).selectOption('subset');
  await page.getByLabel('Caminho dentro do projeto').fill('D:/Fixture/fora/index.ts');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect(page.locator('.launch-page [role=alert]')).toContainText('dentro da pasta');
  expect(await submissions(page)).toHaveLength(0);
  await page.getByLabel('Caminho dentro do projeto').fill('D:/Fixture/Sample/src/index.ts');
  await page.getByRole('switch',{name:'Contexto adicional'}).check();
  await page.getByLabel('Áreas de foco').fill('Rotas administrativas');
  await page.getByRole('switch',{name:'Opções avançadas'}).check();
  await expect(page.getByRole('checkbox',{name:/^ripgrep /})).toBeVisible();
  await expect(page.getByRole('checkbox',{name:/^curl /})).toHaveCount(0);
  await expect(page.getByRole('checkbox',{name:/^file /})).toHaveCount(0);
  await page.getByLabel('Título da conversa').fill('Título desativado');
  await page.getByRole('switch',{name:'Opções avançadas'}).uncheck();
  await expect(page.getByLabel('Título da conversa')).toHaveCount(0);
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect(page.locator('.launch-page [role=alert]')).toContainText('Falha de criação simulada');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect.poll(async()=>(await submissions(page)).length).toBe(2);
  const [first,second]=await submissions(page);
  expect(first).toEqual(second);
  expect(first).toMatchObject({root:'D:/Fixture/Sample',title:'Análise inicial do código',kind:'code',relativeTarget:'src/index.ts',minutes:null,useRipgrep:false});
  expect(first.objective).toContain('Rotas administrativas');
});

test('cada modalidade apresenta suas ferramentas instaladas e os campos do alvo',async({page})=>{
  await launchFixture(page);
  await page.getByRole('button',{name:'Análise Web / API'}).click();
  await page.getByLabel('URL do site ou da API').fill('https://teste.invalid/api');
  await page.getByRole('switch',{name:'Opções avançadas'}).check();
  await expect(page.getByText('Nenhuma ferramenta instalada com suporte a esta análise.')).toBeVisible();
  await expect(page.getByRole('checkbox',{name:/^curl /})).toHaveCount(0);
  await expect(page.getByRole('checkbox',{name:/^nmap /})).toHaveCount(0);
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect.poll(async()=>(await submissions(page)).length).toBe(1);
  expect((await submissions(page))[0]).toMatchObject({kind:'web',name:'teste.invalid',root:'https://teste.invalid/api',ports:[]});
  await page.getByRole('button',{name:'Análise de rede'}).click();
  await expect(page.getByLabel('URL do site ou da API')).toHaveCount(0);
  await expect(page.getByRole('checkbox',{name:/^nmap /})).toHaveCount(0);
  await page.getByLabel('IP ou rede CIDR').fill('127.0.0.0/30');
  await page.getByLabel('Portas TCP').fill('70000');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect(page.locator('.launch-page [role=alert]')).toContainText('65535');
  expect(await submissions(page)).toHaveLength(1);
  await page.getByLabel('Portas TCP').fill('80 443');
  await page.getByRole('button',{name:'Criar projeto',exact:true}).click();
  await expect.poll(async()=>(await submissions(page)).length).toBe(2);
  expect((await submissions(page))[1]).toMatchObject({kind:'network',ports:[80,443],root:'127.0.0.0/30'});
});

test('a conversa permite usar ripgrep somente sobre a evidência escolhida e revisada',async({page})=>{
  await installScopeFixture(page);
  await page.addInitScript(({settings})=>{
    const now=new Date().toISOString();
    const project={id:'project-rg',name:'Código',description:'Amostra',kind:'code',target:'D:/Fixture/Sample',createdAt:now};
    const assessment={id:'assessment-rg',projectId:project.id,title:'Revisão local',kind:'code',target:project.target,objective:'Testar',status:'partial',
      events:[],coverage:[],summary:'Resumo da operação local.',createdAt:now,updatedAt:now,
      evidence:[{id:'rg-evidence',name:'Amostra registrada',content:'exemplo sintético',sha256:'fixture-hash',createdAt:now}]};
    const state={projects:[project],assessments:[assessment],settings,findings:[],plans:[],executions:[]};
    let scope:ReturnType<typeof window.__scopeFixture>;
    Object.assign(window,{isTauri:true,__TAURI_INTERNALS__:{invoke:async(_name:string,args:{request?:{action:string;payload:Record<string,unknown>}})=>{
      const r=args?.request;
      if(!r)return {};
      if(r.action==='snapshot')return state;
      if(r.action==='assessment.scopeStatus')return (scope??=window.__scopeFixture(state,['source.rules.v1','kali.rg.signals.v1'])).status(r.payload.assessmentId);
      if(r.action==='ai.history')return {messages:[]};
      if(r.action==='ai.chat.status')return {requestId:null,stopping:false};
      if(r.action==='kali.mcp.status')return {active:null,recent:[],hasUnresolved:false,capabilities:['scan_bytes']};
      if(r.action==='kali.mcp.scanEvidence')throw new Error('Indisponibilidade simulada do Kali.');
      return {};
    }}});
  },{settings:emptySnapshot().settings});
  await page.goto('/');
  await page.getByText('Analisar evidência com ripgrep',{exact:true}).click();
  const panel=page.locator('.project-chat .kali-bridge');
  await expect(panel.getByRole('button',{name:'Verificar hash no Kali'})).toHaveCount(0);
  await panel.getByLabel('Evidência para verificar no Kali',{exact:true}).selectOption('rg-evidence');
  const scan=panel.getByRole('button',{name:'Analisar evidência no Kali',exact:true});
  await expect(scan).toBeDisabled();
  await reviewKaliData(panel,'scan');
  await scan.click();
  await expect(page.getByRole('alert').filter({hasText:'Indisponibilidade simulada'}).first()).toBeVisible();
  const scans=await page.evaluate(()=>((window as unknown as {__dataCalls:{action:string;payload:Record<string,unknown>}[]}).__dataCalls).filter(c=>c.action==='kali.mcp.scanEvidence'));
  expect(scans).toHaveLength(1);
  expect(scans[0].payload).toMatchObject({assessmentId:'assessment-rg',evidenceId:'rg-evidence',acknowledgeLocalTransfer:true});
});
