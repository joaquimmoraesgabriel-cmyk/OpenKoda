import { test, expect } from './fixtures';

test('visão geral reflete os dados e abre o projeto ou a avaliação correta', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('heading', { name: 'Visão geral', exact: true })).toBeVisible();
  await expect(page.locator('.dashboard-no-activity')).toBeVisible();
  await page.getByRole('button', { name: 'Novo projeto', exact: true }).click();
  await page.getByLabel('Nome do projeto', { exact: true }).fill('Portal de teste');
  await page.getByLabel('Modalidade', { exact: true }).selectOption('web');
  await page.getByLabel('Alvo', { exact: true }).fill('https://portal.example.test');
  await page.getByRole('button', { name: 'Criar projeto', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Portal de teste' })).toBeVisible();
  await page.evaluate(async () => {
    const path = '/src/api.ts';
    const { request } = await import(path);
    const { projects: [project] } = await request('snapshot');
    await request('assessment.create', {
      projectId: project.id, title: 'Verificar portal', kind: 'web', target: project.target, objective: 'Avaliação sintética.'
    });
  });
  await page.reload();
  await expect(page.locator('.dashboard-stat').filter({ hasText: 'Projetos ativos' })).toContainText('01');
  await expect(page.locator('.dashboard-chart li').last()).toHaveAccessibleName(/avaliações criadas: 1/);
  await page.getByRole('button', { name: 'Mês', exact: true }).click();
  await expect(page.locator('.dashboard-chart li')).toHaveCount(4);
  await expect(page.locator('.dashboard-chart li').last()).toHaveAccessibleName(/avaliações criadas: 1/);
  await page.getByRole('button', { name: 'Abrir avaliação', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Verificar portal' })).toBeVisible();
  const nav = page.getByRole('navigation', { name: 'Navegação principal' });
  const main = page.getByRole('main');
  await main.evaluate(element => element.scrollTo(0, element.scrollHeight));
  await expect.poll(() => main.evaluate(element => element.scrollTop)).toBeGreaterThan(0);
  await nav.getByRole('button', { name: 'Nova avaliação', exact: true }).click();
  await expect(main.getByRole('heading', { name: 'Nova avaliação', exact: true })).toBeInViewport();
  await expect(main).toBeFocused();
  await nav.getByRole('button', { name: 'Visão geral', exact: true }).click();
  await page.getByRole('button', { name: /Portal de teste Web/ }).click();
  await expect(page.getByRole('heading', { name: 'Portal de teste' })).toBeVisible();
});

test('a lateral permanece visível ao rolar e o perfil abre pelo teclado', async ({ page }) => {
  for (const width of [1400, 512]) {
    await page.setViewportSize({ width, height: 430 });
    await page.goto('/');
    const main = page.locator('#main-content');
    await expect(main.getByRole('heading', { name: 'Visão geral' })).toBeVisible();
    const nav = page.getByRole('navigation', { name: 'Navegação principal' });
    const profile = nav.getByRole('button', { name: 'Perfil', exact: true });
    const settings = nav.getByRole('button', { name: 'Configurações', exact: true });
    const position = await settings.boundingBox();
    await main.evaluate(el => el.scrollTo(0, el.scrollHeight));
    await expect.poll(() => main.evaluate(el => el.scrollTop)).toBeGreaterThan(0);
    expect(await settings.boundingBox()).toEqual(position);
    await expect(settings).toBeInViewport();
    await expect(profile).toBeInViewport();
    await profile.focus();
    await page.keyboard.press('Enter');
    await expect(main.getByRole('heading', { name: 'Perfil', exact: true })).toBeVisible();
    await expect(main).toBeFocused();
    await expect(main.getByText('Dados de demonstração do navegador')).toBeVisible();
    await expect(profile).toHaveAttribute('aria-current', 'page');
    await page.getByRole('button', { name: 'Preferências do aplicativo' }).click();
    await expect(main.getByRole('heading', { name: 'Configurações', exact: true })).toBeVisible();
  }
});
