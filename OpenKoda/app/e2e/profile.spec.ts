import { test, expect } from './fixtures';

test('perfil salva o nome e a cor no dispositivo e preserva cancelamentos', async({page})=>{
  await page.goto('/');
  const profile=page.getByRole('button',{name:'Perfil',exact:true});
  await profile.click();
  await page.getByRole('button',{name:'Editar perfil'}).click();
  await page.getByLabel('Nome de exibição').fill('Marina Pires');
  await page.getByRole('radio',{name:'Ameixa'}).check();
  await page.getByRole('button',{name:'Salvar perfil'}).click();
  await expect(page.getByRole('heading',{name:'Marina Pires'})).toBeVisible();
  await expect(page.locator('.person-avatar')).toHaveText('MP');
  await expect(page.locator('.person-avatar')).toHaveAttribute('data-color','ameixa');
  await page.reload(); await profile.click();
  await expect(page.getByRole('heading',{name:'Marina Pires'})).toBeVisible();
  await page.getByRole('button',{name:'Editar perfil'}).click();
  await page.getByLabel('Nome de exibição').fill('Descartar alteração');
  await page.keyboard.press('Escape');
  await expect(page.getByRole('heading',{name:'Marina Pires'})).toBeVisible();
  await expect(page.getByRole('button',{name:'Editar perfil'})).toBeFocused();
  await expect(page.getByText('Nenhuma avaliação criada neste período.')).toBeVisible();
});
