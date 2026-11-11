import { Locator, Page } from '@playwright/test';
import { test, expect } from './fixtures';

// 512 CSS px is the available width of a 1024px window at 200% zoom.
// This checks reflow and keyboard behavior, not browser zoom or a screen reader.
async function keyboardActivate(page:Page,control:Locator){
  await control.focus();await expect(control).toBeFocused();
  await expect.poll(()=>control.evaluate(element=>element.matches(':focus-visible')&&getComputedStyle(element).outlineStyle!=='none'&&parseFloat(getComputedStyle(element).outlineWidth)>=2)).toBe(true);
  await page.keyboard.press('Enter');
}
async function checkLayoutAndLabels(page:Page){
  await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1)).toBe(true);
  const unnamed=await page.locator('input:not([type=hidden]),textarea,select,button').evaluateAll(elements=>elements.filter(element=>element.getClientRects().length>0).filter(element=>{
    if(element.getAttribute('aria-label')?.trim()||element.getAttribute('aria-labelledby')?.trim())return false;
    if(element instanceof HTMLButtonElement)return !element.textContent?.trim();
    return !(element as HTMLInputElement).labels?.length;
  }).map(element=>element.outerHTML.slice(0,180)));
  expect(unnamed,'Visible controls need a label or accessible name').toEqual([]);
}

test('seis áreas mantêm reflow equivalente a 200%, rótulos e foco visível por teclado',async({page})=>{
  await page.setViewportSize({width:512,height:384});await page.goto('/?area=tecnica');
  await expect(page.getByRole('heading',{name:'Visão geral',exact:true})).toBeVisible();
  await page.keyboard.press('Tab');await expect(page.getByRole('link',{name:'Ir para o conteúdo',exact:true})).toBeFocused();await page.keyboard.press('Enter');await expect(page.locator('#main-content')).toBeFocused();
  await page.evaluate(async()=>{const moduleUrl='/src/api.ts';const {request}=await import(moduleUrl);const project=await request('project.create',{name:'Projeto reflow',kind:'code',target:'D:/Fixture/reflow',description:'Fixture de interface sem leitura de disco.'});await request('assessment.create',{projectId:project.id,title:'Avaliação reflow',kind:'code',target:project.target,objective:'Verificar os controles em área reduzida.'});});
  await page.reload();await page.keyboard.press('Tab');
  for(const name of ['Avaliações','Achados','Projetos','Kali e ferramentas','Configurações']){
    const navigation=['Avaliações','Achados','Projetos'].includes(name)?page.getByRole('navigation',{name:'Navegação principal',exact:true}):page;
    await keyboardActivate(page,navigation.getByRole('button',{name,exact:true}));await expect(page.getByRole('heading',{name,exact:true})).toBeVisible();await checkLayoutAndLabels(page);
    if(name==='Avaliações'){
      await keyboardActivate(page,page.getByRole('button').filter({hasText:'Avaliação reflow'}));await expect(page.getByRole('heading',{name:'Avaliação reflow',exact:true})).toBeVisible();await checkLayoutAndLabels(page);
    }
    if(name==='Projetos'){
      await keyboardActivate(page,page.getByRole('button',{name:'Novo projeto',exact:true}));await expect(page.getByLabel('Nome do projeto',{exact:true})).toBeVisible();await checkLayoutAndLabels(page);
    }
    if(name==='Configurações'){
      await keyboardActivate(page,page.getByRole('button',{name:'IA e modelos',exact:true}));await expect(page.getByLabel('Endpoint',{exact:true})).toBeVisible();await keyboardActivate(page,page.getByRole('button',{name:'Dados e privacidade',exact:true}));await checkLayoutAndLabels(page);
      await page.screenshot({path:test.info().outputPath('reflow-200-configuracoes.png'),fullPage:true});
    }
  }
  await keyboardActivate(page,page.getByRole('button',{name:'Nova avaliação',exact:true}));await expect(page.getByRole('heading',{name:'Nova avaliação',exact:true})).toBeVisible();await expect(page.getByLabel('Objetivo',{exact:true})).toBeVisible();await checkLayoutAndLabels(page);
  await page.getByLabel('Título',{exact:true}).focus();await page.keyboard.type('Avaliação por teclado');await page.keyboard.press('Tab');await expect(page.getByLabel('Alvo',{exact:true})).toBeFocused();
  await page.screenshot({path:test.info().outputPath('reflow-200-nova-avaliacao.png'),fullPage:true});
});
