import type { Page } from '@playwright/test';
import { expect, test } from './fixtures';
import { emptySnapshot } from '../src/preview';
import { installScopeFixture } from './scope-fixture';

type Turn = { id: string; role: string; text: string; createdAt: string };
type Bridge = {
  turns: Turn[];
  calls: { action: string; payload: Record<string, unknown> }[];
  running: boolean;
};
declare global { interface Window { __conversationTest: Bridge } }

async function installChat(page: Page, turns: Turn[] = [], running = false) {
  await installScopeFixture(page);
  const snapshot = emptySnapshot();
  snapshot.settings = { ...snapshot.settings, provider: 'openai', model: 'modelo-original', endpoint: 'https://api.openai.com/v1', hasApiKey: true };
  const date = '2026-01-01T12:00:00Z';
  snapshot.projects = [{ id: 'fixture', name: 'Laboratório', kind: 'network', target: '127.0.0.1', description: '', createdAt: date }];
  snapshot.assessments = ['A', 'B'].map(letter => ({
    id: letter, projectId: 'fixture', title: 'Conversa ' + letter, kind: 'network', target: '127.0.0.1',
    objective: '', status: 'completed', events: [
      { id: 'e1', message: 'Alvo resolvido', kind: 'info', at: date },
      { id: 'e2', message: 'Leitura concluída', kind: 'info', at: date },
    ],
    evidence: [], coverage: [], summary: '', createdAt: date, updatedAt: date,
    aiSelection: { provider: 'openai', model: 'modelo-original' },
  }));
  await page.addInitScript(({ snapshot, turns, running }) => {
    const scope = window.__scopeFixture(snapshot, ['network.tcp-connect.v1', 'ai.remote-review.v1']);
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
        bridge.running = true;
        await new Promise(resolve => window.addEventListener('fixture:reply', resolve, { once: true }));
        bridge.running = false;
        return {};
      }
      if (action === 'environment.status') return { tools: [], status: 'unavailable', message: 'Teste' };
      if (action === 'ai.history') return { messages: payload.assessmentId === 'A' ? bridge.turns : [] };
      if (action === 'ai.chat.status') return { requestId: bridge.running && payload.assessmentId === 'A' ? 'turn-A' : null, stopping: false };
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
  await page.locator('.project-composer input[type=file]').setInputFiles({
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
