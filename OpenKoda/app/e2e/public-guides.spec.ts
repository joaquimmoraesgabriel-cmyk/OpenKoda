import {expect, test} from '@playwright/test';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {resolve, sep} from 'node:path';
import {fileURLToPath} from 'node:url';
import type {BrowserContext} from '@playwright/test';

const origin='https://openkoda.example.invalid';
const dist=fileURLToPath(new URL('../../site/dist/',import.meta.url));
const routes=['/','/produto/','/download/','/precos/','/suporte/','/seguranca/','/perguntas/','/guias/',
  '/conexoes/','/conexoes/chave-propria/','/conexoes/assinatura/',
  '/guias/primeiro-projeto/','/guias/conversas/','/guias/relatorios/','/guias/uso/','/guias/skills/','/guias/conta/',
  '/exemplos/','/exemplos/revisao-de-api/','/novidades/','/novidades/openkoda-0-2-5/','/novidades/openkoda-0-2-4/','/novidades/openkoda-0-2-3/',
  '/politicas/','/privacidade/','/termos/','/reembolso/','/suboperadores/'];
test.beforeAll(()=>execFileSync(process.execPath,[fileURLToPath(new URL('../../site/build.mjs',import.meta.url))]));
async function serve(context:BrowserContext){
  await context.route('**/*',async route=>{
    const url=new URL(route.request().url());
    expect(url.origin).toBe(origin);
    const file=resolve(dist,'.'+url.pathname+(url.pathname.endsWith('/')?'index.html':''));
    expect(file.startsWith(resolve(dist)+sep)).toBe(true);
    const type=file.endsWith('.html')?'text/html':file.endsWith('.css')?'text/css':file.endsWith('.png')?'image/png':
      file.endsWith('.svg')?'image/svg+xml':'text/javascript';
    await route.fulfill({body:await readFile(file),contentType:type});
  });
}
for(const width of [375,1440]){
  test('guias e demonstração navegáveis em '+width+'px',async({browser},testInfo)=>{
    test.setTimeout(90000);
    const context=await browser.newContext({viewport:{width,height:900},reducedMotion:'reduce'});
    try{
      await serve(context);
      const page=await context.newPage();
      await page.goto(origin+'/');
      await page.getByRole('button',{name:'Entendi'}).click();
      await page.reload();
      await expect(page.getByRole('complementary',{name:'Cookies e armazenamento'})).toBeHidden();
      for(const path of routes){
        await page.goto(origin+path,{waitUntil:'domcontentloaded'});
        await expect(page.getByRole('main').getByRole('heading',{level:1})).toHaveCount(1);
        expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
        await expect(page.locator('img:not([alt])')).toHaveCount(0);
        for(const link of await page.locator('main a[href^="#"]').all()){
          await expect(page.locator(await link.getAttribute('href') as string)).toHaveCount(1);
        }
        if(['/guias/','/guias/relatorios/','/precos/','/novidades/','/privacidade/','/'].includes(path)){
          const filename=(path.split('/').filter(Boolean).at(-1)??'inicio')+'-'+width+'.png';
          await page.screenshot({path:testInfo.outputPath(filename),animations:'disabled'});
        }
      }
      await page.goto(origin+'/');
      const project=page.getByRole('tab',{name:'Projeto',exact:true});
      await project.focus();
      await expect(page.getByRole('heading',{name:'Investigar dois arquivos'})).toBeVisible();
      await page.keyboard.press('ArrowRight');
      await expect(page.getByRole('heading',{name:'Da entrada HTTP ao eval()'})).toBeVisible();
      await page.getByRole('tab',{name:'Chat',exact:true}).click();
      await expect(page.getByRole('heading',{name:'De onde vem a fórmula?'})).toBeVisible();
      await page.getByRole('tab',{name:'IA da assinatura'}).click();
      await expect(page.getByRole('heading',{name:'Acompanhe uma cota na sua conta Koda.'})).toBeVisible();
      await page.getByRole('link',{name:'Entender o uso da assinatura'}).click();
      await expect(page.getByRole('heading',{level:1})).toHaveText('Usar a conexão da assinatura');
      await page.goto(origin+'/');
      await page.locator('#demonstracao').screenshot({path:testInfo.outputPath('demonstracao-'+width+'.png')});
      await page.locator('#conexoes').screenshot({path:testInfo.outputPath('conexoes-'+width+'.png')});
      await page.getByRole('link',{name:'Ver todos os guias',exact:true}).click();
      await page.getByRole('main').getByRole('link',{name:/Ler um relatório/}).click();
      await page.locator('.lesson-index a').last().focus();
      await page.keyboard.press('Enter');
      await expect(page).toHaveURL(/#cobertura$/);
      await page.goto(origin+'/perguntas/');
      const question=page.getByRole('main').locator('summary').first();
      await expect(question).toHaveCSS('list-style-type','none');
      await question.focus();await page.keyboard.press('Enter');
      await expect(page.getByRole('main').locator('details[open] p')).toBeVisible();
      if(width===375){
        const menu=page.getByRole('button',{name:'Menu principal'});await menu.focus();await page.keyboard.press('Enter');
        const sub=page.locator('.nav-group summary').first();await sub.focus();await page.keyboard.press('Enter');
        await expect(page.locator('.nav-flyout a[href="/conexoes/"]')).toBeVisible();
        await page.keyboard.press('Tab');await expect(page.locator('.nav-flyout a[href="/produto/"]')).toBeFocused();
        await page.keyboard.press('Escape');await expect(sub).toBeFocused();await page.keyboard.press('Escape');
        await expect(menu).toBeFocused();
      }else{
        const group=page.locator('.nav-group').first();await group.locator('summary').hover();
        await expect(group.getByRole('link',{name:/Revisões|Exemplos/})).toBeVisible();
        await group.locator('summary').click();await group.locator('summary').click();
        await expect(group.locator('.nav-flyout')).toBeHidden();
      }
      await page.goto(origin+'/precos/');
      await expect(page.locator('.offer-grid article')).toHaveCount(4);
      const table=page.getByRole('region',{name:'Comparação dos quatro planos'});
      await expect(table.getByRole('columnheader',{name:'Ultra',exact:true})).toBeAttached();
      if(width===375){
        await table.focus();await page.keyboard.press('End');await page.keyboard.press('ArrowRight');
        await expect.poll(()=>table.evaluate(el=>el.scrollLeft)).toBeGreaterThan(0);
        expect(await page.evaluate(()=>window.scrollX)).toBe(0);
      }
    }finally{await context.close();}
  });
}
test('conteúdo estático e navegação de leitura funcionam sem JavaScript',async({browser})=>{
  const context=await browser.newContext({javaScriptEnabled:false,viewport:{width:375,height:850}});
  try{
    await serve(context);
    const page=await context.newPage();
    await page.goto(origin+'/');
    await expect(page.getByRole('heading',{name:'Acompanhe uma revisão'})).toBeVisible();
    await expect(page.getByRole('heading',{name:'De onde vem a fórmula?'})).toBeVisible();
    await expect(page.getByRole('heading',{name:'Investigar dois arquivos'})).toBeVisible();
    await expect(page.getByRole('heading',{name:'Acompanhe uma cota na sua conta Koda.'})).toBeVisible();
    await expect(page.getByRole('tab')).toHaveCount(0);
    await page.locator('.nav-group').filter({has:page.locator('summary',{hasText:'Guias'})}).locator('summary').click();
    await page.locator('#primary-nav a[href="/guias/"]').click();
    await page.getByRole('link',{name:/Conversar sobre o código/}).click();
    await expect(page.getByRole('heading',{name:'Compacte uma conversa longa'})).toBeVisible();
  }finally{await context.close();}
});
