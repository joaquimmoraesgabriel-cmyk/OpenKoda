import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import { emptySnapshot } from '../src/preview';
import type { Finding } from '../src/types';
import type { UsageRow } from '../src/usage-history';

async function openPanel(page:Page, failUsage=false) {
  const snapshot=emptySnapshot();
  const at='2026-11-12T12:00:00Z';
  snapshot.projects=['portal','externo'].map(id=>({id,name:id==='portal'?'Portal local':'Outro projeto',kind:'code',target:'D:/fixture/'+id,description:'Revisão de autorização.',createdAt:at}));
  snapshot.assessments=Array.from({length:12},(_,index)=>{
    const projectId=index===11?'externo':'portal';
    const title=['Análise Alfa','Revisão Beta'][index]??(projectId==='externo'?'Conversa externa':'Consulta '+String(index+1).padStart(2,'0'));
    return {id:'a'+index,projectId,title,kind:'code',target:'D:/fixture/'+projectId,objective:title,events:[],evidence:[],coverage:[],summary:'',status:'completed',createdAt:at,updatedAt:at.replace('12:00:',String(index).padStart(2,'0')+':00:')};
  });
  const findings:[string,string,Finding['severity'],Finding['status']][]=[
    ['a0','Autorização do portal','critical','candidate'],['a1','Registro de acesso','low','fixed'],['a11','Achado externo','critical','validated'],
  ];
  snapshot.findings=findings.map(([assessmentId,title,severity,status],i)=>({id:'f'+i,assessmentId,title,severity,status,description:'Resultado sintético.',recommendation:'Revisar.',confidence:'high',evidenceIds:[],createdAt:at}));
  snapshot.plans=['a0','a1','a11'].map((assessmentId,i)=>({id:'p'+i,assessmentId,adapter:['source.rules.v1','inventory.files.v1','externo.v1'][i],target:'D:/fixture',timeoutSeconds:10,outputLimitKiB:16,inputFiles:[],resolvedAddresses:[],digest:'digest'+i,createdAt:at,expiresAt:at,approvedAt:at,status:'consumed'}));
  snapshot.executions=snapshot.plans.map((plan,i)=>({id:'r'+i,assessmentId:plan.assessmentId,planId:plan.id,status:'completed',startedAt:at,finishedAt:at,evidenceId:null,message:''}));
  const usage:UsageRow[]=[
    {day:'2026-11-11',provider:'openai',model:'modelo-A',assessmentId:'a0',requests:2,input:3,output:7,partial:0},
    {day:'2026-11-12',provider:'openai',model:'modelo-A',assessmentId:'a1',requests:1,input:2,output:null,partial:1},
    {day:'2026-11-12',provider:'openrouter',model:'modelo-A',assessmentId:'a0',requests:1,input:null,output:null,partial:1},
    {day:'2026-11-12',provider:'openai',model:'modelo-externo',assessmentId:'a11',requests:8,input:100,output:40,partial:0},
    {day:'2026-11-12',provider:'openai',model:'modelo-sem-projeto',assessmentId:null,requests:9,input:20,output:20,partial:0},
  ];
  await page.addInitScript(({snapshot,usage,failUsage})=>{
    let unavailable=failUsage;
    window.addEventListener('fixture:budget-online',()=>{unavailable=false;},{once:true});
    Object.assign(window,{isTauri:true,__TAURI_INTERNALS__:{invoke:async(_command:string,args:{request?:{action:string}})=>{
      switch(args?.request?.action){
        case 'snapshot':return snapshot;
        case 'environment.status':return {tools:[],status:'unavailable',message:'Fixture'};
        case 'ai.history':return {messages:[]};
        case 'ai.chat.status':return {requestId:null,stopping:false};
        case 'ai.budget.history':
          if(unavailable)throw new Error('Histórico temporariamente indisponível.');
          return {rows:usage};
        default:return {};
      }
    }}});
  },{snapshot,usage,failUsage});
  await page.goto('/');
  await page.getByRole('button',{name:'Painel do projeto'}).click();
  return page.getByRole('complementary',{name:'Projeto',exact:true});
}

test('conversas do projeto permitem paginar, ordenar, buscar por acento e abrir a seleção',async({page})=>{
  const panel=await openPanel(page);
  const table=panel.getByRole('table',{name:'Conversas'});
  await expect(panel.getByRole('button',{name:'Conversas 11'})).toHaveAttribute('aria-pressed','true');
  await expect(table.getByRole('rowheader')).toHaveCount(10);
  await panel.getByRole('button',{name:'Próxima página'}).click();
  await expect(table.getByRole('rowheader')).toHaveCount(1);
  await expect(table.getByRole('button',{name:'Análise Alfa',exact:true})).toBeVisible();
  await table.getByRole('button',{name:'Ordenar por Conversa',exact:true}).click();
  const heading=table.getByRole('columnheader',{name:'Ordenar por Conversa',exact:true});
  await expect(heading).toHaveAttribute('aria-sort','ascending');
  await expect(table.getByRole('rowheader').first()).toHaveText('Análise Alfa');
  await heading.getByRole('button').click();
  await expect(heading).toHaveAttribute('aria-sort','descending');
  await expect(table.getByRole('rowheader').first()).toHaveText('Revisão Beta');
  await panel.getByRole('searchbox',{name:'Buscar em conversas'}).fill('analise');
  await expect(table.getByRole('rowheader')).toHaveCount(1);
  await table.getByRole('button',{name:'Análise Alfa',exact:true}).click();
  await expect(panel).toBeHidden();
  await expect(page.getByRole('complementary',{name:'Projetos e conversas'}).getByRole('button',{name:'Análise Alfa',exact:true})).toHaveAttribute('aria-current','page');
});

test('execuções e achados usam vínculos reais e preservam os filtros combinados',async({page},info)=>{
  const panel=await openPanel(page);
  await panel.getByRole('button',{name:'Execuções 2'}).click();
  const runs=panel.getByRole('table',{name:'Execuções'});
  await expect(runs.getByRole('row',{name:/source.rules.v1 Análise Alfa/})).toBeVisible();
  await panel.getByRole('searchbox',{name:'Buscar em execuções'}).fill('externo');
  await expect(panel.getByText('Nenhum registro corresponde aos filtros.')).toBeVisible();
  await panel.getByRole('button',{name:'Achados 2'}).click();
  const table=panel.getByRole('table',{name:'Achados'});
  await expect(table.getByRole('rowheader').first()).toContainText('Autorização do portal');
  await panel.screenshot({path:info.outputPath('painel-projeto-achados.png')});
  await panel.getByRole('combobox',{name:'Estado do achado'}).selectOption('fixed');
  await panel.getByRole('combobox',{name:'Severidade do achado'}).selectOption('critical');
  await expect(panel.getByText('Nenhum registro corresponde aos filtros.')).toBeVisible();
  await panel.getByRole('combobox',{name:'Severidade do achado'}).selectOption('low');
  await expect(table.getByRole('rowheader')).toHaveCount(1);
  await expect(table.getByRole('rowheader')).toHaveText('Registro de acesso');
  await page.setViewportSize({width:520,height:720});
  await expect(panel.getByRole('tab',{name:'Configurações'})).toBeInViewport();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  await panel.screenshot({path:info.outputPath('painel-projeto-compacto.png')});
});

test('histórico de uso se recupera de falha e distingue uso parcial, desconhecido e externo',async({page})=>{
  const panel=await openPanel(page,true);
  await panel.getByRole('button',{name:/^Uso de IA/}).click();
  await expect(panel.getByRole('alert')).toHaveText('Histórico temporariamente indisponível.');
  await page.evaluate(()=>window.dispatchEvent(new Event('fixture:budget-online')));
  await panel.getByRole('button',{name:'Tentar novamente'}).click();
  await expect(panel.getByRole('button',{name:'Uso de IA 4 chamadas'})).toBeVisible();
  const table=panel.getByRole('table',{name:'Uso de IA'});
  await expect(table.getByRole('rowheader')).toHaveCount(2);
  await expect(table.getByRole('row',{name:/modelo-A OpenAI 3 12 \(parcial\)/})).toBeVisible();
  await expect(table.getByRole('row',{name:/modelo-A OpenRouter 1 Não informado/})).toBeVisible();
  await table.getByRole('button',{name:'Ordenar por Tokens medidos'}).click();
  await expect(table.getByRole('row').last()).toContainText('Não informado');
  await panel.getByRole('searchbox',{name:'Buscar em uso de ia'}).fill('OpenRouter');
  await expect(table.getByRole('rowheader')).toHaveCount(1);
});
