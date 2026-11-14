import {expect,test,type BrowserContext} from '@playwright/test';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {resolve,sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const dist=fileURLToPath(new URL('../../site/dist/',import.meta.url));
const origin='https://guias.example.invalid';
test.describe.configure({mode:'serial'});
async function serve(context:BrowserContext){
 await context.route(origin+'/**',async route=>{
  const path=new URL(route.request().url()).pathname;
  const file=resolve(dist,'.'+path+(path.endsWith('/')?'index.html':''));
  if(!file.startsWith(resolve(dist)+sep))throw Error('Fixture path outside site');
  await route.fulfill({body:await readFile(file),contentType:file.endsWith('.html')?'text/html':file.endsWith('.css')?'text/css':file.endsWith('.js')?'text/javascript':file.endsWith('.png')?'image/png':'image/svg+xml'});
 });
}
test.beforeAll(()=>execFileSync(process.execPath,[fileURLToPath(new URL('../../site/build.mjs',import.meta.url))]));
test('guias mostram capturas e exemplos interativos sem serviços externos',async({context,page})=>{
 await serve(context);await page.emulateMedia({reducedMotion:'reduce'});
 const errors:string[]=[],external:string[]=[];
 page.on('pageerror',error=>errors.push(error.message));
 page.on('request',request=>{if(!request.url().startsWith(origin))external.push(request.url());});
 await page.goto(origin+'/guias/primeiro-projeto/');
 const figure=page.locator('.guide-figure');
 await expect(figure.locator('img')).toHaveAttribute('alt',/criação de projeto/);
 await expect.poll(()=>figure.locator('img').evaluate((image:HTMLImageElement)=>image.naturalWidth)).toBe(1280);
 const scope=page.locator('[data-scope-example]');
 await scope.getByRole('button',{name:'Uma pasta',exact:true}).click();
 await expect(scope.locator('[data-scope-file="worker"]')).toContainText('Incluído');
 await scope.getByRole('button',{name:'Um arquivo',exact:true}).click();
 await expect(scope.locator('[data-scope-file="worker"]')).toContainText('Fora do escopo');
 await page.goto(origin+'/guias/uso/');
 await expect(page.locator('.guide-figure img')).toHaveAttribute('src','/uso.png');
 const range=page.getByLabel('Uso ilustrativo nesta janela');
 await range.fill('100');
 await expect(page.locator('[data-quota-left]')).toHaveText('0%');
 await expect(page.locator('[data-quota-note]')).toContainText('terminou');
 await range.fill('20');
 await expect(page.locator('[data-quota-left]')).toHaveText('80%');
 await page.goto(origin+'/conexoes/');
 const own=page.locator('[data-route-example="propria"]');
 await own.getByRole('button',{name:'Simular um envio'}).click();
 await expect(own.getByRole('status')).toContainText('provedor fatura sua conta');
 await page.getByRole('tab',{name:'IA da assinatura',exact:true}).click();
 const included=page.locator('[data-route-example="assinatura"]');
 await included.getByRole('button',{name:'Simular um envio'}).click();
 await expect(included.getByRole('status')).toContainText('Uso e limites');
 expect(errors).toEqual([]);expect(external).toEqual([]);
});
test('texto permanece legível e a revisão acompanha a rolagem sem capturá-la',async({context,page})=>{
 await serve(context);await page.emulateMedia({reducedMotion:'no-preference'});
 await page.goto(origin+'/guias/primeiro-projeto/');
 const last=page.locator('#revisao');
 await expect(last).toHaveCSS('opacity','1');
 await last.scrollIntoViewIfNeeded();
 await expect(last.getByRole('heading',{name:'Inicie e acompanhe'})).toBeVisible();
 await expect(page.locator('.letter-piece,.motion-pending')).toHaveCount(0);
 const link=page.locator('.next-guide a');
 await link.focus();
 await expect(link).toBeFocused();
 await page.goto(origin+'/guias/relatorios/#cobertura');
 await expect(page.locator('#cobertura')).toHaveCSS('opacity','1');
 await page.setViewportSize({width:1440,height:1000});
 await page.goto(origin+'/produto/');
 const review=page.locator('[data-review-story]');
 const third=review.locator('[data-review-step="2"]');
 await third.scrollIntoViewIfNeeded();
 await expect(review).toHaveAttribute('data-review-current','2');
 await review.getByRole('button',{name:'Projeto',exact:true}).click();
 await expect(review.locator('[data-review-capture="0"]')).toBeVisible();
 await expect(review.locator('[data-review-capture="2"]')).toBeHidden();
 await page.emulateMedia({reducedMotion:'reduce'});
 await review.getByRole('button',{name:'Relatório',exact:true}).click();
 await expect(review.locator('[data-review-capture="1"]')).toBeVisible();
 await expect(review.locator('.review-visual')).toHaveCSS('position','static');
});
test('guias continuam legíveis sem JavaScript e cabem no celular',async({browser,context,page})=>{
 const noJs=await browser.newContext({javaScriptEnabled:false,viewport:{width:390,height:844}});
 await serve(noJs);const plain=await noJs.newPage();
 await plain.goto(origin+'/guias/relatorios/');
 await expect(plain.locator('#cobertura')).toHaveCSS('opacity','1');
 await expect(plain.locator('.guide-figure img')).toHaveAttribute('alt',/Relatório/);
 await expect(plain.locator('.motion-pending')).toHaveCount(0);
 await noJs.close();
 await serve(context);await page.setViewportSize({width:375,height:812});
 await page.emulateMedia({reducedMotion:'reduce'});
 for(const route of ['guias/primeiro-projeto','guias/uso','guias/aparencia','conexoes']){
  await page.goto(origin+'/'+route+'/');
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
  const text=page.locator('.lesson-section p,.connection-scene p').first();
  expect(await text.evaluate(el=>parseFloat(getComputedStyle(el).fontSize))).toBeGreaterThanOrEqual(17);
 }
 await page.screenshot({path:'.local/site-guide-mobile.png',fullPage:true,animations:'disabled'});
});

test('apresentação mantém leitura e controles nas três larguras',async({context,page})=>{
 await serve(context);await page.emulateMedia({reducedMotion:'reduce'});
 for(const width of [390,768,1440]){
  await page.setViewportSize({width,height:1000});
  for(const route of ['produto','precos','guias/relatorios','conexoes']){
   await page.goto(origin+'/'+route+'/');
   expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
   await expect(page.locator('h1')).toBeVisible();
  }
  await page.goto(origin+'/produto/');
  const skip=page.getByRole('link',{name:'Pular para o conteúdo',exact:true});
  await expect(skip).toHaveCSS('clip-path','inset(50%)');
  await page.keyboard.press('Tab');
  await expect(skip).toBeFocused();
  await expect(skip).toHaveCSS('clip-path','none');
  const report=page.getByRole('button',{name:'Relatório',exact:true});
  await report.focus();await page.keyboard.press('Enter');
  await expect(skip).toHaveCSS('clip-path','inset(50%)');
  await expect(report).toHaveAttribute('aria-pressed','true');
  const cookies=page.getByRole('button',{name:'Entendi',exact:true});
  if(await cookies.isVisible())await cookies.click();
  const image=page.locator('.screen-print img');
  await image.scrollIntoViewIfNeeded();
  await expect.poll(()=>image.evaluate((node:HTMLImageElement)=>node.naturalWidth)).toBeGreaterThan(0);
  await page.screenshot({path:`.local/public-product-${width}.png`,fullPage:true,animations:'disabled'});
 }
});

test('novo site apresenta produto, comparação e guias com navegação por teclado',async({context,page})=>{
 await serve(context);await page.emulateMedia({reducedMotion:'reduce'});
 const errors:string[]=[];page.on('pageerror',error=>errors.push(error.message));
 for(const width of [1440,390]){
  await page.setViewportSize({width,height:1000});
  for(const [route,name] of [['','home'],['precos/','precos'],['guias/primeiro-projeto/','guia']]){
   await page.goto(origin+'/'+route);
   const cookies=page.getByRole('button',{name:'Entendi',exact:true});
   if(await cookies.isVisible())await cookies.click();
   if(name==='home'){
    await expect(page.locator('h1')).toContainText('Abra o projeto.');
    const annotation=page.getByRole('button',{name:'Examinar esta linha'});
    await annotation.click();await expect(page.locator('#evidence-explanation')).toBeVisible();
    await page.getByRole('button',{name:'Fechar explicação'}).click();
    await page.getByLabel('Mude o consumo do exemplo').fill('100');
    await expect(page.locator('.home-quota [data-quota-left]')).toHaveText('0%');
    await page.getByLabel('Mude o consumo do exemplo').fill('30');
   }
   for(const image of await page.locator('main img').all()){
    if(!await image.isVisible())continue;
    await image.scrollIntoViewIfNeeded();
    await expect.poll(()=>image.evaluate((node:HTMLImageElement)=>node.naturalWidth)).toBeGreaterThan(0);
   }
   expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
   await page.evaluate(()=>window.scrollTo(0,0));
   await page.screenshot({path:`.local/visual-rebuild-${name}-${width}.png`,fullPage:true,animations:'disabled'});
   if(name==='home'&&width===1440)await page.screenshot({path:'.local/visual-rebuild-home-1440-viewport.png',animations:'disabled'});
  }
  await page.goto(origin+'/');
  if(width===390)await page.getByRole('button',{name:'Menu principal',exact:true}).click();
  const summary=page.locator('#primary-nav summary').first();
  await summary.focus();await page.keyboard.press('Enter');
  await expect(page.locator('#primary-nav .nav-group').first()).toHaveAttribute('open','');
  await page.screenshot({path:`.local/visual-rebuild-menu-${width}.png`,animations:'disabled'});
  await page.keyboard.press('Escape');await expect(summary).toBeFocused();
  await expect(page.locator('#primary-nav .nav-group').first()).not.toHaveAttribute('open','');
 }
 expect(errors).toEqual([]);
});
