import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import { emptySnapshot } from '../src/preview';
import { installScopeFixture } from './scope-fixture';

type Turn = { id: string; role: string; text: string; createdAt: string };
type Bridge = {
  turns: Turn[];
  calls: { action: string; payload: Record<string, unknown> }[];
  running: boolean;
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
      if (action === 'snapshot') return snapshot;
      if (action === 'assessment.scopeStatus') return scope.status(payload.assessmentId);
      if (action === 'ai.context') return { digest: 'packet-a', bytes: 120, context: { assessmentId: payload.assessmentId } };
      if (action === 'ai.budget.status') return { enabled: false, degraded: false };
      if (action === 'ai.ask') {
        if(bridge.rejectBeforeSend)throw new Error('O escopo foi revogado antes do envio.');
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
      if (action === 'ai.history') return { messages: payload.assessmentId === 'A' ? bridge.turns : [], requests:payload.assessmentId==='A'&&bridge.acceptedMessage?[{id:'turn-A',dataReviewRef:bridge.activeReview,chatMessageIds:[bridge.acceptedMessage]}]:[] };
      if (action === 'ai.chat.status') return { requestId: bridge.running && payload.assessmentId === 'A' ? 'turn-A' : null, stopping: false, messageId:bridge.running?bridge.acceptedMessage:null,dataReview:bridge.running?bridge.activeReview:null };
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
  await expect(attachment).toBeFocused();
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
  await page.evaluate(() => window.__conversationTest.turns.push({
    id: 'new', createdAt: '2026-01-03T12:00:00Z', role: 'assistant', text: 'Nova resposta recebida',
  }));
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
