import { test, expect } from './fixtures';

test('as áreas principais abrem vazias e a prévia não simula Kali ou IA', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByText('Prévia no navegador · Dados locais de demonstração')).toBeVisible();
  for (const name of ['Projetos', 'Achados', 'Avaliações']) {
    await page.getByRole('button', { name, exact: true }).click();
    await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  }
  await page.getByRole('button', { name: 'Kali e ferramentas', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Kali e ferramentas', exact: true })).toBeVisible();
  await expect(page.getByText('Kali pronto', { exact: true })).toHaveCount(0);
  await page.getByRole('button',{name:'Ambiente',exact:true}).click();
  await expect(page.getByRole('button',{name:'Preparar com imagem oficial verificada',exact:true})).toHaveCount(0);
  await page.getByRole('button', { name: 'Configurações', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Configurações', exact: true })).toBeVisible();
  await expect(page.getByText('Conexão verificada', { exact: true })).toHaveCount(0);
  await page.screenshot({ path: test.info().outputPath('configuracoes.png'), fullPage: true });
  await page.getByRole('button',{name:'Dados e privacidade',exact:true}).click();
  await expect(page.getByText('Backup e recuperação estão disponíveis no aplicativo desktop.',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Revisar ativação desta cópia',exact:true})).toHaveCount(0);
  await expect(page.getByText('A revisão e a exclusão de arquivos geridos estão disponíveis no aplicativo desktop.',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Revisar exclusão selecionada',exact:true})).toHaveCount(0);
  await expect(page.getByText('A exclusão revisada de projetos e histórico está disponível no aplicativo desktop.',{exact:true})).toBeVisible();
  await expect(page.getByRole('button',{name:'Preparar revisão do projeto arquivado',exact:true})).toHaveCount(0);
});

test('cria projeto e rascunho e conserva ambos depois de recarregar', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Projetos', exact: true }).click();
  await page.getByRole('button', { name: 'Adicionar projeto', exact: true }).click();
  await page.getByLabel('Nome do projeto', { exact: true }).fill('Laboratório E2E');
  await page.getByLabel('Modalidade', { exact: true }).selectOption('web');
  await page.getByLabel('Alvo', { exact: true }).fill('https://api.lab.local');
  await page.getByLabel('Descrição', { exact: true }).fill('Projeto isolado desta execução do teste.');
  await page.getByRole('button', { name: 'Criar projeto', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Laboratório E2E', exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Nova avaliação', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Nova avaliação', exact: true })).toBeVisible();
  await page.getByLabel('Título', { exact: true }).fill('Avaliação de integração E2E');
  await page.getByLabel('Objetivo', { exact: true }).fill('Revisar o fluxo de autenticação do laboratório.');
  await expect(page.getByLabel('Alvo', { exact: true })).toHaveValue('https://api.lab.local');
  await page.screenshot({ path: test.info().outputPath('nova-avaliacao.png'), fullPage: true });
  await page.getByRole('button', { name: 'Revisar avaliação', exact: true }).click();
  await page.getByRole('button', { name: 'Salvar rascunho', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Avaliação de integração E2E', exact: true })).toBeVisible();
  await expect(page.getByText('Rascunho', { exact: true }).first()).toBeVisible();
  await page.reload();
  await expect(page.getByText('Avaliação de integração E2E', { exact: true }).first()).toBeVisible();
  await page.getByRole('button', { name: 'Projetos', exact: true }).click();
  await expect(page.getByText('Laboratório E2E', { exact: true }).first()).toBeVisible();
  await page.screenshot({ path: test.info().outputPath('projeto-persistido.png'), fullPage: true });
});

test('formulário não aceita projeto sem nome e sem alvo', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Projetos', exact: true }).click();
  await page.getByRole('button', { name: 'Adicionar projeto', exact: true }).click();
  await page.getByRole('button', { name: 'Criar projeto', exact: true }).click();
  await expect(page.getByLabel('Nome do projeto', { exact: true })).toBeVisible();
  await expect.poll(() => page.getByLabel('Nome do projeto', { exact: true }).evaluate(input => input.matches(':invalid'))).toBe(true);
});

test('iniciar na prévia informa indisponibilidade e preserva o rascunho', async ({ page }) => {
  await page.goto('/');
  // Seed through the actual app transport: no invented endpoints or shared storage snapshot.
  await page.evaluate(async () => {
    const moduleUrl = '/src/api.ts';
    const { request } = await import(moduleUrl);
    const project = await request('project.create', {
      name: 'Projeto de bloqueio', kind: 'web', target: 'https://blocked.lab.local', description: '',
    });
    await request('assessment.create', {
      projectId: project.id, title: 'Avaliação sem executor', kind: project.kind,
      target: project.target, objective: 'Confirmar indisponibilidade sem executar.',
    });
  });
  await page.reload();
  await page.getByText('Avaliação sem executor', { exact: true }).first().click();
  await page.getByRole('button', { name: 'Iniciar avaliação', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText(/desktop|executor/i);
  await page.reload();
  await page.getByText('Avaliação sem executor', { exact: true }).first().click();
  await expect(page.getByText('Rascunho', { exact: true }).first()).toBeVisible();
  await expect(page.getByText('Em andamento', { exact: true })).toHaveCount(0);
  await page.locator('summary').filter({hasText:'Revisar controle HTTP'}).click();
  await expect(page.locator('.http-candidate-review')).toContainText('A prévia não consulta alvos');
  await expect(page.getByRole('button',{name:'Revisar observação HTTP',exact:true})).toHaveCount(0);
  await page.locator('summary').filter({hasText:'Kali: diagnóstico e integridade'}).click();
  await expect(page.locator('.kali-bridge')).toContainText('A prévia não inicia sessões MCP.');
  await expect(page.getByRole('button',{name:'Diagnosticar Kali nesta avaliação',exact:true})).toHaveCount(0);
});

test('navegação responde ao teclado em janela menor sem rolagem horizontal', async ({ page }) => {
  await page.setViewportSize({ width: 1024, height: 768 });
  await page.goto('/');
  const projects = page.getByRole('button', { name: 'Projetos', exact: true });
  await projects.focus();
  await page.keyboard.press('Enter');
  await expect(page.getByRole('heading', { name: 'Projetos', exact: true })).toBeVisible();
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.getByRole('button', { name: 'Kali e ferramentas', exact: true }).click();
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  await page.screenshot({ path: test.info().outputPath('kali-1024.png'), fullPage: true });
});

test('salva configurações para a próxima avaliação sem testar conexão implicitamente', async ({ page }) => {
  const providerRequests: string[] = [];
  page.on('request', request => {
    if (request.url().startsWith('https://api.example.test')) providerRequests.push(request.url());
  });
  await page.goto('/');
  await page.getByRole('button', { name: 'Configurações', exact: true }).click();
  await page.getByLabel('Endpoint', { exact: true }).fill('https://api.example.test/v1');
  await page.getByLabel('Modelo', { exact: true }).fill('modelo-e2e');
  await page.getByRole('button', { name: 'Salvar alterações', exact: true }).click();
  await expect.poll(() => page.evaluate(() => JSON.parse(localStorage.getItem('openkoda.preview.v1') ?? '{}').settings?.model)).toBe('modelo-e2e');
  await page.reload();
  await page.getByRole('button', { name: 'Configurações', exact: true }).click();
  await expect(page.getByLabel('Endpoint', { exact: true })).toHaveValue('https://api.example.test/v1');
  await expect(page.getByLabel('Modelo', { exact: true })).toHaveValue('modelo-e2e');
  await expect(page.getByText('Conexão verificada', { exact: true })).toHaveCount(0);
  expect(providerRequests).toEqual([]);
});

test('configurações e navegação continuam legíveis em janela estreita',async({page})=>{
  await page.setViewportSize({width:390,height:844});await page.goto('/');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await expect(page.getByLabel('Endpoint',{exact:true})).toBeVisible();
  await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
  await page.getByRole('button',{name:'Geral',exact:true}).click();
  await page.getByLabel('Tema',{exact:true}).selectOption('dark');
  await page.getByRole('button',{name:'Salvar alterações',exact:true}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme','dark');
  await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
});
