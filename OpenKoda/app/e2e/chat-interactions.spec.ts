import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import { emptySnapshot } from '../src/preview';
import { installScopeFixture } from './scope-fixture';

type Turn = { id: string; role: string; text: string; createdAt: string; outputLimited?:boolean; outputLimitTokens?:number };
type Bridge = {
  turns: Turn[];
  calls: { action: string; payload: Record<string, unknown> }[];
  running: boolean;
  activeAssessment?:string;
  processingMs?:number;
  queuePosition?:number;
  partialText?:string;
  scanAdmitted?:boolean;
  sensitiveSource?:boolean;
  activeReview?: {id:string;digest:string};
  acceptedMessage?: string;
  rejectBeforeSend?: boolean;
  pretendOtherReview?: boolean;
  failImmediatelyAfterSend?: boolean;
};
declare global { interface Window { __conversationTest: Bridge } }

async function installChat(page: Page, turns: Turn[] = [], running = false) {
  await installScopeFixture(page);
  const snapshot = emptySnapshot();
  snapshot.settings = { ...snapshot.settings, provider: 'openai', model: 'modelo-original', endpoint: 'https://api.openai.com/v1', hasApiKey: true };
  const date = '2026-01-01T12:00:00Z';
  snapshot.projects = [{ id: 'fixture', name: 'Laboratório', kind: 'code', target: 'D:/Fixture/source', description: '', createdAt: date }];
  snapshot.assessments = ['A', 'B'].map(letter => ({
    id: letter, projectId: 'fixture', title: 'Conversa ' + letter, kind: 'code', target: 'D:/Fixture/source',
    objective: '', status: 'completed', events: [
      { id: 'e1', message: 'Alvo resolvido', kind: 'info', at: date },
      { id: 'e2', message: 'Leitura concluída', kind: 'info', at: date },
    ],
    evidence: [], coverage: [], summary: '', createdAt: date, updatedAt: date,
    aiSelection: { provider: 'openai', model: 'modelo-original' },
  }));
  await page.addInitScript(({ snapshot, turns, running }) => {
    const scope = window.__scopeFixture(snapshot, ['source.rules.v1', 'ai.remote-review.v1']);
    const bridge: Bridge = { turns, running, calls: [] };
    window.__conversationTest = bridge;
    Object.assign(window, { isTauri: true, __TAURI_INTERNALS__: { invoke: async (_cmd: string, args: { request?: Bridge['calls'][number] }) => {
      if (!args.request) return {};
      const { action, payload } = args.request;
      bridge.calls.push(args.request);
      if (action === 'project.newConversation') {
        const already=snapshot.assessments.find(a=>a.id==='new-thread');
        if(already)return already;
        const item={...snapshot.assessments[0],id:'new-thread',title:String(payload.title),objective:String(payload.analysisObjective??''),status:'draft' as const,
          events:payload.analysisObjective?[{id:'first-scan',kind:'chat_scan_first_turn',at:snapshot.projects[0].createdAt,message:'Preparação da análise.'}]:[]};
        snapshot.assessments.push(item);
        scope.statuses[item.id]={...scope.statuses.A,assessmentId:item.id,revision:{...scope.statuses.A.revision,assessmentId:item.id}};
        return item;
      }
      if (action === 'snapshot') return {...snapshot,assessments:snapshot.assessments.filter(a=>a.id!=='new-thread'||bridge.scanAdmitted||bridge.activeAssessment===a.id&&bridge.acceptedMessage)};
      if(action==='project.options.get')return {options:{advancedEnabled:true,additionalContextEnabled:false,context:['','',''],minutes:null,maxCostUsd:0.03,title:''}};
      if(action==='security.status')return {available:true,provider:'openai',model:'modelo-original',hasApiKey:true,requiresSensitiveConfirmation:!!bridge.sensitiveSource};
      if(action==='security.scan.start'||action==='security.scan.get'){
        const id=String(payload.assessmentId);
        if(action==='security.scan.start')bridge.scanAdmitted=true;
        if(!bridge.sensitiveSource||id==='new-thread'&&!bridge.scanAdmitted)return {scan:null};
        const at='2026-11-12T12:00:00Z';
        return {scan:{id:'scan-'+id,assessmentId:id,selectedPaths:['src/api.rs'],model:'modelo-original',provider:'openai',createdAt:at,updatedAt:at,
          status:'completed',stage:'completed',maxCostUsd:0.03,message:'Análise concluída.',
          events:[{stage:'source_query',message:'Leitura de src/api.rs',at}],error:null,
          result:{report:'Revisão da API.',scanId:null,findings:[],coverage:{scope:['src/api.rs'],excludedPaths:[],additionalExclusions:0,findingsTotal:0,reportTruncated:false},cost:null}}};
      }
      if (action === 'assessment.scopeStatus') return scope.status(payload.assessmentId);
      if (action === 'ai.context') return { digest: 'packet-a', bytes: 120, context: { assessmentId: payload.assessmentId } };
      if (action === 'ai.budget.status') return { enabled: false, degraded: false };
      if (action === 'ai.ask') {
        if(bridge.rejectBeforeSend)throw new Error('O escopo foi revogado antes do envio.');
        bridge.activeAssessment=String(payload.assessmentId);
        if(bridge.activeAssessment==='new-thread')snapshot.assessments.find(a=>a.id==='new-thread')!.title=String(payload.message);
        bridge.running = true;
        bridge.activeReview = bridge.pretendOtherReview?{id:'other-window-review',digest:'unrelated-digest'}:payload.dataReview as {id:string;digest:string};
        bridge.acceptedMessage = 'message-' + bridge.calls.filter(c=>c.action==='ai.ask').length;
        bridge.turns.push({id:bridge.acceptedMessage,role:'user',text:String(payload.message),createdAt:'2026-11-12T12:00:00Z'});
        if(bridge.failImmediatelyAfterSend){bridge.running=false;throw new Error('O provedor recusou a solicitação após receber o envio (429).');}
        try{await new Promise((resolve,reject)=>{
          window.addEventListener('fixture:reply', resolve, { once: true });
          window.addEventListener('fixture:uncertain',()=>reject(new Error('Resposta interrompida. O resultado remoto é incerto.')), { once: true });
        });}finally{bridge.running = false;}
        return {};
      }
      if (action === 'environment.status') return { tools: [], status: 'unavailable', message: 'Teste' };
      if (action === 'project.renameConversation') {
        const chat=snapshot.assessments.find(a=>a.id===payload.assessmentId)!;
        chat.title=String(payload.title);
        return {id:chat.id};
      }
      if (action === 'ai.history') return { messages: payload.assessmentId === (bridge.activeAssessment??'A') ? bridge.turns : [], requests:payload.assessmentId===(bridge.activeAssessment??'A')&&bridge.acceptedMessage?[{id:'turn-A',dataReviewRef:bridge.activeReview,chatMessageIds:[bridge.acceptedMessage]}]:[] };
      if (action === 'ai.chat.status') return { requestId: bridge.running && payload.assessmentId === (bridge.activeAssessment??'A') ? 'turn-A' : null, queuePosition:bridge.queuePosition,processingMs:bridge.processingMs,partialText:bridge.partialText, stopping: false, messageId:bridge.running?bridge.acceptedMessage:null,dataReview:bridge.running?bridge.activeReview:null };
      if (action === 'ai.chat.stop') bridge.running = false;
      if (action === 'ai.status') return {
        provider: payload.provider, hasApiKey: ['openai', 'openrouter'].includes(String(payload.provider)),
        enabledModels: payload.provider === 'openai' ? ['modelo-original'] : ['lab/rapido', 'lab/outro'],
      };
      if (action === 'ai.selection.save') {
        const item = snapshot.assessments.find(a => a.id === payload.assessmentId);
        if (item) item.aiSelection = payload.aiSelection as typeof item.aiSelection;
      }
      return {};
    } } });
  }, { snapshot, turns, running });
  await page.goto('/');
}

test('continuar depois do limite prepara um rascunho sem enviar nem cobrar', async ({page})=>{
  await installChat(page,[{id:'partial',role:'assistant',createdAt:'2026-11-12T12:00:00Z',text:'Fragmento recebido.',outputLimited:true,outputLimitTokens:32}]);
  await expect(page.getByText('A resposta atingiu o limite de saída (32 tokens).')).toBeVisible();
  await page.getByRole('button',{name:'Continuar resposta'}).click();
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toHaveValue(/Continue a resposta anterior/);
  await expect(page.getByRole('button',{name:'Continuar resposta'})).toHaveCount(0);
  expect(await page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='ai.ask'||c.action==='ai.context'))).toEqual([]);
});

test('a posição na fila fica visível até o processamento começar',async({page})=>{
  await installChat(page,[],true);
  await page.evaluate(()=>{window.__conversationTest.queuePosition=2;window.__conversationTest.processingMs=1000;});
  await expect(page.getByRole('status').filter({hasText:'Aguardando vaga · posição 2'})).toBeVisible();
  await page.evaluate(()=>{delete window.__conversationTest.queuePosition;});
  await expect(page.getByText('Processando · 1 s')).toBeVisible();
  expect(await page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='ai.ask'))).toEqual([]);
});

test('cada conversa recupera seu rascunho e anexo sem enviar mensagens', async ({ page }) => {
  await installChat(page);
  const projects = page.getByRole('complementary', { name: 'Projetos e conversas' });
  const input = page.getByRole('textbox', { name: 'Mensagem da conversa' });
  await input.fill('Revisar este trecho');
  await page.getByRole('button', { name: 'Adicionar à mensagem' }).click();
  const [chooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    page.getByRole('menuitem', { name: 'Anexar foto ou arquivo' }).click(),
  ]);
  await chooser.setFiles({
    name: 'trecho.txt', mimeType: 'text/plain', buffer: Buffer.from('Conteúdo de teste'),
  });
  await expect(page.getByRole('button', { name: 'Remover anexo' })).toBeVisible();
  await projects.getByRole('button', { name: 'Conversa B', exact: true }).click();
  await expect(input).toBeEmpty();
  await expect(page.getByRole('button', { name: 'Remover anexo' })).toHaveCount(0);
  await input.fill('Segundo rascunho');
  await projects.getByRole('button', { name: 'Conversa A', exact: true }).click();
  await expect(input).toHaveValue('Revisar este trecho');
  await expect(page.getByText('trecho.txt')).toBeVisible();
  expect(await page.evaluate(() => window.__conversationTest.calls.filter(c => ['ai.ask', 'ai.operation.propose'].includes(c.action)))).toEqual([]);
});

test('uma resposta recebida em outra conversa limpa apenas o turno enviado', async ({ page }) => {
  await installChat(page);
  const projects = page.getByRole('complementary', { name: 'Projetos e conversas' });
  const input = page.getByRole('textbox', { name: 'Mensagem da conversa' });
  await input.fill('Pergunta que já foi enviada');
  await page.getByRole('button', { name: 'Enviar mensagem' }).click();
  await expect.poll(() => page.evaluate(() => window.__conversationTest.running)).toBe(true);
  await projects.getByRole('button', { name: 'Conversa B', exact: true }).click();
  await input.fill('Pergunta independente');
  await page.evaluate(() => window.dispatchEvent(new Event('fixture:reply')));
  await expect.poll(() => page.evaluate(() => window.__conversationTest.running)).toBe(false);
  await expect(input).toHaveValue('Pergunta independente');
  await projects.getByRole('button', { name: 'Conversa A', exact: true }).click();
  await expect(input).toBeEmpty();
  expect(await page.evaluate(() => window.__conversationTest.calls.filter(c => c.action === 'ai.ask').length)).toBe(1);
});

test('o envio confirmado limpa o compositor antes de o provedor responder', async ({ page }) => {
  await installChat(page);
  const input = page.getByRole('textbox', { name: 'Mensagem da conversa' });
  await input.fill('Revisar a autorização do projeto');
  await page.getByRole('button', { name: 'Enviar mensagem' }).click();
  await expect.poll(() => page.evaluate(() => window.__conversationTest.running)).toBe(true);
  await expect(page.getByRole('region', { name: 'Histórico da conversa' })).toContainText('Revisar a autorização do projeto');
  await expect(input).toBeEmpty();
  expect(await page.evaluate(() => window.__conversationTest.running)).toBe(true);
  await expect(page.getByRole('region',{name:'Histórico da conversa'}).getByText('Revisar a autorização do projeto',{exact:true})).toHaveCount(1);
  await input.fill('Próxima pergunta em preparação');
  await page.evaluate(() => window.dispatchEvent(new Event('fixture:reply')));
  await expect.poll(() => page.evaluate(() => window.__conversationTest.running)).toBe(false);
  await expect(input).toHaveValue('Próxima pergunta em preparação');
});

test('a confirmação de outra revisão não apaga o rascunho e o aceite preserva a próxima edição', async ({ page }) => {
  await installChat(page);
  await page.evaluate(()=>{window.__conversationTest.pretendOtherReview=true;});
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await input.fill('Revisar o arquivo selecionado');
  await page.getByRole('button',{name:'Adicionar à mensagem'}).click();
  const [chooser]=await Promise.all([page.waitForEvent('filechooser'),page.getByRole('menuitem',{name:'Anexar foto ou arquivo'}).click()]);
  await chooser.setFiles({name:'amostra.txt',mimeType:'text/plain',buffer:Buffer.from('Texto sintético sem credenciais')});
  await page.getByRole('button',{name:'Enviar mensagem'}).click();
  await page.getByRole('button',{name:'Autorizar envio'}).click();
  await expect.poll(()=>page.evaluate(()=>window.__conversationTest.running)).toBe(true);
  // The regular renderer refresh confirms that the unrelated active request was observed.
  await expect.poll(()=>page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='ai.chat.status').length)).toBeGreaterThan(3);
  await expect(input).toHaveValue('Revisar o arquivo selecionado');
  await expect(page.getByRole('button',{name:'Remover anexo'})).toBeVisible();
  await input.fill('Nova pergunta digitada enquanto aguardo');
  await page.evaluate(()=>{const bridge=window.__conversationTest;bridge.activeReview=bridge.calls.find(c=>c.action==='ai.ask')!.payload.dataReview as {id:string;digest:string};});
  await expect(page.getByRole('button',{name:'Remover anexo'})).toHaveCount(0);
  await expect(input).toHaveValue('Nova pergunta digitada enquanto aguardo');
  await page.evaluate(()=>window.dispatchEvent(new Event('fixture:reply')));
  await expect.poll(()=>page.evaluate(()=>window.__conversationTest.running)).toBe(false);
  await expect(input).toHaveValue('Nova pergunta digitada enquanto aguardo');
  expect(await page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='ai.ask').length)).toBe(1);
});

test('uma recusa antes do despacho mantém o texto e não cria turno enviado', async ({ page }) => {
  await installChat(page);
  await page.evaluate(()=>{window.__conversationTest.rejectBeforeSend=true;});
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await input.fill('Conferir o projeto após rever o escopo');
  await input.press('Enter');
  await expect(page.getByRole('region',{name:'Conversa do projeto'}).getByRole('alert')).toContainText('revogado antes do envio');
  await expect(input).toHaveValue('Conferir o projeto após rever o escopo');
  expect(await page.evaluate(()=>window.__conversationTest.turns)).toEqual([]);
});

test('falha incerta conserva o turno aceito e não recoloca a mensagem na fila de envio', async ({ page }) => {
  await installChat(page);
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await input.fill('Pergunta com resposta remota incerta');
  await input.press('Enter');
  await expect(input).toBeEmpty();
  await page.evaluate(()=>window.dispatchEvent(new Event('fixture:uncertain')));
  await expect(page.getByRole('region',{name:'Conversa do projeto'}).getByRole('alert')).toContainText('resultado remoto é incerto');
  const projects=page.getByRole('complementary',{name:'Projetos e conversas'});
  await projects.getByRole('button',{name:'Conversa B',exact:true}).click();
  await projects.getByRole('button',{name:'Conversa A',exact:true}).click();
  await expect(input).toBeEmpty();
  await expect(page.getByRole('region',{name:'Histórico da conversa'}).getByText('Pergunta com resposta remota incerta',{exact:true})).toHaveCount(1);
  expect(await page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='ai.ask').length)).toBe(1);
});

test('uma recusa 429 imediata após o aceite recupera a confirmação do histórico', async ({ page }) => {
  await installChat(page);
  await page.evaluate(()=>{window.__conversationTest.failImmediatelyAfterSend=true;});
  const input=page.getByRole('textbox',{name:'Mensagem da conversa'});
  await input.fill('Pergunta admitida antes da recusa rápida');
  await input.press('Enter');
  await expect(page.getByRole('region',{name:'Conversa do projeto'}).getByRole('alert')).toContainText('(429)');
  await expect(input).toBeEmpty();
  await expect(page.getByRole('region',{name:'Histórico da conversa'}).getByText('Pergunta admitida antes da recusa rápida',{exact:true})).toHaveCount(1);
  expect(await page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='ai.ask').length)).toBe(1);
});

test('o teclado envia uma vez, respeita o IME e conserva o rascunho recusado', async ({ page }) => {
  await installChat(page);
  const chat = page.getByRole('region', { name: 'Conversa do projeto' });
  const input = chat.getByRole('textbox', { name: 'Mensagem da conversa' });
  const tooLarge = 'á'.repeat(4050);
  await input.fill(tooLarge);
  await input.press('Enter');
  await expect(chat.getByRole('alert')).toContainText('8.000 bytes');
  await expect(input).toHaveValue(tooLarge);
  await chat.getByRole('button', { name: 'Modo de aprovação' }).click();
  await chat.getByRole('menuitemradio', { name: 'Sempre solicitar' }).click();
  await input.fill('Linha um');
  await input.press('Shift+Enter');
  await input.press('x');
  await expect(input).toHaveValue('Linha um\nx');
  await input.dispatchEvent('keydown', { key: 'Enter', isComposing: true, keyCode: 229 });
  const approve = chat.getByRole('button', { name: 'Autorizar envio' });
  await expect(approve).toHaveCount(0);
  await input.press('Enter');
  await chat.getByRole('button', { name: 'Recusar' }).click();
  await expect(input).toHaveValue('Linha um\nx');
  const count = (action: string) => page.evaluate(action => window.__conversationTest.calls.filter(c => c.action === action).length, action);
  expect(await count('ai.ask')).toBe(0);
  await input.focus();
  await page.keyboard.down('Enter');
  await page.keyboard.down('Enter');
  await page.keyboard.up('Enter');
  await approve.click();
  await expect.poll(() => count('ai.ask')).toBe(1);
  await input.press('Enter');
  expect(await count('ai.ask')).toBe(1);
  await page.evaluate(() => window.dispatchEvent(new Event('fixture:reply')));
  await expect(input).toBeEmpty();
});

test('Ctrl + Enter escolhido continua enviando e os menus trocam em um clique', async ({ page }) => {
  await page.addInitScript(() => localStorage.setItem('openkoda.composer.submit.v1', 'modifier-enter'));
  await installChat(page);
  const chat = page.getByRole('region', { name: 'Conversa do projeto' });
  const input = chat.getByRole('textbox', { name: 'Mensagem da conversa' });
  const add = chat.getByRole('button', { name: 'Adicionar à mensagem' });
  await add.focus();
  await page.keyboard.press('ArrowDown');
  const attachment = chat.getByRole('menuitem', { name: 'Anexar foto ou arquivo' });
  await expect(attachment).toBeFocused();
  await page.keyboard.press('ArrowDown');
  await expect(chat.getByRole('menuitem',{name:'Analisar código do projeto'})).toBeFocused();
  await expect(chat.getByRole('menuitemcheckbox', { name: 'Pedir verificação' })).toHaveCount(0);
  await chat.getByRole('button', { name: 'Modo de aprovação' }).click();
  await expect(chat.getByRole('menu', { name: 'Adicionar à mensagem' })).toBeHidden();
  await expect(chat.getByRole('menuitemradio', { name: 'Solicitar para ações de risco' })).toHaveAttribute('aria-checked', 'true');
  await page.keyboard.press('Escape');
  await expect(chat.getByRole('button', { name: 'Modo de aprovação' })).toBeFocused();
  await chat.getByRole('button', { name: 'Modo de aprovação' }).click();
  await expect(chat.getByRole('menuitemradio', { name: 'Solicitar para ações de risco' })).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(chat.getByRole('menu', { name: 'Modo de aprovação' })).toBeHidden();
  await expect(chat.getByRole('button', { name: 'Modo de aprovação' })).toBeFocused();
  await chat.getByRole('button', { name: 'Modo de aprovação' }).click();
  await expect(chat.getByRole('menuitemradio', { name: 'Solicitar para ações de risco' })).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(chat.getByRole('menu', { name: 'Modo de aprovação' })).toBeHidden();
  await expect(chat.getByRole('button', { name: 'Escolher modelo' })).toBeFocused();
  await input.fill('Dados de teste');
  await input.press('Enter');
  await expect(input).toHaveValue('Dados de teste\n');
  expect(await page.evaluate(() => window.__conversationTest.calls.filter(c => c.action === 'ai.ask').length)).toBe(0);
  await input.press('Control+Enter');
  await expect.poll(() => page.evaluate(() => window.__conversationTest.calls.filter(c => c.action === 'ai.ask').length)).toBe(1);
});

test('o seletor se ancora na barra, aceita teclado e fecha por Escape e clique fora', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await installChat(page);
  const button = page.getByRole('button', { name: /Escolher modelo/ });
  await button.click();
  const dialog = page.getByRole('dialog', { name: 'Escolher provedor e modelo' });
  const search = dialog.getByRole('searchbox');
  await expect(search).toBeFocused();
  await expect(dialog.getByRole('button', { name: 'modelo-original' })).toBeVisible();
  const rect = await dialog.boundingBox(), anchor = await button.boundingBox();
  expect(Math.abs(rect!.y + rect!.height - anchor!.y)).toBeLessThan(12);
  await dialog.getByRole('button', { name: 'OpenRouter' }).click();
  await search.fill('rapido');
  await search.press('ArrowDown');
  await expect(dialog.getByRole('button', { name: 'lab/rapido' })).toBeFocused();
  await page.keyboard.press('Enter');
  await expect(button).toContainText('lab/rapido');
  await expect(button).toBeFocused();
  await button.click();
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
  await expect(button).toBeFocused();
  await button.click();
  const composer = page.getByRole('textbox', { name: 'Mensagem da conversa' });
  // The popover is above the toolbar. Use the exposed end of the textarea so
  // this is a genuine click outside the floating panel.
  const editor = await composer.boundingBox();
  await page.mouse.click(editor!.x + 4, editor!.y + editor!.height - 4);
  await expect(dialog).toHaveCount(0);
  await expect(composer).toBeFocused();
  await page.setViewportSize({ width: 430, height: 620 });
  await button.click();
  const narrow = await dialog.boundingBox();
  expect(narrow!.x).toBeGreaterThanOrEqual(0);
  expect(narrow!.x + narrow!.width).toBeLessThanOrEqual(430);
  await page.mouse.click(2, 2);
  await expect(dialog).toHaveCount(0);
});

test('atividade, resposta formatada e rolagem independente enquanto o provedor trabalha', async ({ page }) => {
  const turns = Array.from({ length: 28 }, (_, i) => ({
    id: String(i), createdAt: new Date(Date.UTC(2026, 0, 2, 0, i)).toISOString(),
    role: 'assistant', text: 'Resultado da etapa ' + i,
  }));
  turns[0].text = '## Revisão\n\n- **Autenticação**\n- Sessão\n\n```html\n<img src="https://invalid.test/pixel">\n```';
  await installChat(page, turns, true);
  const log = page.getByRole('region', { name: 'Histórico da conversa' });
  const jump = page.getByRole('button', { name: 'Voltar à mensagem mais recente' });
  const endDistance = () => log.evaluate(el => el.scrollHeight - el.scrollTop - el.clientHeight);
  await expect(log.getByText('Resultado da etapa 27')).toBeVisible();
  await expect.poll(endDistance).toBeLessThan(4);
  await log.evaluate(el => { el.scrollTop = 0; });
  await expect(jump).toBeVisible();
  await log.locator('summary').filter({ hasText: 'Atividade' }).click();
  await expect(log.getByText('Alvo resolvido', { exact: true })).toBeVisible();
  await expect(log.getByRole('heading', { name: 'Revisão' })).toBeVisible();
  await expect(log.locator('pre code')).toContainText('<img');
  expect(await log.locator('img').count()).toBe(0);
  const top = await log.evaluate(el => el.scrollTop);
  await page.evaluate(() => {
    window.__conversationTest.processingMs=2400;
    window.__conversationTest.partialText='Nova resposta recebida';
  });
  await expect(log.getByText('Nova resposta recebida')).toBeAttached();
  expect(Math.abs(await log.evaluate(el => el.scrollTop) - top)).toBeLessThan(3);
  await jump.click();
  await expect.poll(endDistance).toBeLessThan(4);
  await expect(page.getByRole('textbox', { name: 'Mensagem da conversa' })).toBeFocused();
  await page.getByRole('button', { name: 'Cancelar trabalho' }).click();
  await expect(page.getByRole('button', { name: 'Enviar mensagem' })).toBeVisible();
  expect(await page.evaluate(() => window.__conversationTest.calls.filter(c => c.action === 'ai.chat.stop'))).toEqual([
    { action: 'ai.chat.stop', payload: { assessmentId: 'A', requestId: 'turn-A' } },
  ]);
});


test('o novo chat só entra na lista após a primeira mensagem aceita e exibe o fluxo parcial', async ({page})=>{
  await installChat(page);
  const tree=page.getByRole('complementary',{name:'Projetos e conversas'});
  const create=tree.getByRole('button',{name:'Criar nova conversa em Laboratório'});
  await create.click();
  const chat=page.getByRole('region',{name:'Conversa do projeto'});
  await expect(chat.locator('.project-chat-transcript')).toBeEmpty();
  await expect(tree.locator('.tree-chat')).toHaveCount(2);
  expect(await page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='project.newConversation').length)).toBe(0);
  const input=chat.getByRole('textbox',{name:'Mensagem da conversa'});
  const question='Verificar acesso ao painel administrativo';
  await input.fill(question);
  await input.press('Enter');
  await expect(tree.getByRole('button',{name:question})).toBeVisible();
  await expect(input).toHaveValue('');
  await page.evaluate(()=>{window.__conversationTest.processingMs=2130;});
  await expect(chat.getByText('Processando · 2 s')).toBeVisible();
  await page.evaluate(()=>{window.__conversationTest.partialText='Estou revisando';});
  await expect(chat.getByLabel('Resposta em andamento')).toHaveText(/Estou revisando/);
  await page.evaluate(()=>{window.__conversationTest.partialText='Estou revisando o painel.';});
  await expect(chat.getByLabel('Resposta em andamento')).toContainText('o painel.');
  await page.evaluate(()=>{
    window.__conversationTest.turns.push({id:'reply',role:'assistant',text:'Estou revisando o painel.',createdAt:'2026-11-12T12:01:00Z'});
    window.dispatchEvent(new Event('fixture:reply'));
  });
  await expect(chat.getByLabel('Resposta em andamento')).toHaveCount(0);
  await expect(chat.getByText('Estou revisando o painel.',{exact:true})).toHaveCount(1);
  await page.getByRole('button',{name:'Renomear conversa'}).click();
  const title=page.getByRole('textbox',{name:'Título da conversa'});
  await title.fill('Acesso administrativo');
  await title.press('Enter');
  await expect(tree.getByRole('button',{name:'Acesso administrativo',exact:true})).toBeVisible();
  expect(await page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='ai.ask').length)).toBe(1);
});


for(const fresh of [false,true]){
  test('o compositor autoriza a fonte sensível e publica a análise uma vez '+(fresh?'no chat vazio':'a partir da conversa existente'),async({page})=>{
    await installChat(page);
    await page.evaluate(()=>{window.__conversationTest.sensitiveSource=true;});
    const tree=page.getByRole('complementary',{name:'Projetos e conversas'});
    if(fresh)await tree.getByRole('button',{name:'Criar nova conversa em Laboratório'}).click();
    const chat=page.getByRole('region',{name:'Conversa do projeto'});
    const objective='Revisar os controles de autorização da API';
    await chat.getByRole('textbox',{name:'Mensagem da conversa'}).fill(objective);
    await chat.getByRole('button',{name:'Adicionar à mensagem'}).click();
    await chat.getByRole('menuitem',{name:'Analisar código do projeto'}).click();
    const form=chat.getByRole('form',{name:'Analisar código do projeto'});
    await expect(form.getByRole('textbox',{name:'Objetivo da análise'})).toHaveValue(objective);
    await expect(form).toContainText('Limite de US$ 0,03');
    if(!fresh)await expect(form).toContainText('A análise abre uma nova conversa');
    const start=form.getByRole('button',{name:'Iniciar análise de código'});
    await start.click();
    const consent=form.getByRole('checkbox',{name:/fonte sensível/});
    await expect(consent).toBeVisible();
    await expect(tree.locator('.tree-chat')).toHaveCount(2);
    expect(await page.evaluate(()=>window.__conversationTest.calls.filter(c=>c.action==='security.scan.start'))).toHaveLength(0);
    await consent.check();await start.click();
    await expect(tree.getByRole('button',{name:objective,exact:true})).toBeVisible();
    const scan=chat.getByRole('region',{name:'Análise de segurança'});
    await scan.getByText(/Atividade da análise/).click();
    await expect(scan.getByText('Leitura de src/api.rs')).toBeVisible();
    const calls=await page.evaluate(()=>window.__conversationTest.calls);
    expect(calls.filter(c=>c.action==='security.scan.start').map(c=>c.payload)).toEqual([expect.objectContaining({
      assessmentId:'new-thread',acknowledgeSensitive:true,maxCostUsd:0.03,target:['src/api.rs'],
    })]);
    const creations=calls.filter(c=>c.action==='project.newConversation').map(c=>c.payload);
    expect(new Set(creations.map(c=>c.idempotencyKey)).size).toBe(1);
    expect(creations[0]).toMatchObject({sourceAssessmentId:'A',analysisObjective:objective,aiSelection:{provider:'openai',model:'modelo-original'}});
    expect(calls.filter(c=>c.action==='ai.ask')).toHaveLength(0);
  });
}

test('uma resposta longa acompanha o final e preserva a posição ao subir para ler', async ({page})=>{
  await installChat(page,[],true);
  const log=page.getByRole('region',{name:'Histórico da conversa'});
  await page.evaluate(()=>{
    window.__conversationTest.processingMs=1800;
    window.__conversationTest.partialText='Início da resposta para leitura.';
  });
  const reply=log.getByLabel('Resposta em andamento');
  await expect(reply).toBeVisible();
  await page.evaluate(()=>{window.__conversationTest.partialText+='\n\n'+Array.from({length:170},(_,i)=>'Parágrafo '+i).join('\n\n');});
  const jump=page.getByRole('button',{name:'Voltar à mensagem mais recente'});
  await expect(reply).toContainText('Parágrafo 169');
  const endDistance=()=>log.evaluate(el=>el.scrollHeight-el.scrollTop-el.clientHeight);
  await expect.poll(endDistance).toBeLessThan(4);
  await log.evaluate(el=>{el.scrollTop=150;});
  await expect(jump).toBeVisible();
  await page.evaluate(()=>{window.__conversationTest.partialText+='\n\nÚltimo trecho.';});
  await expect(reply).toContainText('Último trecho.');
  expect(await log.evaluate(el=>el.scrollTop)).toBe(150);
  await jump.click();
  await expect.poll(endDistance).toBeLessThan(4);
});
