import type { Page } from '@playwright/test';

export async function openAssessmentList(page: Page) {
  if (new URL(page.url()).searchParams.get('area')!=='tecnica') {
    await page.getByRole('button',{name:'Configurações',exact:true}).click();
    await page.getByRole('button',{name:'Abrir área técnica',exact:true}).click();
  }
  return page.getByRole('navigation', { name: 'Navegação principal', exact: true })
    .getByRole('button', { name: 'Avaliações', exact: true }).click();
}
