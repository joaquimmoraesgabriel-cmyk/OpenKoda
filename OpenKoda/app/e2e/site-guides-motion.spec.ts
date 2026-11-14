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
 await expect(included.getByRole('status')).toContainText('Plano e uso');
 expect(errors).toEqual([]);expect(external).toEqual([]);
});
test('movimento revela conteúdo ao rolar e ao focar, sem alterar nomes acessíveis',async({context,page})=>{
 await serve(context);await page.emulateMedia({reducedMotion:'no-preference'});
 await page.goto(origin+'/guias/primeiro-projeto/');
 const last=page.locator('#revisao');
 await expect(last).toHaveClass(/motion-pending/);
 await expect(last).toHaveCSS('opacity','0');
 await last.scrollIntoViewIfNeeded();
 await expect(last).not.toHaveClass(/motion-pending/);
 await expect(last).toHaveCSS('opacity','1');
 await expect(last.getByRole('heading',{name:'Inicie e acompanhe'})).toBeVisible();
 await expect(last.locator('.letter-piece').first()).toBeVisible();
 await page.reload();
 const link=page.locator('.next-guide a');
 await link.focus();
 await expect(link).toBeFocused();
 await page.goto(origin+'/guias/relatorios/#cobertura');
 await expect(page.locator('#cobertura')).not.toHaveClass(/motion-pending/);
 await page.emulateMedia({reducedMotion:'reduce'});
 await expect(page.locator('.motion-pending')).toHaveCount(0);
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
