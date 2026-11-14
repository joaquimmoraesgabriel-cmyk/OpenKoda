import {expect,test} from '@playwright/test';
import {execFileSync} from 'node:child_process';
import {readFile} from 'node:fs/promises';
import {resolve,sep} from 'node:path';
import {fileURLToPath} from 'node:url';

const dist=fileURLToPath(new URL('../../site/dist/',import.meta.url));
const origin='https://aparencia.example.invalid';
test.beforeAll(()=>execFileSync(process.execPath,[fileURLToPath(new URL('../../site/build.mjs',import.meta.url))]));
test('aparência acompanha o sistema, preserva escolha entre páginas e sincroniza abas',async({context,page})=>{
  await context.route(origin+'/**',async route=>{
    const path=new URL(route.request().url()).pathname;
    const file=resolve(dist,'.'+path+(path.endsWith('/')?'index.html':''));
    if(!file.startsWith(resolve(dist)+sep))throw Error('Caminho fora da fixture');
    await route.fulfill({body:await readFile(file),contentType:file.endsWith('.html')?'text/html':file.endsWith('.css')?'text/css':file.endsWith('.js')?'text/javascript':file.endsWith('.png')?'image/png':'image/svg+xml'});
  });
  await page.emulateMedia({colorScheme:'dark'});
  await page.goto(origin+'/precos/');
  const preference=page.getByLabel('Aparência',{exact:true});
  await expect(preference).toHaveValue('system');
  await expect(page.locator('body')).toHaveCSS('background-color','rgb(22, 35, 28)');
  const other=await context.newPage();await other.goto(origin+'/privacidade/');
  const copy=other.getByLabel('Aparência',{exact:true});
  await preference.selectOption('light');
  await expect(copy).toHaveValue('light');
  await page.goto(origin+'/suporte/');
  await expect(preference).toHaveValue('light');
  await expect(page.locator('body')).toHaveCSS('background-color','rgb(250, 251, 246)');
  await copy.selectOption('dark');
  await expect(preference).toHaveValue('dark');
  await page.emulateMedia({colorScheme:'light'});
  await expect(page.locator('body')).toHaveCSS('background-color','rgb(22, 35, 28)');
  await page.reload();
  await expect(preference).toHaveValue('dark');
  await preference.selectOption('system');
  await expect(copy).toHaveValue('system');
  await expect(page.locator('body')).toHaveCSS('background-color','rgb(250, 251, 246)');
});
