import type { Page } from '@playwright/test';

export function openAssessmentList(page: Page) {
  return page.getByRole('navigation', { name: 'Navegação principal', exact: true })
    .getByRole('button', { name: 'Avaliações', exact: true }).click();
}
