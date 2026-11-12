import { openAssessmentList } from './navigation';
import { chooseAiData, reviewKaliData } from './data-fixture';
import { installScopeFixture } from './scope-fixture';
import { Locator, Page } from '@playwright/test';
import { test, expect } from './fixtures';

// Monetary values and provider replies below are synthetic, not official prices.
async function fixture(page:Page,mode='enabled'){
  await installScopeFixture(page);
  await page.addInitScript(({mode})=>{
    const now=new Date().toISOString();const w=window as unknown as {isTauri:boolean;__TAURI_INTERNALS__:unknown;__budgetCalls:{action:string;payload:Record<string,unknown>}[];__changeBudget:()=>void};w.isTauri=true;w.__budgetCalls=[];
    const assessment={id:'budget-assessment',projectId:'budget-project',title:'Avaliação com orçamento sintético',kind:'code',target:'D:/Fixture/source',objective:'Fixture de custo',status:'partial',createdAt:now,updatedAt:now,events:[],evidence:[],coverage:[],summary:''};
    const state={projects:[{id:assessment.projectId,name:'Projeto de orçamento',kind:'code',target:assessment.target,description:'Fixture',createdAt:now,archived:false}],assessments:[assessment],findings:[],settings:{provider:'openai',endpoint:'https://api.openai.com/v1',model:'fixture-model',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:10,outputLimitKiB:16,hasApiKey:true},plans:[],executions:[],validations:[]};
    const scope=window.__scopeFixture(state,['ai.remote-review.v1','source.rules.v1']);
    let enabled=!['disabled','configure'].includes(mode);let revision:string|null=mode==='configure'?null:'policy-1';let epoch:string|null=mode==='configure'?null:'epoch-1';let prices=[{model:'fixture-model',inputUsdPerMillion:'2.000000',outputUsdPerMillion:'8.000000',sourceNote:'Tarifas exclusivamente sintéticas.',revision:'price-1'}];let installationLimit='10.000000',assessmentLimit='2.000000';let unknown=mode==='unknown';let resolved=false;let sequence=0;let review:Record<string,unknown>|undefined;let investigation:Record<string,unknown>|undefined;const messages:Record<string,unknown>[]=[];
    w.__changeBudget=()=>{revision='policy-changed';window.dispatchEvent(new Event('openkoda-budget-changed'));};
    function status(payload:Record<string,unknown>){const balance={limitUsd:installationLimit,pricedUsageUsd:'0.250000',heldUsd:unknown?'0.030000':'0.000000',assumedChargeUsd:resolved?'0.040000':'0.000000',remainingUsd:resolved?'9.710000':unknown?'9.720000':'9.750000',unresolvedCount:unknown?1:0};return {enabled,degraded:mode==='degraded',revision,epochId:epoch,currency:'USD',policy:mode==='configure'&&!revision?null:{installationLimitUsd:installationLimit,assessmentLimitUsd:assessmentLimit,prices},installation:mode==='degraded'?null:balance,assessment:payload.assessmentId?{...balance,limitUsd:assessmentLimit,remainingUsd:'1.750000'}:null,legacyCostsUnknown:true,legacyReviewDigest:mode==='degraded'?null:'legacy-digest',knownLegacyRequests:3,unknownCharges:unknown?[{chargeId:'unknown-charge',heldUsd:'0.030000',model:'fixture-model',createdAt:now}]:[],unknownChargesTotal:mode==='degraded'?null:unknown?1:0,recentCharges:[{inputTokens:14,outputTokens:0,chargeId:'prior-charge',model:'fixture-model',createdAt:now,state:'priced',reservedUsd:'0.300000',pricedUsageUsd:'0.250000',assumedChargeUsd:null}],recentChargesTotal:1,notice:'Estimativa local por tarifa manual; não é fatura nem garantia de teto.'};}
    function accept(payload:Record<string,unknown>){if(enabled){const consent=payload.budgetReview as {id:string;digest:string}|undefined;if(!consent||consent.id!==review?.id||consent.digest!==review.digest)throw new Error('Revisão monetária ausente ou divergente.');}else if(payload.budgetReview)throw new Error('Payload legado deve permanecer sem revisão.');}
    w.__TAURI_INTERNALS__={invoke:async(command:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      if(command!=='request')throw new Error('Unexpected command');const {action,payload}=args.request;w.__budgetCalls.push({action,payload});
      if(action==='assessment.scopeStatus')return scope.status(payload.assessmentId);
      if(action==='snapshot')return structuredClone(state);
      if(action==='finding.httpStatus')return {supported:false,findingId:payload.findingId};
      if(action==='projectPurge.status')return {hasUnresolved:false,hasPartialHistory:false,journals:[],limits:[],confirmationPhrase:'EXCLUIR PROJETO E HISTÓRICO'};
      if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'Fixture',tools:[],executorReady:false};
      if(action==='ai.status')return {hasApiKey:true,verified:false,model:state.settings.model,endpoint:state.settings.endpoint};
      if(action==='ai.history')return {messages:structuredClone(messages),requests:[]};
      if(action==='ai.budget.history'){
        const day=(offset:number)=>{const date=new Date();date.setDate(date.getDate()+offset);return date.getFullYear()+'-'+String(date.getMonth()+1).padStart(2,'0')+'-'+String(date.getDate()).padStart(2,'0');};
        return {rows:[
          {day:day(0),provider:'openai',model:'fixture-model',assessmentId:assessment.id,requests:1,input:500,output:150,partial:0},
          {day:day(-1),provider:'groq',model:'fixture-model',assessmentId:null,requests:1,input:40,output:null,partial:1},
          {day:day(-38),provider:'openai',model:'fixture-model',assessmentId:assessment.id,requests:1,input:70,output:10,partial:0}
        ]};
      }
      if(action==='ai.context')return {context:{assessmentId:assessment.id,evidenceIds:payload.evidenceIds},digest:'context-budget',bytes:150,notice:'Contexto sintético.'};
      if(action==='ai.budget.status'){
        const result = status(payload);
        if(mode==='analytics'){
          const [charge] = result.recentCharges;
          const records = [
            {...charge,endpoint:'https://api.openai.com/v1'},
            {...charge,endpoint:'https://api.groq.com/openai/v1',chargeId:'groq-charge',inputTokens:9,outputTokens:4},
          ];
          return {...result,recentCharges:records,recentChargesTotal:records.length};
        }
        return result;
      }
      if(action==='ai.budget.preview'){
        if(mode==='missing-price')throw new Error('Modelo sem tarifa manual revisada; configure o orçamento.');if(mode==='insufficient')throw new Error('A reserva estimada excede o orçamento restante. Nenhum envio foi feito.');if(unknown)throw new Error('Há cobrança desconhecida na instalação.');
        const input=payload.payload as Record<string,unknown>;if(input.allowRemote||input.budgetReview)throw new Error('Prévia não aceita consentimento de envio.');if(payload.operation==='investigation'&&(JSON.stringify(Object.keys(input).sort())!==JSON.stringify(['digest','id'])||input.id!=='investigation'||input.digest!=='investigation-digest'))throw new Error('Contrato estrito do orçamento de investigação exige somente id e digest.');
        review={id:'budget-review-'+(++sequence),digest:'budget-digest-'+sequence,createdAt:now,expiresAt:new Date(Date.now()+(mode==='expired'?-1000:600000)).toISOString(),epochId:epoch,policyRevision:revision,model:'fixture-model',endpoint:'https://api.openai.com/v1',priceRevision:'price-1',rates:prices[0],estimatedInputTokens:payload.operation==='investigation'?8192:4096,maxOutputTokens:payload.operation==='investigation'?512:2048,requests:payload.operation==='investigation'?2:1,reservedUsd:'0.020000',installationRemainingUsd:'9.750000',assessmentRemainingUsd:payload.operation==='test'?null:'1.750000',contextDigest:payload.operation==='test'?null:'context-budget',notice:'Reserva sintética. Não é fatura nem garantia de teto de cobrança.'};return {enabled,review:structuredClone(review),notice:'Fixture'};
      }
      if(action==='ai.budget.configure'){if(payload.expectedRevision!==revision||payload.legacyReviewDigest!=='legacy-digest'||payload.acknowledgeEstimates!==true||payload.acknowledgeLegacyExcluded!==true)throw new Error('Política sem revisão.');if(enabled&&payload.enabled===false&&payload.acknowledgeUnbounded!==true)throw new Error('Desativação sem confirmação.');enabled=payload.enabled as boolean;revision='policy-saved';epoch??='epoch-1';installationLimit=payload.installationLimitUsd as string;assessmentLimit=payload.assessmentLimitUsd as string;prices=(payload.prices as typeof prices).map(p=>({...p,revision:'price-saved'}));return status({});}
      if(action==='ai.budget.resolveUnknown'){if(payload.chargeId!=='unknown-charge'||payload.acknowledgeUnknownCharge!==true||payload.assumedChargeUsd!=='0.040000')throw new Error('Débito conservador divergente.');unknown=false;resolved=true;return {usageRemainsUnknown:true};}
      if(action==='ai.ask'){accept(payload);if(mode==='dispatch-error'){unknown=true;throw new Error('Resposta não confirmada. Reserva preservada; consumo desconhecido.');}messages.push({id:'reply',role:'assistant',text:'Resposta sintética sem API.',createdAt:now});return {text:'Resposta sintética sem API.',toolsExecuted:false};}
      if(action==='ai.test'){accept(payload);return {verified:true,capability:'text'};}
      if(action==='ai.operation.propose'){accept(payload);return {id:'proposal',requestId:'request',responseId:'response',assessmentId:assessment.id,contextDigest:'context-budget',adapter:'source.rules.v1',rationale:'Sugestão sintética.',question:null,model:'fixture-model',endpoint:'https://api.openai.com/v1',usage:{input_tokens:1,output_tokens:1},toolsExecuted:false,requiresHumanReview:true,availabilityVerified:false,notice:'Sem execução.'};}
      if(action==='ai.investigation.list')return {items:investigation?[structuredClone(investigation)]:[],limit:500};
      if(action==='ai.investigation.plan'){investigation={scopeRef:scope.ref,id:'investigation',digest:'investigation-digest',status:'prepared',assessmentId:assessment.id,evidenceIds:payload.evidenceIds,contextDigest:payload.contextDigest,context:{literal:'Fixture'},message:payload.message,perspectives:payload.perspectives,endpoint:'https://api.openai.com/v1',model:'fixture-model',credentialRevision:'fixture',createdAt:now,expiresAt:new Date(Date.now()+600000).toISOString(),budget:{maxRequests:2,maxOutputTokensPerRequest:512,maxTotalTokens:10000,conservativeReservationPerRequest:4096,maxContextBytes:65536},results:[],outcomeNote:'',reconciledFrom:null,knownTokens:0,usageUnknown:false,toolsExecuted:false,maxConcurrentRequests:1,notice:'Perspectivas independentes.'};return structuredClone(investigation);}
      if(action==='ai.investigation.run'){accept(payload);if(mode==='run-refused')throw new Error('A reserva mudou antes do provedor. Nenhum envio foi feito.');investigation!.status='completed';investigation!.results=[];return structuredClone(investigation);}
      throw new Error('Ação não prevista no orçamento: '+action);
    }};
  },{mode});await page.goto('/');
}
async function calls(page:Page,action:string){return page.evaluate(name=>(window as unknown as {__budgetCalls:{action:string;payload:Record<string,unknown>}[]}).__budgetCalls.filter(call=>call.action===name),action);}
const budgetAck='Revisei a reserva estimada em USD desta solicitação e seus limites locais.';
test('atalho do compositor revisa contexto e exige autorização para enviar', async({page}) => {
  await fixture(page,'disabled');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByLabel('Atalho de envio').selectOption('enter');
  await openAssessmentList(page);
  await page.getByText('Avaliação com orçamento sintético',{exact:true}).click();
  await page.locator('summary').filter({hasText:'Conversa com a IA'}).click();
  const panel = page.locator('.ai-conversation');
  const message = panel.getByLabel('Sua mensagem',{exact:true});
  const consent = panel.getByRole('checkbox',{name:'Autorizo o envio deste contexto e da mensagem ao provedor configurado.',exact:true});
  await message.fill('Revisar o objetivo');
  await chooseAiData(panel);
  await message.press('End');
  await message.press('Shift+Enter');
  await expect(message).toHaveValue('Revisar o objetivo\n');
  await message.press('Enter');
  await expect(consent).toBeVisible();
  await message.press('Enter');
  expect(await calls(page,'ai.ask')).toHaveLength(0);
  await consent.check();
  await message.press('Enter');
  await expect.poll(async() => (await calls(page,'ai.ask')).length).toBe(1);
});
async function flow(page:Page,operation='ask'):Promise<{panel:Locator;send:Locator;action:string}>{
  if(operation==='test'){await page.getByRole('button',{name:'Configurações',exact:true}).click();await page.getByRole('button',{name:'IA e modelos',exact:true}).click();const panel=page.locator('.settings-content .content-section').filter({has:page.getByRole('heading',{name:'Conexão',exact:true})});return {panel,send:panel.getByRole('button',{name:'Testar conexão',exact:true}),action:'ai.test'};}
  await openAssessmentList(page);
  await page.getByText('Avaliação com orçamento sintético',{exact:true}).click();
  if(operation==='ask'){await page.locator('summary').filter({hasText:'Conversa com a IA'}).click();const panel=page.locator('.ai-conversation');await panel.getByLabel('Sua mensagem',{exact:true}).fill('Analise somente o contexto sintético.');await chooseAiData(panel);await panel.getByRole('button',{name:'Revisar contexto de envio',exact:true}).click();await panel.getByRole('checkbox',{name:'Autorizo o envio deste contexto e da mensagem ao provedor configurado.',exact:true}).check();return {panel,send:panel.getByRole('button',{name:'Enviar mensagem',exact:true}),action:'ai.ask'};}
  if(operation==='operation_proposal'){await page.locator('.ai-operation-proposal>summary').click();const panel=page.locator('.operation-proposal-panel');await panel.getByRole('checkbox',{name:'Análise local de padrões textuais',exact:true}).check();await panel.getByLabel('Objetivo da sugestão',{exact:true}).fill('Sugira uma observação limitada.');await chooseAiData(panel);await panel.getByRole('button',{name:'Revisar contexto da sugestão',exact:true}).click();await panel.getByRole('checkbox',{name:'Autorizo enviar este contexto, objetivo e opções ao modelo e provedor exibidos para receber uma sugestão sem execução.',exact:true}).check();return {panel,send:panel.getByRole('button',{name:'Solicitar sugestão revisada',exact:true}),action:'ai.operation.propose'};}
  await page.locator('summary').filter({hasText:'Perspectivas independentes de IA'}).click();const panel=page.locator('.ai-investigation');await panel.getByLabel('Objetivo da investigação',{exact:true}).fill('Compare interpretações sintéticas.');await panel.getByRole('checkbox',{name:'Contraevidências e alternativas',exact:true}).check();await chooseAiData(panel);await panel.getByRole('button',{name:'Revisar contexto da investigação',exact:true}).click();await panel.getByRole('button',{name:'Preparar investigação para revisão',exact:true}).click();await panel.getByRole('checkbox',{name:'Autorizo o envio deste contexto e objetivo ao modelo e provedor mostrados, nas perspectivas e limites deste plano.',exact:true}).check();return {panel,send:panel.getByRole('button',{name:'Enviar investigação revisada',exact:true}),action:'ai.investigation.run'};
}
async function reviewCost(panel:Locator){await panel.getByRole('button',{name:'Revisar estimativa de custo',exact:true}).click();await panel.getByRole('checkbox',{name:budgetAck,exact:true}).check();}
async function settings(page:Page, section: 'Visão geral'|'Análises'|'Política de orçamento' = 'Política de orçamento') {
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Uso e orçamento',exact:true}).click();
  const panel = page.locator('.ai-budget-settings');
  await panel.getByRole('button',{name:section,exact:true}).click();
  return panel;
}

for(const operation of ['ask','test','operation_proposal','investigation'])test(`${operation}: prévia local de custo e confirmação específica antecedem a chamada paga`, { tag: operation === 'test' ? '@source-edition' : '@legacy-engine' }, async ({page})=>{
  await fixture(page);const {panel,send,action}=await flow(page,operation);await expect(send).toBeDisabled();expect(await calls(page,action)).toEqual([]);await panel.getByRole('button',{name:'Revisar estimativa de custo',exact:true}).click();await expect(panel.getByLabel('Estimativa de custo revisada',{exact:true})).toContainText('USD 0.020000');await expect(panel.getByLabel('Estimativa de custo revisada',{exact:true})).toContainText('Não é fatura');await expect(send).toBeDisabled();expect(await calls(page,action)).toEqual([]);const input=(await calls(page,'ai.budget.preview'))[0].payload;expect(input.operation).toBe(operation);expect(input.payload).not.toHaveProperty('allowRemote');expect(input.payload).not.toHaveProperty('budgetReview');if(operation==='investigation'){expect(input.payload).toEqual({id:'investigation',digest:'investigation-digest'});await expect(panel.getByLabel('Estimativa de custo revisada',{exact:true})).toContainText('512 de saída · 2 chamada(s)');}await panel.getByRole('checkbox',{name:budgetAck,exact:true}).check();if(operation==='ask')await panel.locator('.ai-budget-review').screenshot({path:test.info().outputPath('revisao-orcamento-ia.png')});await send.click();await expect.poll(async()=>(await calls(page,action)).length).toBe(1);expect((await calls(page,action))[0].payload.budgetReview).toEqual({id:'budget-review-1',digest:'budget-digest-1'});if(operation==='investigation'){const plan=(await calls(page,'ai.investigation.plan'))[0].payload;const run=(await calls(page,action))[0].payload;expect(run.dataReview).toEqual(plan.dataReview);expect(run).toHaveProperty('acknowledgeSensitive',false);}
});

test('mudança de mensagem ou política invalida autorização monetária sem novo envio',async({page})=>{
  await fixture(page);const {panel,send}=await flow(page);await reviewCost(panel);await expect(send).toBeEnabled();await panel.getByLabel('Sua mensagem',{exact:true}).fill('Mensagem alterada.');await expect(send).toBeDisabled();await expect(panel.getByLabel('Estimativa de custo revisada',{exact:true})).toHaveCount(0);await panel.getByRole('button',{name:'Revisar contexto de envio',exact:true}).click();await panel.getByRole('checkbox',{name:'Autorizo o envio deste contexto e da mensagem ao provedor configurado.',exact:true}).check();await reviewCost(panel);await page.evaluate(()=>(window as unknown as {__changeBudget:()=>void}).__changeBudget());await expect(send).toBeDisabled();await expect(panel.getByLabel('Estimativa de custo revisada',{exact:true})).toHaveCount(0);expect(await calls(page,'ai.ask')).toEqual([]);
});

for(const mode of ['missing-price','insufficient','expired'])test(`revisão ${mode} bloqueia envio e não amplia teto ou tarifa`,async({page})=>{
  await fixture(page,mode);const {panel,send}=await flow(page);await panel.getByRole('button',{name:'Revisar estimativa de custo',exact:true}).click();await expect(send).toBeDisabled();if(mode==='expired'){await expect(panel.getByLabel('Estimativa de custo revisada',{exact:true})).toContainText('Expirada');await expect(panel.getByRole('checkbox',{name:budgetAck,exact:true})).toBeDisabled();}else await expect(panel.getByRole('alert')).toContainText(mode==='missing-price'?'Modelo sem tarifa':'excede o orçamento');expect(await calls(page,'ai.ask')).toEqual([]);expect(await calls(page,'ai.budget.configure')).toEqual([]);expect(await calls(page,'ai.budget.preview')).toHaveLength(1);
});

test('registro indisponível com saldos null falha fechado e não presume zero', { tag: '@source-edition' }, async ({page})=>{
  await fixture(page,'degraded');const {panel,send}=await flow(page,'test');await expect(panel.locator('.ai-budget-review')).toContainText('registro de orçamento requer atenção');await expect(send).toBeDisabled();const config=await settings(page,'Visão geral');await expect(config).toContainText('Não informado');await config.getByRole('button',{name:'Política de orçamento',exact:true}).click();await expect(config.getByRole('button',{name:'Revisar política de orçamento',exact:true})).toBeDisabled();expect(await calls(page,'ai.ask')).toEqual([]);
});

test('configuração começa sem preços ou tetos e exige revisão de estimativa e legado', { tag: '@source-edition' }, async ({page})=>{
  await fixture(page,'configure');const panel=await settings(page);await expect(panel.getByLabel('Limite da instalação em USD',{exact:true})).toHaveValue('');await expect(panel.getByLabel('Limite por avaliação em USD',{exact:true})).toHaveValue('');await expect(panel.getByLabel('Modelo do preço 1',{exact:true})).toHaveCount(0);await panel.getByRole('checkbox',{name:'Ativar controle monetário local para próximas chamadas.',exact:true}).check();await expect(panel.getByRole('button',{name:'Revisar política de orçamento',exact:true})).toBeDisabled();await panel.getByLabel('Limite da instalação em USD',{exact:true}).fill('10.000000');await panel.getByLabel('Limite por avaliação em USD',{exact:true}).fill('2.000000');await panel.getByRole('button',{name:'Adicionar preço de modelo',exact:true}).click();await panel.getByLabel('Modelo do preço 1',{exact:true}).fill('fixture-model');await panel.getByLabel('Entrada por milhão em USD 1',{exact:true}).fill('0');await panel.getByLabel('Saída por milhão em USD 1',{exact:true}).fill('8.000000');await panel.getByLabel('Origem e data do preço 1',{exact:true}).fill('Fonte manual sintética, sem preço oficial.');await expect(panel.getByRole('button',{name:'Revisar política de orçamento',exact:true})).toBeDisabled();await panel.getByLabel('Entrada por milhão em USD 1',{exact:true}).fill('2.000000');await panel.getByRole('button',{name:'Revisar política de orçamento',exact:true}).click();await expect(panel.getByRole('button',{name:'Salvar política revisada',exact:true})).toBeDisabled();await panel.getByRole('checkbox',{name:'Entendo que estes preços são manuais e as estimativas locais não são fatura nem garantia de cobrança máxima.',exact:true}).check();await panel.getByRole('checkbox',{name:'Revisei o legado e reconheço que custos anteriores sem preço não entram no saldo deste período.',exact:true}).check();await panel.getByRole('button',{name:'Salvar política revisada',exact:true}).click();await expect(panel.getByRole('status')).toContainText('Custos e reservas anteriores foram preservados');expect((await calls(page,'ai.budget.configure'))[0].payload).toMatchObject({expectedRevision:null,enabled:true,currency:'USD',installationLimitUsd:'10.000000',assessmentLimitUsd:'2.000000',acknowledgeEstimates:true,acknowledgeLegacyExcluded:true,legacyReviewDigest:'legacy-digest',acknowledgeUnbounded:false});expect(await calls(page,'ai.test')).toEqual([]);
});

test('custo desconhecido exige débito conservador maior que reserva e não vira fatura confirmada', { tag: '@source-edition' }, async ({page})=>{
  await fixture(page,'unknown');const panel=await settings(page);await panel.getByText('Custos desconhecidos · 1',{exact:true}).click();await panel.getByLabel('Custo desconhecido para revisar',{exact:true}).selectOption('unknown-charge');await panel.getByLabel('Débito conservador assumido em USD',{exact:true}).fill('0.010000');await panel.getByLabel('Justificativa do custo desconhecido',{exact:true}).fill('Reserva revisada sem comprovação da fatura.');await panel.getByRole('checkbox',{name:'Reconheço a cobrança desconhecida e autorizo debitar esta estimativa conservadora no controle local, sem afirmar o valor da fatura.',exact:true}).check();await expect(panel.getByRole('button',{name:'Registrar débito conservador',exact:true})).toBeDisabled();await panel.getByLabel('Débito conservador assumido em USD',{exact:true}).fill('0.040000');await expect(panel.getByRole('checkbox',{name:'Reconheço a cobrança desconhecida e autorizo debitar esta estimativa conservadora no controle local, sem afirmar o valor da fatura.',exact:true})).not.toBeChecked();await panel.getByRole('checkbox',{name:'Reconheço a cobrança desconhecida e autorizo debitar esta estimativa conservadora no controle local, sem afirmar o valor da fatura.',exact:true}).check();await panel.getByRole('button',{name:'Registrar débito conservador',exact:true}).click();await expect(panel.getByRole('status')).toContainText('Uso e cobrança reais continuam desconhecidos');expect((await calls(page,'ai.budget.resolveUnknown'))[0].payload).toMatchObject({chargeId:'unknown-charge',assumedChargeUsd:'0.040000',acknowledgeUnknownCharge:true});expect(await calls(page,'ai.request.reconcile')).toEqual([]);expect(await calls(page,'ai.ask')).toEqual([]);
});

test('desativar política exige confirmação adicional e não reinicia débitos', { tag: '@source-edition' }, async ({page})=>{
  await fixture(page);const panel=await settings(page);await panel.getByRole('checkbox',{name:'Ativar controle monetário local para próximas chamadas.',exact:true}).uncheck();await panel.getByRole('button',{name:'Revisar política de orçamento',exact:true}).click();await panel.getByRole('checkbox',{name:'Entendo que estes preços são manuais e as estimativas locais não são fatura nem garantia de cobrança máxima.',exact:true}).check();await panel.getByRole('checkbox',{name:'Revisei o legado e reconheço que custos anteriores sem preço não entram no saldo deste período.',exact:true}).check();await expect(panel.getByRole('button',{name:'Salvar política revisada',exact:true})).toBeDisabled();await panel.getByRole('checkbox',{name:'Autorizo desativar o controle monetário para novas chamadas, mantendo débitos e reservas já registrados.',exact:true}).check();await panel.getByRole('button',{name:'Salvar política revisada',exact:true}).click();await expect(panel.getByRole('status')).toContainText('preservados');await panel.getByRole('button',{name:'Visão geral',exact:true}).click();await expect(panel).toContainText('USD 0,25');expect((await calls(page,'ai.budget.configure'))[0].payload.acknowledgeUnbounded).toBe(true);
});

test('política desativada preserva contrato legado sem inserir preço ou revisão fictícia',async({page})=>{
  await fixture(page,'disabled');const {panel,send}=await flow(page);await expect(panel.locator('.ai-budget-review')).toContainText('Controle monetário local desativado');await expect(send).toBeEnabled();await send.click();await expect.poll(async()=>(await calls(page,'ai.ask')).length).toBe(1);expect((await calls(page,'ai.ask'))[0].payload).not.toHaveProperty('budgetReview');expect(await calls(page,'ai.budget.preview')).toEqual([]);
});

test('falha depois do envio preserva reserva e exige revisão sem repetir solicitação',async({page})=>{
  await fixture(page,'dispatch-error');const {panel,send}=await flow(page);await reviewCost(panel);await send.click();await expect(panel).toContainText('Reserva preservada; consumo desconhecido');await expect(send).toBeDisabled();await panel.getByRole('button',{name:'Revisar estimativa de custo',exact:true}).click();await expect(panel.locator('.ai-budget-review').getByRole('alert')).toContainText('cobrança desconhecida');expect(await calls(page,'ai.ask')).toHaveLength(1);
});

test('investigação recusada antes do provedor informa tentativa sem declarar envio',async({page})=>{
  await fixture(page,'run-refused');const {panel,send}=await flow(page,'investigation');await reviewCost(panel);await send.click();await expect(panel).toContainText('Nenhum envio foi feito.');await expect(panel).toContainText('Houve uma tentativa de envio deste plano.');await expect(panel).not.toContainText('Esta solicitação já foi enviada.');await expect(panel.getByRole('button',{name:'Enviar investigação revisada',exact:true})).toHaveCount(0);await panel.getByRole('button',{name:'Atualizar investigações',exact:true}).click();expect(await calls(page,'ai.investigation.run')).toHaveLength(1);await expect(panel).toContainText('não haverá repetição automática');
});

test('análises separam tokens informados, saldos e detalhes da chamada', { tag: '@source-edition' }, async ({page})=>{
  await fixture(page,'analytics');
  const panel = await settings(page,'Visão geral');
  await expect(panel.getByRole('progressbar',{name:'Percentual comprometido do limite'})).toHaveAttribute('aria-valuetext','2,5% comprometido');
  await expect(panel.getByText('USD 9,75',{exact:true})).toBeVisible();
  await panel.getByRole('button',{name:'Ver análises'}).click();
  const models = panel.getByRole('combobox',{name:'Modelo no histórico'});
  await expect(panel.locator('.chart-totals')).toHaveText('Entrada23Saída4');
  await models.selectOption({label:'Groq · fixture-model'});
  await expect(panel.locator('.chart-totals')).toHaveText('Entrada9Saída4');
  await models.selectOption({label:'OpenAI · fixture-model'});
  await expect(panel.getByRole('list',{name:'Tokens informados por dia'}).getByRole('listitem')).toHaveCount(7);
  await expect(panel.locator('.chart-totals')).toHaveText('Entrada14Saída0');
  await panel.getByRole('button',{name:'30 dias'}).click();
  await expect(panel.getByRole('list',{name:'Tokens informados por dia'}).getByRole('listitem')).toHaveCount(30);
  const row = panel.getByRole('row').filter({hasText:'fixture-model'});
  await expect(row).toContainText('Estimado');
  await row.locator('summary').click();
  await expect(row.getByText('prior-charge',{exact:true})).toBeVisible();
  await models.selectOption({label:'Todos os modelos'});
  await expect(panel.getByRole('row').filter({hasText:'fixture-model'})).toHaveCount(2);
  expect(await calls(page,'ai.ask')).toEqual([]);
});

test('histórico do perfil inclui datas antigas e permite filtrar pelo calendário e abrir a avaliação', { tag: '@source-edition' }, async ({page})=>{
  await fixture(page,'analytics');
  const budget=await settings(page,'Visão geral');
  await budget.getByRole('button',{name:'Abrir histórico de tokens'}).click();
  const history=page.getByRole('region',{name:'Conteúdo de Histórico de uso'});
  const total=history.locator('.uh-stats > div').filter({hasText:'Tokens no período'}).locator('dd');
  await expect(total).toHaveText('770');
  await history.getByRole('combobox',{name:'Período',exact:true}).selectOption('7d');
  await expect(total).toHaveText('690');
  await history.getByRole('combobox',{name:'Modelo',exact:true}).selectOption({label:'Groq · fixture-model'});
  await expect(total).toHaveText('40');
  await expect(history.getByText('1 chamada com contagem de tokens incompleta no período.')).toBeVisible();
  await history.getByRole('combobox',{name:'Modelo',exact:true}).selectOption('');
  const today=history.locator('.uh-calendar button').last();
  await today.focus();await page.keyboard.press('ArrowUp');await page.keyboard.press('Enter');
  await expect(total).toHaveText('40');
  await history.getByRole('combobox',{name:'Período',exact:true}).selectOption('all');
  await page.screenshot({path:test.info().outputPath('historico-desktop.png')});
  await page.setViewportSize({width:512,height:480});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1)).toBe(true);
  await history.getByRole('button',{name:'Avaliação com orçamento sintético'}).click();
  await expect(page.getByRole('heading',{name:'Avaliação com orçamento sintético',exact:true})).toBeVisible();
  expect(await calls(page,'ai.ask')).toHaveLength(0);
});

test('tarifas de modelos homônimos são independentes por provedor', { tag: '@source-edition' }, async ({page})=>{
  await fixture(page);
  const panel=await settings(page);
  await panel.getByRole('button',{name:'Adicionar preço de modelo'}).click();
  await panel.getByLabel('Modelo do preço 2',{exact:true}).fill('fixture-model');
  await panel.getByLabel('Entrada por milhão em USD 2').fill('1');
  await panel.getByLabel('Saída por milhão em USD 2').fill('2');
  await panel.getByLabel('Origem e data do preço 2').fill('Tarifa de teste.');
  const review=panel.getByRole('button',{name:'Revisar política de orçamento'});
  await expect(review).toBeDisabled();
  await panel.getByLabel('Provedor do preço 2').selectOption('groq');
  await panel.getByLabel('Modelo do preço 2',{exact:true}).fill('fixture-model');
  await panel.getByLabel('Entrada por milhão em USD 2').fill('1');
  await panel.getByLabel('Saída por milhão em USD 2').fill('2');
  await panel.getByLabel('Origem e data do preço 2').fill('Tarifa Groq sintética.');
  await review.click();
  const confirmation=panel.getByRole('region',{name:'Revisão da política de orçamento'});
  await expect(confirmation).toContainText('openai · fixture-model');
  await expect(confirmation).toContainText('groq · fixture-model');
  await confirmation.getByRole('checkbox').first().check();
  await confirmation.getByRole('checkbox').nth(1).check();
  await confirmation.getByRole('button',{name:'Salvar política revisada'}).click();
  await expect(panel.getByRole('status')).toContainText('Política de orçamento salva');
  const [save]=await calls(page,'ai.budget.configure');
  expect(save.payload.prices).toMatchObject([{model:'fixture-model'},{provider:'groq',model:'fixture-model'}]);
});
