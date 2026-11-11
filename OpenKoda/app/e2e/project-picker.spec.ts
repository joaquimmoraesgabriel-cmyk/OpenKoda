import { expect, test } from './fixtures';
import { installCombinedFixture } from './combined-fixture';

test('a escolha de fonte respeita a modalidade e cancelar conserva o alvo', async ({ page }) => {
  await installCombinedFixture(page);
  // Cover the UI contract independently of the Windows file dialog.
  await page.evaluate(() => {
    const ipc = (window as any).__TAURI_INTERNALS__;
    const original = ipc.invoke.bind(ipc);
    let attempts = 0;
    ipc.invoke = (command: string, args: { request?: { action: string; payload: { selection: string } } }) => {
      if (args?.request?.action !== 'project.pickTarget') return original(command, args);
      attempts++;
      if (attempts === 2) return Promise.resolve(null);
      if (attempts === 4) return Promise.reject('O seletor não abriu.');
      if (attempts === 5) return new Promise(resolve => { (window as any).finishProjectPick = resolve; });
      return Promise.resolve(args.request.payload.selection === 'folder' ? 'D:/Laboratório/Fonte' : 'D:/Laboratório/nota.txt');
    };
  });
  await page.getByRole('button', { name: 'Projetos', exact: true }).click();
  await page.getByRole('button', { name: 'Novo projeto', exact: true }).click();
  const target = page.getByLabel('Alvo', { exact: true });
  const kind = page.getByLabel('Modalidade', { exact: true });
  const folder = page.getByRole('button', { name: 'Escolher pasta', exact: true });
  const file = page.getByRole('button', { name: 'Escolher arquivo', exact: true });
  await expect(file).toHaveCount(0);
  await folder.click();
  await expect(target).toHaveValue('D:/Laboratório/Fonte');
  await folder.click();
  await expect(target).toHaveValue('D:/Laboratório/Fonte');
  await kind.selectOption('files');
  await file.click();
  await expect(target).toHaveValue('D:/Laboratório/nota.txt');
  await folder.click();
  await expect(page.getByRole('alert')).toContainText('O seletor não abriu.');
  await expect(target).toHaveValue('D:/Laboratório/nota.txt');
  await file.click();
  await kind.selectOption('web');
  await target.fill('https://exemplo.local');
  await page.evaluate(() => (window as any).finishProjectPick('D:/Resultado antigo'));
  await expect(target).toHaveValue('https://exemplo.local');
  for (const remote of ['web', 'network']) {
    await kind.selectOption(remote);
    await expect(folder).toHaveCount(0);
    await expect(file).toHaveCount(0);
    await expect(target).toBeEditable();
  }
});

test('a prévia conserva a entrada manual sem solicitar acesso aos arquivos do navegador', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Projetos', exact: true }).click();
  await page.getByRole('button', { name: 'Novo projeto', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Escolher pasta' })).toHaveCount(0);
  const target = page.getByLabel('Alvo', { exact: true });
  await target.fill('D:/Projetos/Exemplo');
  await expect(target).toHaveValue('D:/Projetos/Exemplo');
  await page.getByLabel('Nome do projeto', { exact: true }).fill('Verificação de foco');
  await page.getByRole('button', { name: 'Criar projeto', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'Verificação de foco' })).toBeFocused();
  await page.getByRole('button', { name: 'Voltar aos projetos' }).click();
  await expect(page.getByRole('button', { name: /Verificação de foco/ })).toBeFocused();
  const newProject = page.getByRole('button', { name: 'Novo projeto' });
  await newProject.click();
  await page.getByRole('button', { name: 'Cancelar', exact: true }).click();
  await expect(newProject).toBeFocused();
});
