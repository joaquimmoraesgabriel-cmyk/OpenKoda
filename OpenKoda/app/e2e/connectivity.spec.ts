import { Page } from '@playwright/test';
import { emptySnapshot } from '../src/preview';
import { test, expect } from './fixtures';

async function generalSettings(page: Page) {
  await page.getByRole('button', { name: 'Configurações', exact: true }).click();
  await page.getByRole('button', { name: 'Geral', exact: true }).click();
  return page.getByRole('region', { name: 'Conectividade com a Internet' });
}

test('a prévia não atribui ao Windows o estado online do navegador', async ({ page }) => {
  await page.addInitScript(() => Object.defineProperty(navigator, 'onLine', { value: true }));
  await page.goto('/?area=tecnica');
  const panel = await generalSettings(page);
  await expect(panel.getByText('O indicador do Windows está indisponível na prévia do navegador.')).toBeVisible();
  await expect(panel.getByRole('button')).toHaveCount(0);
});

test('reabrir as configurações compartilha a consulta e preserva o acesso aos projetos locais', async ({ page }) => {
  const snapshot = emptySnapshot();
  snapshot.projects.push({
    id: 'local-index', name: 'Projeto disponível sem rede', description: 'Fixture local',
    kind: 'code', target: 'D:/fixture', createdAt: '2026-01-01T12:00:00Z',
  });
  const observedAt = '2026-01-02T12:34:56Z';
  await page.addInitScript(({ snapshot, observedAt }) => {
    const host = window as unknown as {
      isTauri: boolean; __TAURI_INTERNALS__: unknown; observations: number; finishReading: () => void;
    };
    host.isTauri = true;
    host.observations = 0;
    host.__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args: { request: { action: string; payload: object } }) => {
        if (cmd !== 'request') throw new Error('Unexpected IPC');
        switch (args.request.action) {
          case 'snapshot': return snapshot;
          case 'data.session.select': return {};
          case 'projectPurge.status': return { hasUnresolved: false, journals: [] };
          case 'environment.status': return { status: 'degraded', distro: 'Fixture', tools: [], message: 'Ambiente indisponível' };
          case 'host.connectivity': {
            if (Object.keys(args.request.payload).length) throw new Error('Unexpected payload');
            const attempt = ++host.observations;
            if (attempt === 1) await new Promise<void>(resolve => { host.finishReading = resolve; });
            if (attempt === 2) throw new Error('Private fixture diagnostic');
            return { source: 'windowsNlm', status: attempt === 1 ? 'internet' : 'disconnected', observedAt };
          }
          default: throw new Error('Optional service unavailable in this fixture');
        }
      },
    };
  }, { snapshot, observedAt });
  await page.goto('/?area=tecnica');
  const panel = await generalSettings(page);
  const read = panel.getByRole('button', { name: 'Consultar conectividade' });
  await expect(panel.getByRole('status')).toContainText('Ainda não consultado');
  await expect(panel.getByText(/Gerenciador de Lista de Redes do Windows/)).toBeVisible();
  expect(await page.evaluate(() => Reflect.get(window, 'observations'))).toBe(0);
  await read.click();
  await expect(read).toBeDisabled();
  await page.getByRole('button', { name: 'Permissões', exact: true }).click();
  await page.getByRole('button', { name: 'Geral', exact: true }).click();
  await read.click();
  await expect(read).toBeDisabled();
  expect(await page.evaluate(() => Reflect.get(window, 'observations'))).toBe(1);
  await page.evaluate(() => Reflect.get(window, 'finishReading')());
  await expect(panel.getByRole('status')).toContainText('Acesso à Internet indicado');
  await expect(panel.locator('time')).toHaveAttribute('datetime', observedAt);
  await read.click();
  await expect(panel.getByRole('status')).toContainText('Indicador indisponível');
  await expect(panel).not.toContainText('Private fixture diagnostic');
  await read.click();
  await expect(panel.getByRole('status')).toContainText('Desconectado da rede');
  await page.getByRole('button', { name: 'Projetos', exact: true }).click();
  await expect(page.getByText('Projeto disponível sem rede', { exact: true })).toBeVisible();
});
