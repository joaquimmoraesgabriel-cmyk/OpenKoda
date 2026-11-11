import { openAssessmentList } from './navigation';
import { chooseAiData, reviewKaliData } from './data-fixture';
import { installScopeFixture } from './scope-fixture';
import { Page } from '@playwright/test';
import { test, expect } from './fixtures';

// Browser IPC fixture only: no credentials, provider connection or paid inference.
async function fixture(page:Page,mode='unknown'){
  await installScopeFixture(page);
  await page.addInitScript(({mode})=>{
    const now=new Date().toISOString();const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__ledgerCalls:{action:string;payload:Record<string,unknown>}[]};w.isTauri=true;w.__ledgerCalls=[];
    const assessment={id:'assessment-ledger',projectId:'project-ledger',title:'Avaliação do histórico de IA',kind:'code',target:'D:/Fixture/source',objective:'Fixture de interface',status:'partial',createdAt:now,updatedAt:now,events:[],evidence:[],coverage:[],summary:''};
    const state={projects:[{id:assessment.projectId,name:'Projeto de contrato',kind:'code',target:assessment.target,description:'Fixture',createdAt:now}],assessments:[assessment],findings:[],settings:{provider:'openai',endpoint:'https://fixture.example.test/v1',model:'fixture-model',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:false},plans:[],executions:[],validations:[]};
    const scope=window.__scopeFixture(state,['ai.remote-review.v1']);
    const unknown={dataReceiptStatus:'legacy_no_data_policy_receipt',id:'unknown-request',model:'fixture-model',status:'unknown',createdAt:now,inputTokens:null,outputTokens:null,unknownAcknowledged:false,resolution:null as Record<string,unknown>|null};
    const known={id:'known-request',model:'fixture-model',status:'completed',createdAt:now,inputTokens:12,outputTokens:8,unknownAcknowledged:false,resolution:null};
    const requests:Record<string,unknown>[]=[known];if(mode!=='dispatch-error')requests.push(unknown);
    w.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');const {action,payload}=args.request;w.__ledgerCalls.push({action,payload});
      if(action==='ai.budget.status')return {enabled:false,degraded:false,revision:null,epochId:null,currency:'USD',policy:null,installation:{limitUsd:null,pricedUsageUsd:null,heldUsd:null,assumedChargeUsd:null,remainingUsd:null,unresolvedCount:0},assessment:null,legacyCostsUnknown:false,legacyReviewDigest:'fixture-legacy',knownLegacyRequests:0,unknownCharges:[],unknownChargesTotal:0,recentCharges:[],recentChargesTotal:0,notice:'Controle monetário desativado na fixture.'};
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture sem ambiente.',tools:[],executorReady:false};
      if(action==='ai.history')return {messages:[],requests:structuredClone(requests)};
      if(action==='ai.context')return {context:{objective:assessment.objective,target:assessment.target,evidence:[]},digest:'ledger-context',bytes:150,notice:'Contrato sem envio real.'};
      if(action==='ai.ask'){requests.push(unknown);throw new Error('Resposta não confirmada; consumo permanece desconhecido.');}
      if(action==='ai.request.reconcile'){
        if(mode==='refusal')throw new Error('Aguarde a inferência ativa antes de reconciliar o histórico.');
        if(payload.requestId!==unknown.id||payload.assessmentId!==assessment.id||payload.acknowledgeUnknownUsage!==true)throw new Error('Solicitação ou reconhecimento divergente.');
        if(unknown.resolution&&unknown.resolution.note!==payload.note)throw new Error('Justificativa divergente da preservada.');
        unknown.unknownAcknowledged=true;unknown.resolution??={id:'resolution-ledger',recordedAt:now,author:'local-user',note:payload.note,acknowledgeUnknownUsage:true,usageRemainsUnknown:true};return {id:unknown.id,assessmentId:assessment.id,status:'unknown',unknownAcknowledged:true,resolution:structuredClone(unknown.resolution),repeated:false};
      }
      throw new Error('Ação inesperada no contrato do histórico: '+action);
    }};
  },{mode});
  await page.goto('/');await openAssessmentList(page);await page.getByText('Avaliação do histórico de IA',{exact:true}).click();await page.locator('summary').filter({hasText:'Conversa com a IA'}).click();
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__ledgerCalls:{action:string;payload:Record<string,unknown>}[]}).__ledgerCalls.filter(call=>call.action===name),action);}
const acknowledgment='Reconheço que o resultado e o consumo desta solicitação continuam desconhecidos, inclusive a possível cobrança.';
async function openLedger(page:Page){await page.locator('.ai-request-ledger>summary').click();return page.locator('.ai-request-ledger');}

test('histórico mantém consumo desconhecido após reconhecimento explícito e preserva recibo',async({page})=>{
  await fixture(page);const ledger=await openLedger(page);const button=ledger.getByRole('button',{name:'Registrar reconhecimento da solicitação',exact:true});
  await expect(ledger.getByLabel('Solicitação para consultar',{exact:true})).toHaveValue('unknown-request');await expect(button).toBeDisabled();await expect(ledger).toContainText('valores não informados não são zero');await expect(ledger).toContainText('Registro legado sem recibo da política de dados');await expect(ledger.locator('.key-value').filter({hasText:'Tokens com entrada e saída informadas'})).toContainText('20');expect(await calls(page,'ai.request.reconcile')).toEqual([]);
  await ledger.getByLabel('Justificativa da solicitação interrompida',{exact:true}).fill('Revisei a interrupção; não tenho confirmação da resposta nem da cobrança.');await expect(button).toBeDisabled();await ledger.getByRole('checkbox',{name:acknowledgment}).check();await button.click();
  await expect(ledger).toContainText('Reconhecimento humano registrado');await expect(ledger.getByLabel('Solicitação para consultar',{exact:true})).toHaveValue('unknown-request');await expect(ledger.getByLabel('Detalhes da solicitação de IA',{exact:true})).toContainText('Resultado ou consumo desconhecido');await expect(ledger).toContainText('Revisei a interrupção; não tenho confirmação da resposta nem da cobrança.');await expect(button).toHaveCount(0);
  await ledger.getByRole('button',{name:'Atualizar solicitações da IA',exact:true}).click();expect(await calls(page,'ai.request.reconcile')).toHaveLength(1);expect(await calls(page,'ai.ask')).toEqual([]);await expect(ledger.getByLabel('Detalhes da solicitação de IA',{exact:true})).toContainText('Não informados');
  await ledger.screenshot({path:test.info().outputPath('historico-ia-consumo-desconhecido.png')});
});

test('revisão de solicitação respeita bytes e mantém recusa sem repetição automática',async({page})=>{
  await fixture(page,'refusal');const ledger=await openLedger(page);const note=ledger.getByLabel('Justificativa da solicitação interrompida',{exact:true});const button=ledger.getByRole('button',{name:'Registrar reconhecimento da solicitação',exact:true});
  await note.fill('é'.repeat(1001));await ledger.getByRole('checkbox',{name:acknowledgment}).check();await expect(button).toBeDisabled();expect(await calls(page,'ai.request.reconcile')).toEqual([]);
  await note.fill('A resposta e o consumo não foram confirmados.');await expect(ledger.getByRole('checkbox',{name:acknowledgment})).not.toBeChecked();await ledger.getByRole('checkbox',{name:acknowledgment}).check();await button.click();
  await expect(ledger.getByRole('alert')).toContainText('Aguarde a inferência ativa');await expect(button).toBeDisabled();await ledger.getByRole('button',{name:'Atualizar solicitações da IA',exact:true}).click();await expect(ledger.getByRole('button',{name:'Atualizar solicitações da IA',exact:true})).toBeEnabled();expect(await calls(page,'ai.request.reconcile')).toHaveLength(1);expect(await calls(page,'ai.ask')).toEqual([]);
});

test('erro no envio atualiza o histórico sem nova chamada e invalida consentimento anterior',async({page})=>{
  await fixture(page,'dispatch-error');const conversation=page.locator('.ai-conversation');await conversation.getByLabel('Sua mensagem',{exact:true}).fill('Revisar o contexto de contrato.');await chooseAiData(conversation);await conversation.getByRole('button',{name:'Revisar contexto de envio',exact:true}).click();await conversation.getByRole('checkbox',{name:'Autorizo o envio deste contexto e da mensagem ao provedor configurado.'}).check();await conversation.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
  await expect(conversation.getByRole('alert')).toContainText('consumo permanece desconhecido');await expect(conversation.getByRole('button',{name:'Enviar mensagem',exact:true})).toBeDisabled();await expect(conversation.getByRole('checkbox',{name:'Autorizo o envio deste contexto e da mensagem ao provedor configurado.'})).not.toBeChecked();
  const ledger=await openLedger(page);await expect(ledger.getByLabel('Solicitação para consultar',{exact:true})).toHaveValue('unknown-request');await expect(ledger).toContainText('Resultado ou consumo desconhecido');await ledger.getByRole('button',{name:'Atualizar solicitações da IA',exact:true}).click();await expect(ledger.getByRole('button',{name:'Atualizar solicitações da IA',exact:true})).toBeEnabled();expect(await calls(page,'ai.ask')).toHaveLength(1);expect(await calls(page,'ai.request.reconcile')).toEqual([]);
});
