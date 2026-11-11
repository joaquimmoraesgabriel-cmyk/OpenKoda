import type { Page } from '@playwright/test';
import { test, expect } from './fixtures';
import { emptySnapshot } from '../src/preview';

const at = '2026-11-11T00:00:00Z';
async function openHistory(page:Page, unavailable=false) {
  const base = emptySnapshot();
  const project = {id:'p',name:'Laboratório',kind:'code',target:'C:/fixture/v1',description:'',createdAt:at};
  const assessments = ['a','b','legacy','foreign'].map((id,i) => ({
    id,projectId:id==='foreign'?'other':'p',scopeId:'scope-'+id,title:['Primeira','Segunda','Legada','Outro projeto secreto'][i],
    kind:'code',target:'C:/fixture/v'+(i+1),objective:'Revisão',status:'draft',createdAt:at,updatedAt:at,events:[],evidence:[],coverage:[],summary:'',
  }));
  const data = {...base,projects:[project,{...project,id:'other',name:'Separado'}],assessments,
    assessmentScopes:assessments.slice(0,2).map(a=>({id:a.scopeId,assessmentId:a.id,currentRevisionId:a.id+'-r',frozen:{revisionId:a.id+'-r',digest:a.id+'-digest'}})),
    scopeRevisions:assessments.slice(0,2).map((a,i)=>({id:a.id+'-r',assessmentId:a.id,digest:a.id+'-digest',body:{kind:'code',target:a.target,methods:[i?'source.rules.v1':'local.inventory.v1']}})),
    plans:assessments.slice(0,2).map(a=>({id:a.id+'-plan',assessmentId:a.id,target:a.target,adapter:'source.rules.v1',sourceScope:{manifestSha256:a.id.repeat(64)}})),
    executions:[{id:'observed',assessmentId:'b',planId:'b-plan'}],
  };
  await page.addInitScript(({data,unavailable})=>{
    Object.assign(window,{isTauri:true,__TAURI_INTERNALS__:{invoke:async (_:string,args:{request:{action:string;payload:{projectId?:string;assessmentIds?:string[]}}})=>{
      switch(args.request.action) {
        case 'snapshot': return structuredClone(data);
        case 'environment.status': return {status:'unavailable',distro:'fixture',message:'Indisponível',tools:[]};
        case 'projectPurge.status': return {hasUnresolved:false,hasPartialHistory:false};
        case 'kali.mcp.assessmentEnvironments':
          if(unavailable) throw new Error('Sem histórico');
          if(args.request.payload.projectId!=='p'||args.request.payload.assessmentIds?.join(',')!=='a,b') throw new Error('Consulta fora do projeto selecionado.');
          return [{assessmentId:'a',environmentId:'ambiente-anterior'},{assessmentId:'b',environmentId:'ambiente-novo'}];
        default: throw new Error('Consulta indisponível na fixture.');
      }
    }},moveAssessment:()=>{data.assessments[1].projectId='other';}});
  },{data,unavailable});
  await page.goto('/?area=tecnica');
  await page.getByRole('button',{name:'Projetos',exact:true}).click();
  await page.getByRole('button',{name:/Laboratório/}).click();
  await page.getByRole('navigation',{name:'Seções'}).getByRole('button',{name:'Avaliações'}).click();
  await page.getByText('Comparar avaliações',{exact:true}).click();
  await page.getByLabel('Avaliação A',{exact:true}).selectOption('a');
}

test('compara alvos, versões das fontes, ambientes e métodos do mesmo projeto',async({page})=>{
  await openHistory(page);
  const second=page.getByLabel('Avaliação B',{exact:true});
  await expect(second.locator('option[value="a"]')).toBeDisabled();
  await expect(second.locator('option[value="foreign"]')).toHaveCount(0);
  await second.selectOption('b');
  await page.getByRole('button',{name:'Comparar registros'}).click();
  const panel=page.getByRole('region',{name:'Resultado da comparação'});
  for(const [label,values] of [
    ['Alvos registrados',['C:/fixture/v1','C:/fixture/v2']],
    ['Métodos da revisão de escopo',['local.inventory.v1','source.rules.v1']],
    ['Fontes nos planos locais',['a'.repeat(64),'b'.repeat(64)]],
    ['Identidades de ambiente',['ambiente-anterior','ambiente-novo']],
  ] as const) {
    const row=panel.getByRole('region',{name:new RegExp(label)});
    await expect(row).toContainText('Registros diferentes');
    for(const value of values) await expect(row).toContainText(value);
  }
  await expect(panel).not.toContainText('privado');
  await expect(panel).toContainText('A correção de um achado exige reteste e confirmação.');
  await second.selectOption('legacy');
  await expect(panel).toHaveCount(0);
});

test('mostra limites de registros legados e de histórico Kali indisponível',async({page})=>{
  await openHistory(page,true);
  await page.getByLabel('Avaliação B',{exact:true}).selectOption('legacy');
  await page.getByRole('button',{name:'Comparar registros'}).click();
  await expect(page.getByText('O histórico de ambientes do Kali está indisponível.')).toBeVisible();
  const row=page.getByRole('region',{name:'Métodos da revisão de escopo'});
  await expect(row).toContainText('Dados insuficientes');
  await expect(row).toContainText('Sem registro disponível.');
});

test('valida o projeto no novo snapshot antes de mostrar os dados',async({page})=>{
  await openHistory(page);
  await page.getByLabel('Avaliação B',{exact:true}).selectOption('b');
  await page.evaluate(()=> (window as unknown as {moveAssessment:()=>void}).moveAssessment());
  await page.getByRole('button',{name:'Comparar registros'}).click();
  await expect(page.getByText('Não foi possível consultar as avaliações deste projeto. Atualize a comparação.')).toBeVisible();
  await expect(page.getByRole('region',{name:'Resultado da comparação'})).toHaveCount(0);
});
