import { test, expect } from './fixtures';
import { appearanceStorageKey } from '../src/appearance';
import type { Page } from '@playwright/test';

async function appearance(page: Page) {
  await page.getByRole('button', {name:'Configurações', exact:true}).click();
  await page.getByRole('button', {name:'Aparência', exact:true}).click();
}

test('Geral compartilha o perfil, a busca encontra categorias sem acentos e fontes se aplicam em outras páginas', async({page})=>{
  await page.goto('/');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByLabel('Nome de exibição').fill('Usuária de teste');
  await page.getByRole('button',{name:'Salvar nome'}).click();
  await page.getByLabel('Atalho de envio').selectOption('enter');
  const search=page.getByRole('searchbox',{name:'Pesquisar configurações'});
  await search.fill('aparencia');
  await expect(page.getByRole('button',{name:'Geral',exact:true})).toHaveCount(0);
  await page.getByRole('button',{name:'Aparência',exact:true}).click();
  await page.getByLabel('Fonte de interface',{exact:true}).fill('Arial');
  await page.getByLabel('Tamanho do texto',{exact:true}).selectOption('18');
  await page.getByLabel('Tamanho do código',{exact:true}).selectOption('15');
  await page.getByRole('combobox',{name:'Navegação principal',exact:true}).selectOption('icons');
  await page.getByLabel('Avaliações e achados',{exact:true}).selectOption('line');
  await page.getByRole('radio',{name:'Névoa',exact:true}).check();
  await page.getByRole('button',{name:'Salvar alterações'}).click();
  await expect(page.locator('html')).toHaveAttribute('data-wallpaper','mist');
  await page.getByRole('button',{name:'Perfil',exact:true}).click();
  await expect(page.getByRole('heading',{name:'Usuária de teste'})).toBeVisible();
  expect(await page.locator('h1').evaluate(el=>getComputedStyle(el).fontFamily)).toContain('Arial');
  await expect(page.locator('.workspace-wordmark > span')).toBeHidden();
  await page.reload();
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await expect(page.getByLabel('Atalho de envio')).toHaveValue('enter');
  await expect(page.locator('html')).toHaveCSS('font-size','18px');
  expect(await page.locator('html').evaluate(el=>getComputedStyle(el).getPropertyValue('--code-font-size').trim())).toBe('15px');
  await expect(page.locator('html')).toHaveAttribute('data-list-style','line');
});

test('aparência é aplicada no dispositivo, mantém o formato anterior dos dados e sobrevive à reabertura', async ({page}) => {
  await page.goto('/');
  const open = () => appearance(page);
  await open();
  await expect(page.getByRole('heading', {name:'Aparência', exact:true})).toBeVisible();
  const scale = page.getByLabel('Tamanho da interface', {exact:true});
  const compact = page.getByRole('radio', {name:'Compacta'});
  const dark = page.getByRole('radio', {name:'Escuro'});
  const save = page.getByRole('button', {name:'Salvar alterações'});
  await dark.check();
  await scale.selectOption('125');
  await compact.check();
  await page.getByLabel('Movimento', {exact:true}).selectOption('reduced');
  await page.getByRole('button', {name:'Descartar', exact:true}).click();
  await expect(save).toBeDisabled();
  await expect(scale).toHaveValue('100');
  await expect(compact).not.toBeChecked();
  await expect(dark).not.toBeChecked();
  await dark.check();
  await scale.selectOption('125');
  await compact.check();
  await page.getByLabel('Movimento', {exact:true}).selectOption('reduced');
  await save.click();
  await expect(page.locator('html')).toHaveAttribute('data-density', 'compact');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await expect(page.locator('html')).toHaveAttribute('data-motion', 'reduced');
  expect(await page.evaluate(() => getComputedStyle(document.documentElement).zoom)).toBe('1.25');
  const oldFormat = await page.evaluate(() => JSON.parse(localStorage.getItem('openkoda.preview.v1')!).settings);
  expect(oldFormat.theme).toBe('dark');
  expect(oldFormat).not.toHaveProperty('interfaceScale');
  expect(oldFormat).not.toHaveProperty('density');
  expect(oldFormat).not.toHaveProperty('motion');
  await page.reload();
  await open();
  await expect(scale).toHaveValue('125');
  await expect(dark).toBeChecked();
  await expect(compact).toBeChecked();
  await expect(save).toBeDisabled();
  await page.setViewportSize({width:390, height:800});
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth + 1)).toBe(true);
  const profile = page.getByRole('button', {name:'Perfil', exact:true});
  await expect(profile).toBeInViewport({ratio:0.9});
});

test('falha ao salvar tema preserva as escolhas anteriores e mantém o rascunho para tentar de novo', async ({page}) => {
  await page.addInitScript(() => {
    const write = Storage.prototype.setItem;
    Storage.prototype.setItem = function(key, value) {
      if (key === 'openkoda.preview.v1') throw new Error('Disco de teste indisponível');
      write.call(this, key, value);
    };
  });
  await page.goto('/');
  await appearance(page);
  await page.getByRole('radio', {name:'Escuro'}).check();
  await page.getByRole('radio', {name:'Compacta'}).check();
  await page.getByRole('button', {name:'Salvar alterações'}).click();
  await expect(page.locator('.settings-actions [role=alert]')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('html')).toHaveAttribute('data-density', 'comfortable');
  expect(await page.evaluate(key => localStorage.getItem(key), appearanceStorageKey)).toBeNull();
  await expect(page.getByRole('radio', {name:'Compacta'})).toBeChecked();
  await expect(page.getByRole('button', {name:'Salvar alterações'})).toBeEnabled();
});

test('armazenamento visual indisponível conserva tema, oferece descarte e permite salvar tema sozinho', async ({page}) => {
  await page.addInitScript(key => {
    const write = Storage.prototype.setItem;
    Storage.prototype.setItem = function(k, value) {
      if (k === key) throw new Error('Quota de teste');
      write.call(this, k, value);
    };
  }, appearanceStorageKey);
  await page.goto('/');
  await appearance(page);
  await page.getByRole('radio', {name:'Escuro'}).check();
  await page.getByRole('radio', {name:'Compacta'}).check();
  await page.getByRole('button', {name:'Salvar alterações'}).click();
  await expect(page.getByRole('alert')).toContainText('não permitiu salvar');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.getByRole('button', {name:'Descartar',exact:true}).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.getByRole('radio', {name:'Escuro'}).check();
  await page.getByRole('button', {name:'Salvar alterações'}).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});
