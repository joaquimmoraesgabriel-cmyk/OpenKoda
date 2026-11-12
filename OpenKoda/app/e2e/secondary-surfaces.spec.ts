import { type Locator } from '@playwright/test';
import { expect, test } from './fixtures';

async function fitsWidth(region: Locator) {
  await expect(region).toBeVisible();
  await expect.poll(() => region.evaluate(element => element.scrollWidth - element.clientWidth)).toBeLessThanOrEqual(1);
}

test('o formulário compacto cabe na janela e as opções adicionais continuam acessíveis pelo teclado', async ({page}) => {
  await page.setViewportSize({width: 1280, height: 850});
  await page.goto('/');
  await page.getByRole('button', {name: 'Criar projeto', exact: true}).click();
  const form = page.locator('.launch-page');
  await form.getByLabel('Pasta do projeto', {exact: true}).fill('D:/Laboratorio/portal');
  await expect(form.getByRole('button', {name: 'Criar projeto', exact: true})).toBeInViewport();
  await page.setViewportSize({width: 390, height: 720});
  const toggle = form.getByRole('switch', {name: 'Contexto adicional'});
  await toggle.focus();
  await page.keyboard.press('Space');
  await expect(toggle).toBeChecked();
  const field = form.getByLabel('Vetores de ataque');
  await page.keyboard.press('Tab');
  await expect(field).toBeFocused();
  await fitsWidth(form);
});

test('conteúdos de ajustes, perfil, achados e diagnóstico cabem na coluna estreita', async ({page}) => {
  await page.setViewportSize({width: 390, height: 720});
  await page.emulateMedia({reducedMotion: 'reduce', colorScheme: 'dark'});
  await page.goto('/');
  await page.getByRole('button', {name: 'Configurações', exact: true}).click();
  const categories = page.getByRole('navigation', {name: 'Categorias de configuração'});
  for (const name of ['Geral', 'Aparência', 'IA e modelos', 'Uso e orçamento', 'Histórico de uso']) {
    const button = categories.getByRole('button', {name, exact: true});
    await button.click();
    await expect(button).toHaveAttribute('aria-current', 'page');
    await fitsWidth(page.getByRole('region', {name: 'Conteúdo de ' + name}));
  }
  await page.getByRole('button', {name: 'Perfil', exact: true}).click();
  await page.getByRole('button', {name: 'Editar perfil'}).click();
  await fitsWidth(page.locator('.profile-content'));
  await expect(page.getByLabel('Nome de exibição')).toBeFocused();
  await page.getByRole('button', {name: 'Achados', exact: true}).click();
  await fitsWidth(page.getByRole('region', {name: 'Lista de achados'}));
  await page.getByRole('button', {name: 'Configurações', exact:true}).click();
  await categories.getByRole('button', {name:'Diagnóstico e suporte', exact:true}).click();
  await fitsWidth(page.getByRole('region', {name:'Conteúdo de Diagnóstico e suporte'}));
});
