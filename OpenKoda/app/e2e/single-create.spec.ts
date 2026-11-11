import { Page } from '@playwright/test';
import { emptySnapshot, previewStorageKey } from '../src/preview';
import { test, expect } from './fixtures';

const project={id:'test-project',name:'Projeto de recibos',kind:'code' as const,target:'D:/fixture',description:'',createdAt:'2026-01-01T12:00:00Z'};
async function newForm(page:Page,title:string) {
  await page.getByRole('button',{name:'Nova avaliação',exact:true}).click();
  await page.getByLabel('Título',{exact:true}).fill(title);
  await page.getByLabel('Objetivo',{exact:true}).fill('Registrar um rascunho local.');
}
async function review(page:Page,scoped:boolean,native=true) {
  await page.getByRole('button',{name:'Revisar avaliação',exact:true}).click();
  if (scoped) await page.getByRole('checkbox',{name:/Revisei o objetivo/}).check();
  const name=scoped?'Salvar avaliação com escopo revisado':native?'Salvar rascunho sem revisar escopo':'Salvar rascunho';
  await page.getByRole('button',{name,exact:true}).click();
}
async function installNative(page:Page,loss:'snapshot'|'reply') {
  const snapshot=emptySnapshot();
  snapshot.projects.push(project);
  await page.addInitScript(({snapshot,loss})=>{
    const calls:Record<string,unknown>[]=[];
    const ledger=new Map<string,{body:string;assessment:typeof snapshot.assessments[number]}>();
    let lost=false, nextSnapshotFails=false;
    const catalog=[{id:'local.inventory.v1',label:'Inventário local',allowed:true,reasonCodes:[],effects:'Leitura local',windowEnforcement:'local_deadline'}];
    Object.assign(window,{isTauri:true,__sentSingle:calls,__TAURI_INTERNALS__:{
      invoke:async (cmd:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
        if(cmd!=='request') throw new Error('Unexpected IPC');
        const {action,payload}=args.request;
        switch(action){
          case 'snapshot':
            if(nextSnapshotFails){nextSnapshotFails=false;throw new Error('Leitura posterior indisponível');}
            return structuredClone(snapshot);
          case 'projectPurge.status':return {hasUnresolved:false,journals:[]};
          case 'data.session.select': return {};
          case 'environment.status':return {status:'unavailable',distro:'Fixture',tools:[],message:'Fixture'};
          case 'assessment.scopeOptions':return {schema:'openkoda.scope-options.v1',projectId:snapshot.projects[0].id,kind:'code',target:snapshot.projects[0].target,
            projectDigest:lost?'updated-digest':'original-digest',catalogVersion:'fixture-v1',methodCatalog:catalog,
            defaultProposal:{methods:[],window:{startsAt:null,endsAt:null}},canCreate:true,reasonCodes:[],limits:[]};
          case 'assessment.create': {
            calls.push(structuredClone(payload));
            const {idempotencyKey,...content}=payload;
            if(typeof idempotencyKey!=='string') throw new Error('Falta a chave do pedido');
            const body=JSON.stringify(content),saved=ledger.get(idempotencyKey);
            if(saved){if(saved.body!==body)throw new Error('scope_idempotency_conflict');return structuredClone(saved.assessment);}
            const a={id:crypto.randomUUID(),projectId:String(content.projectId),kind:'code' as const,title:String(content.title),target:String(content.target),objective:String(content.objective),
              status:'draft' as const,createdAt:'2026-01-01T12:00:00Z',updatedAt:'2026-01-01T12:00:00Z',summary:'',events:[],coverage:[],evidence:[]};
            snapshot.assessments.unshift(a);ledger.set(idempotencyKey,{body,assessment:a});
            if(!lost){lost=true;if(loss==='reply')throw new Error('Resposta perdida após salvar');nextSnapshotFails=true;}
            return structuredClone(a);
          }
          default: throw new Error('Optional service unavailable');
        }
      },
    }});
  },{snapshot,loss});
}
const sent=(page:Page)=>page.evaluate(()=>Reflect.get(window,'__sentSingle') as {idempotencyKey:string;scope?:{projectDigest:string};title:string}[]);

test('uma leitura falha após salvar o escopo e o reenvio recupera exatamente o pedido original',async({page})=>{
  await installNative(page,'snapshot');await page.goto('/');
  await newForm(page,'Revisão recuperada');
  await page.getByRole('checkbox',{name:/Inventário local/}).check();
  await review(page,true);
  await expect(page.getByRole('button',{name:'Reenviar a mesma solicitação'})).toBeVisible();
  expect(await sent(page)).toHaveLength(1);
  // Reviewing the unchanged form again must preserve the original catalog digest and key.
  await review(page,true);
  await expect(page.getByRole('heading',{name:'Revisão recuperada',exact:true})).toBeVisible();
  const calls=await sent(page);
  expect(calls).toHaveLength(2);expect(calls[1]).toEqual(calls[0]);
  expect(calls[0].idempotencyKey).toMatch(/^[a-f0-9-]{36}$/);expect(calls[0].scope?.projectDigest).toBe('original-digest');
});

test('rascunho permite recuperar a resposta perdida e usa outra chave para uma nova avaliação idêntica',async({page})=>{
  await installNative(page,'reply');await page.goto('/');
  await newForm(page,'Mesmo título');await review(page,false);
  await page.getByRole('button',{name:'Reenviar a mesma solicitação'}).click();
  await expect(page.getByRole('heading',{name:'Mesmo título',exact:true})).toBeVisible();
  await newForm(page,'Mesmo título');await review(page,false);
  await expect(page.getByRole('heading',{name:'Mesmo título',exact:true})).toBeVisible();
  const calls=await sent(page);
  expect(calls).toHaveLength(3);expect(calls[1]).toEqual(calls[0]);
  expect(calls[0].scope).toBeUndefined();expect(calls[2].idempotencyKey).not.toBe(calls[0].idempotencyKey);
});

test('editar materialmente o rascunho inicia uma intenção diferente',async({page})=>{
  await installNative(page,'reply');await page.goto('/');
  await newForm(page,'Primeiro título');await review(page,false);
  const retry=page.getByRole('button',{name:'Reenviar a mesma solicitação'});
  await expect(retry).toBeVisible();
  await page.getByLabel('Título',{exact:true}).fill('Outro objetivo de revisão');
  await expect(retry).toHaveCount(0);
  await review(page,false);
  await expect(page.getByRole('heading',{name:'Outro objetivo de revisão',exact:true})).toBeVisible();
  const calls=await sent(page);
  expect(calls).toHaveLength(2);expect(calls[1].idempotencyKey).not.toBe(calls[0].idempotencyKey);
});

test('a prévia recupera a mesma avaliação após a falha de atualização do painel',async({page})=>{
  const snapshot=emptySnapshot();snapshot.projects.push(project);
  await page.addInitScript(({key,snapshot})=>{
    localStorage.setItem(key,JSON.stringify(snapshot));
    const get=Storage.prototype.getItem,set=Storage.prototype.setItem;
    let committed=false,failNextRead=false;
    Storage.prototype.setItem=function(k,v){
      set.call(this,k,v);
      if(k===key&&!committed&&JSON.parse(v).assessments.length){committed=true;failNextRead=true;}
    };
    Storage.prototype.getItem=function(k){
      if(k===key&&failNextRead){failNextRead=false;throw new Error('Leitura posterior indisponível');}
      return get.call(this,k);
    };
  },{key:previewStorageKey,snapshot});
  await page.goto('/');await newForm(page,'Rascunho único');await review(page,false,false);
  const retry=page.getByRole('button',{name:'Reenviar a mesma solicitação'});
  await expect(retry).toBeVisible();
  const bytes=()=>page.evaluate(key=>localStorage.getItem(key),previewStorageKey);
  const original=await bytes();
  await retry.click();
  await expect(page.getByRole('heading',{name:'Rascunho único',exact:true})).toBeVisible();
  expect(await bytes()).toBe(original);expect(JSON.parse(original!).assessments).toHaveLength(1);
});

test('duas abas que reenviam a mesma chave recebem uma única avaliação da prévia',async({page,context})=>{
  const snapshot=emptySnapshot();snapshot.projects.push(project);
  await context.addInitScript(({key,snapshot})=>{
    if (!localStorage.getItem(key)) localStorage.setItem(key,JSON.stringify(snapshot));
  },{key:previewStorageKey,snapshot});
  const second=await context.newPage(), tabs=[page,second];
  await Promise.all(tabs.map(async tab=>{
    await tab.goto('/');await newForm(tab,'Solicitação compartilhada');
    await tab.getByRole('button',{name:'Revisar avaliação',exact:true}).click();
    // Replay one user's intent in two documents. Only the form's next UUID is fixed.
    await tab.evaluate(()=>{
      const original=crypto.randomUUID.bind(crypto);
      crypto.randomUUID=()=>{crypto.randomUUID=original;return '93402016-ad76-47c2-8e49-b9e9410884ad';};
    });
  }));
  await Promise.all(tabs.map(tab=>tab.getByRole('button',{name:'Salvar rascunho',exact:true}).click()));
  await Promise.all(tabs.map(tab=>expect(tab.getByRole('heading',{name:'Solicitação compartilhada',exact:true})).toBeVisible()));
  const state=await second.evaluate(key=>JSON.parse(localStorage.getItem(key)!),previewStorageKey);
  expect(state.assessments).toHaveLength(1);
  expect(Object.keys(state.creationReceipts)).toEqual(['93402016-ad76-47c2-8e49-b9e9410884ad']);
  expect(state.creationReceipts['93402016-ad76-47c2-8e49-b9e9410884ad'].id).toBe(state.assessments[0].id);
});
