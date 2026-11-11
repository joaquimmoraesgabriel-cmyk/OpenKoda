import { openAssessmentList } from './navigation';
import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// Mocked IPC only. No test in this file opens a target TCP connection.
async function networkFixture(page:Page,mode='success'){
  await installScopeFixture(page);
  await page.addInitScript(({mode})=>{
    const fixture=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__networkCalls:{action:string;payload:Record<string,unknown>}[]};
    fixture.isTauri=true;fixture.__networkCalls=[];const now=new Date().toISOString();
    const assessment={id:'network-assessment',projectId:'network-project',title:'Observação de rede de contrato',kind:'network',target:'192.0.2.0/30',objective:'Observar portas explicitamente autorizadas.',status:'draft',createdAt:now,updatedAt:now,events:[],evidence:[] as {id:string;name:string;content:string;sha256:string;createdAt:string}[],coverage:[],summary:''};
    const state={projects:[{id:'network-project',name:'Rede de contrato',kind:'network',target:assessment.target,description:'Fixture sem tráfego.',createdAt:now}],assessments:[assessment],findings:[],settings:{provider:'openai',endpoint:'https://fixture.example.test/v1',model:'fixture',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false},plans:[] as Record<string,unknown>[],executions:[] as Record<string,unknown>[],validations:[]};
    const scope=window.__scopeFixture(state,['network.tcp-connect.v1']);
    fixture.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');const {action,payload}=args.request;fixture.__networkCalls.push({action,payload});
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture sem Kali.',tools:[],executorReady:false};
      if(action==='assessment.networkPlan'){
        if(mode==='rejected')throw new Error('O escopo excede 64 combinações de IP e porta. Nenhuma conexão iniciada.');
        const ports=(payload.ports as number[]).slice().sort((a,b)=>a-b);
        const plan={scopeRef:scope.ref,id:'network-plan',assessmentId:assessment.id,adapter:'network.tcp-connect.v1',target:assessment.target,timeoutSeconds:10,outputLimitKiB:16,inputFiles:[],resolvedAddresses:[0,1,2,3].flatMap(last=>ports.map(port=>`192.0.2.${last}:${port}`)),digest:'network-fixture-digest',createdAt:now,expiresAt:new Date(Date.now()+(mode==='expired'?-1000:600000)).toISOString(),approvedAt:null,status:'proposed'};state.plans=[plan];assessment.status='awaiting_approval';return structuredClone(plan);
      }
      if(action==='plan.approve'||action==='plan.run'||action==='plan.revoke'){
        const plan=state.plans[0];if(payload.id!==plan?.id||payload.digest!==plan?.digest)throw new Error('Digest ou identidade divergente.');
        if(action==='plan.approve'){plan.status='approved';plan.approvedAt=now;assessment.status='queued';return structuredClone(plan);}
        if(action==='plan.revoke'){plan.status='revoked';assessment.status='blocked';return structuredClone(plan);}
        if(plan.status!=='approved')throw new Error('Plano ainda não aprovado.');
        plan.status='completed';assessment.status='partial';assessment.summary='Observação TCP limitada registrada. Nenhuma auditoria completa.';
        const evidence={id:'tcp-evidence',name:'Observação TCP de contrato',content:JSON.stringify({observations:[{address:'192.0.2.1:443',status:'open'}],limited:true}),sha256:'fixture-sha256',createdAt:now};assessment.evidence.push(evidence);
        const execution={id:'tcp-execution',planId:plan.id,assessmentId:assessment.id,status:'completed',startedAt:now,finishedAt:now,evidenceId:evidence.id,message:'Observação TCP delimitada concluída; cobertura parcial.'};state.executions.push(execution);return structuredClone(execution);
      }
      throw new Error('Ação não prevista no contrato TCP: '+action);
    }};
  },{mode});
  await page.goto('/');await openAssessmentList(page);await page.getByText('Observação de rede de contrato',{exact:true}).click();
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__networkCalls:{action:string;payload:Record<string,unknown>}[]}).__networkCalls.filter(call=>call.action===name),action);}

test('TCP exige portas explícitas e revisão dos destinos antes de aprovar e executar',async({page})=>{
  await networkFixture(page);const ports=page.getByLabel('Portas TCP autorizadas',{exact:true});
  await expect(ports).toHaveValue('');await expect(page.getByRole('button',{name:'Preparar plano TCP',exact:true})).toBeDisabled();
  await expect(page.getByRole('button',{name:'Iniciar avaliação',exact:true})).toHaveCount(0);
  expect(await calls(page,'assessment.networkPlan')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
  await ports.fill('443, 22');await page.getByRole('button',{name:'Preparar plano TCP',exact:true}).click();
  expect((await calls(page,'assessment.networkPlan'))[0].payload).toEqual({id:'network-assessment',ports:[443,22]});
  await expect(page.getByText('Observar conectividade TCP',{exact:true})).toBeVisible();
  const planReview=page.locator('.content-section').filter({has:page.getByRole('heading',{name:'Plano de execução',exact:true})}).locator('.review-sheet');
  await expect(planReview).toContainText('192.0.2.0:22');await expect(planReview).toContainText('192.0.2.3:443');
  await expect(planReview).toContainText('8 combinação(ões) de IP e porta');
  await expect(planReview).not.toContainText('HTTP HEAD');
  expect(await calls(page,'plan.approve')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
  await expect(page.getByRole('button',{name:'Aprovar plano',exact:true})).toBeDisabled();
  await page.screenshot({path:test.info().outputPath('tcp-revisao.png'),fullPage:true});
  await page.getByRole('checkbox',{name:'Revisei o alvo, os efeitos e os limites deste plano e autorizo esta operação.'}).check();
  await page.getByRole('button',{name:'Aprovar plano',exact:true}).click();
  expect((await calls(page,'plan.approve'))[0].payload).toEqual({id:'network-plan',digest:'network-fixture-digest'});expect(await calls(page,'plan.run')).toEqual([]);
  await page.getByRole('button',{name:'Executar plano aprovado',exact:true}).click();
  await expect(page.locator('.detail-heading .status')).toHaveText('Parcial');
  await expect(page.locator('.execution-list').getByText('Observação TCP delimitada concluída; cobertura parcial.',{exact:true})).toBeVisible();
  expect(await calls(page,'plan.run')).toHaveLength(1);
});

test('TCP rejeita entradas inválidas localmente e preserva recusa de escopo do backend',async({page})=>{
  await networkFixture(page,'rejected');const ports=page.getByLabel('Portas TCP autorizadas',{exact:true});
  await ports.fill('1-65535');await expect(page.locator('.network-plan').getByRole('alert')).toContainText('sem intervalos');
  await expect(page.getByRole('button',{name:'Preparar plano TCP',exact:true})).toBeDisabled();expect(await calls(page,'assessment.networkPlan')).toEqual([]);
  await ports.fill('443');await page.getByRole('button',{name:'Preparar plano TCP',exact:true}).click();
  await expect(page.locator('.network-plan').getByRole('alert')).toContainText('Nenhuma conexão iniciada');
  expect(await calls(page,'assessment.networkPlan')).toHaveLength(1);expect(await calls(page,'plan.approve')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
});

test('TCP expirado exige descartar o plano antes de revisar novas portas',async({page})=>{
  await networkFixture(page,'expired');await page.getByLabel('Portas TCP autorizadas',{exact:true}).fill('443');
  await page.getByRole('button',{name:'Preparar plano TCP',exact:true}).click();
  await expect(page.getByRole('button',{name:'Aprovar plano',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Descartar plano expirado',exact:true}).click();
  await expect(page.getByLabel('Portas TCP autorizadas',{exact:true})).toBeVisible();
  expect((await calls(page,'plan.revoke'))[0].payload).toEqual({id:'network-plan',digest:'network-fixture-digest'});
  expect(await calls(page,'assessment.networkPlan')).toHaveLength(1);expect(await calls(page,'assessment.plan')).toEqual([]);expect(await calls(page,'plan.run')).toEqual([]);
});

test('prévia de rede não apresenta controle que inicie conexões TCP',async({page})=>{
  await page.goto('/');await page.evaluate(async()=>{
    const moduleUrl='/src/api.ts';const {request}=await import(moduleUrl);
    const project=await request('project.create',{name:'Rede de prévia',kind:'network',target:'192.0.2.1',description:'Dados isolados.'});
    await request('assessment.create',{projectId:project.id,title:'Rede sem executor',kind:project.kind,target:project.target,objective:'Verificar bloqueio na prévia.'});
  });await page.reload();await openAssessmentList(page);await page.getByText('Rede sem executor',{exact:true}).click();
  await expect(page.locator('.network-plan')).toContainText('A prévia não inicia conexões de rede.');
  await expect(page.getByRole('button',{name:'Preparar plano TCP',exact:true})).toHaveCount(0);
});
