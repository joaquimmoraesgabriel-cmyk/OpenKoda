import type { Locator, Page } from '@playwright/test';
import { expect, test } from './fixtures';

async function checkIndependentScroll(page: Page, menu: Locator, detail: Locator) {
  const position = (element: Locator) => element.evaluate(node => node.scrollTop);
  const top = (element: Locator) => element.evaluate(node => node.getBoundingClientRect().top);
  const menuPosition = await position(menu);
  const menuTop = await top(menu);
  const contentTop = await top(detail);
  await detail.hover();
  await page.mouse.wheel(0, 520);
  await expect.poll(() => position(detail)).toBeGreaterThan(0);
  expect(await position(menu)).toBe(menuPosition);
  expect(await top(menu)).toBe(menuTop);
  // Finish the first gesture before changing the pointer's scroll target.
  await detail.evaluate(el => el.scrollTo(0, el.scrollHeight));
  const detailPosition = await position(detail);
  await menu.hover();
  await page.mouse.wheel(0, 540);
  await expect.poll(() => position(menu)).toBeGreaterThan(menuPosition);
  expect(await position(detail)).toBe(detailPosition);
  expect(await top(detail)).toBe(contentTop);
  expect(await position(page.getByRole('main'))).toBe(0);
}

test('as categorias e o conteúdo das configurações rolam de forma independente', async ({page}) => {
  await page.setViewportSize({width: 1280, height: 580});
  await page.goto('/');
  await page.getByRole('button', {name:'Configurações', exact:true}).click();
  await checkIndependentScroll(page, page.locator('.settings-menu'), page.getByRole('region', {name:'Conteúdo de Aparência'}));
  await page.getByRole('button', {name:'Atualizações', exact:true}).click();
  await expect(page.getByRole('heading', {name:'Atualizações', exact:true})).toBeInViewport();
});

test('as listas de avaliações e achados mantêm a posição enquanto se lê um registro', async ({page}) => {
  await page.setViewportSize({width: 1280, height: 560});
  await page.goto('/');
  await page.evaluate(async () => {
    const api = '/src/api.ts';
    const {request} = await import(api);
    const project = await request('project.create', {name:'Laboratório', kind:'web', target:'https://lab.example'});
    for (let i = 1; i <= 20; i++) {
      await request('assessment.create', {projectId:project.id, title:'Avaliação ' + i, kind:project.kind, target:project.target, objective:'Conferir o painel.'});
    }
    const key = 'openkoda.preview.v1';
    const state = JSON.parse(localStorage.getItem(key)!);
    state.findings = state.assessments.map((a: {id:string; createdAt:string}, i:number) => ({
      id:crypto.randomUUID(), assessmentId:a.id, title:'Achado ' + i, createdAt:a.createdAt, severity:'low',
      confidence:'medium', status:'validated', description:'Descrição do achado. '.repeat(150), recommendation:'Revisar.', evidenceIds:[]
    }));
    localStorage.setItem(key, JSON.stringify(state));
  });
  await page.reload();
  for (const area of ['Avaliações', 'Achados']) {
    await page.getByRole('navigation', {name:'Navegação principal'}).getByRole('button', {name:area, exact:true}).click();
    const list = page.locator('.list-pane');
    await list.locator('.list-item').first().click();
    await checkIndependentScroll(page, list, page.locator('.detail-pane'));
  }
});
