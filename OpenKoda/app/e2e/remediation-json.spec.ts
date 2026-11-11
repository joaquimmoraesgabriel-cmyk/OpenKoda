import { test, expect } from './fixtures';
import { combinedCalls, installCombinedFixture } from './combined-fixture';

test('reteste JSON fixa o booleano debug e mantém o achado pendente de confirmação humana', async ({ page }) => {
  await installCombinedFixture(page, 'secondary-remediation');
  await page.getByRole('navigation', { name: 'Navegação principal' }).getByRole('button', { name: 'Achados', exact: true }).click();
  await page.getByText('Achado da fonte secundária', { exact: true }).click();
  await page.getByRole('navigation', { name: 'Seções' }).getByRole('button', { name: 'Correção' }).click();
  await page.getByRole('button', { name: 'Preparar correção local' }).click();
  await page.getByLabel('Entrada da correção local', { exact: true }).selectOption('combined-a-offline');
  await page.getByLabel('Caminho relativo do arquivo', { exact: true }).fill('offline.txt');
  await page.getByLabel('Conteúdo proposto', { exact: true }).fill('{"debug":false}');
  const textPredicate = page.getByLabel('Texto da condição', { exact: true });
  await textPredicate.fill('condição textual anterior');
  await page.getByLabel('Condição do reteste', { exact: true }).selectOption('json.debug-disabled.v1');
  await expect(textPredicate).toHaveCount(0);
  await expect(page.getByText(/A base deve ter/)).toContainText('na raiz do objeto JSON');

  await page.getByRole('button', { name: 'Preparar revisão da alteração' }).click();
  await expect(page.getByLabel('Condição revisada do reteste', { exact: true })).toHaveText('false');
  const prepared = await combinedCalls(page, 'remediation.prepare');
  expect(prepared).toHaveLength(1);
  expect(prepared[0].payload).toMatchObject({
    retest: { kind: 'json.debug-disabled.v1', value: 'false' },
    after: '{"debug":false}', relativePath: 'offline.txt',
    targetRef: { entryId: 'combined-a-offline', entryDigest: 'digest-combined-a-offline' },
  });
  const approve = page.getByRole('button', { name: 'Aprovar alteração revisada' });
  await expect(approve).toBeDisabled();
  await page.getByRole('button', { name: 'Revisar trecho armazenado' }).click();
  await page.getByRole('button', { name: 'Confirmar leitura desta janela' }).click();
  await page.getByRole('checkbox', { name: /Revisei o arquivo, o conteúdo antes e depois/ }).check();
  await approve.click();
  expect(await combinedCalls(page, 'remediation.apply')).toHaveLength(0);
  await page.getByRole('button', { name: 'Aplicar alteração aprovada' }).click();
  const retest = page.getByRole('button', { name: 'Verificar o campo debug no JSON', exact: true });
  await expect(retest).toBeEnabled();
  expect(await combinedCalls(page, 'remediation.retest')).toHaveLength(0);
  await retest.click();
  await expect(page.getByRole('heading', { name: 'Resultado do reteste' })).toBeVisible();
  await expect(page.getByText('Atendida', { exact: true })).toBeVisible();
  await expect(page.getByText('json-retest-proof', { exact: true })).toBeVisible();
  const executed = await combinedCalls(page, 'remediation.retest');
  expect(executed).toHaveLength(1);
  expect(executed[0].payload).toEqual({ id: 'secondary-remediation-plan', digest: 'remediation-digest' });
  await expect(page.locator('.detail-heading').getByText('Validado', { exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Confirmar correção revisada' })).toBeDisabled();
  expect(await combinedCalls(page, 'remediation.confirm')).toHaveLength(0);
});
