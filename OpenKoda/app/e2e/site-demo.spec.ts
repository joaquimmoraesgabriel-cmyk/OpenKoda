import {test,expect} from '@playwright/test';
import {readFile} from 'node:fs/promises';
import {fileURLToPath} from 'node:url';
import {interactiveDemo} from '../../site/demo.mjs';

const origin='https://demonstracao.example.invalid';
const dist=fileURLToPath(new URL('../../site/dist/',import.meta.url));
const requests:string[]=[];
test.beforeEach(async({context,page})=>{
 requests.length=0;
 await context.route('**/*',async route=>{
  const url=new URL(route.request().url());requests.push(url.href);
  if(url.origin!==origin)throw Error('A demonstração tentou acessar uma origem externa: '+url.origin);
  if(url.pathname==='/')return route.fulfill({contentType:'text/html',body:'<!doctype html><html lang="pt-BR"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><meta http-equiv="Content-Security-Policy" content="default-src \'self\'; script-src \'self\'; style-src \'self\'; connect-src \'none\'; object-src \'none\'; form-action \'none\'"><link rel="stylesheet" href="/style.css"><link rel="stylesheet" href="/tema.css"><link rel="stylesheet" href="/demo.css"><title>Demonstração</title><main><h1>Conheça o OpenKoda</h1>'+interactiveDemo+'<section class="inner"><h2>Continue explorando o site</h2><p>O conteúdo abaixo continua fora da demonstração.</p></section><section class="inner"><h2>Mais sobre o produto</h2></section></main><script src="/demo.js" type="module"></script></html>'});
  if(!['/style.css','/tema.css','/demo.css','/demo.js'].includes(url.pathname))throw Error('Requisição inesperada: '+url.pathname);
  await route.fulfill({contentType:url.pathname.endsWith('.css')?'text/css':'text/javascript',body:await readFile(dist+url.pathname)});
 });
 await page.goto(origin);
 await expect(page.locator('[data-koda-demo]')).toHaveAttribute('data-initialized','true');
});

test('cria projeto fictício, conversa e relatório sem acessar a URL digitada',async({page},testInfo)=>{
 const demo=page.locator('[data-koda-demo]');
 await demo.screenshot({path:testInfo.outputPath('demo-desktop.png')});
 await demo.getByRole('button',{name:'Novo projeto',exact:true}).click();
 await demo.getByLabel('Nome do projeto',{exact:true}).fill('Meu projeto <seguro>');
 await demo.getByLabel('Origem do projeto',{exact:true}).selectOption('github');
 await demo.getByLabel('Pasta ou URL de exemplo',{exact:true}).fill('https://github.com/private-company/real-repository');
 await demo.getByRole('button',{name:'Criar projeto de exemplo',exact:true}).click();
 await expect(demo.locator('.kd-header-title strong')).toHaveText('Meu projeto <seguro>');
 await expect(demo.getByText('O que vamos investigar?',{exact:true})).toBeVisible();
 await demo.getByLabel('Mensagem na demonstração',{exact:true}).fill('Explique <img src=x onerror=alert(1)>');
 await demo.getByRole('button',{name:'Enviar mensagem na demonstração'}).click();
 await expect(demo.getByRole('button',{name:'Cancelar resposta de demonstração'})).toBeVisible();
 await expect(demo.getByText('Esta resposta foi preparada para a demonstração.',{exact:false}).last()).toBeVisible();
 await expect(demo.locator('.kd-message-user').last()).toHaveText('Explique <img src=x onerror=alert(1)>');
 await expect(demo.locator('img')).toHaveCount(0);
 await demo.getByRole('button',{name:'Relatório',exact:true}).click();
 await expect(demo.getByRole('heading',{name:'Expressão avaliada sem validação'})).toBeVisible();
 await demo.getByRole('button',{name:'Perguntar sobre esta evidência'}).click();
 await expect(demo.getByLabel('Mensagem na demonstração',{exact:true})).toHaveValue('O que falta conferir antes de corrigir este candidato?');
 expect(requests.every(url=>new URL(url).origin===origin)).toBe(true);
 expect(requests.length).toBe(5);
});

test('cancela e reinicia respostas, mantém conversas e limpa o estado no reset',async({page})=>{
 const demo=page.locator('[data-koda-demo]');
 await demo.getByRole('button',{name:'Nova conversa em Calculadora'}).click();
 await demo.getByLabel('Mensagem na demonstração',{exact:true}).fill('Primeira pergunta');
 await demo.getByLabel('Mensagem na demonstração',{exact:true}).press('Enter');
 await demo.getByRole('button',{name:'Cancelar resposta de demonstração'}).click();
 await expect(demo.locator('.kd-message-assistant').last()).toContainText('interrompida');
 await expect(demo.getByLabel('Mensagem na demonstração',{exact:true})).toBeEnabled();
 await demo.getByLabel('Mensagem na demonstração',{exact:true}).fill('Segunda pergunta');
 await demo.getByLabel('Mensagem na demonstração',{exact:true}).press('Enter');
 await expect(demo.locator('.kd-message-assistant').last()).toContainText('Esta resposta foi preparada para a demonstração.');
 await demo.getByRole('button',{name:'Entender o caminho da fórmula',exact:true}).click();
 await expect(demo.locator('.kd-message-assistant').last()).toContainText('Os dois arquivos deste projeto');
 await demo.getByRole('button',{name:'Recomeçar',exact:true}).click();
 await expect(demo.locator('.kd-chat-link')).toHaveCount(1);
 await expect(demo.locator('.kd-message')).toHaveCount(2);
});

test('explora preferências, uso, suporte e controle de raciocínio por teclado',async({page})=>{
 const demo=page.locator('[data-koda-demo]');
 await demo.getByRole('button',{name:'Configurações',exact:true}).click();
 await demo.getByLabel('Tema da demonstração').selectOption('dark');
 await expect(demo).toHaveAttribute('data-theme','dark');
 await demo.getByRole('tab',{name:'Aparência',exact:true}).focus();
 await page.keyboard.press('ArrowRight');
 await expect(demo.getByRole('tab',{name:'IA e modelos',exact:true})).toBeFocused();
 await demo.getByLabel('Conexão de exemplo').selectOption('own');
 await expect(demo.getByText('Exemplo de assinante com chave própria.',{exact:false})).toBeVisible();
 await demo.getByRole('slider',{name:'Nível de raciocínio da demonstração'}).focus();
 await page.keyboard.press('End');
 await expect(demo.getByRole('slider')).toHaveAttribute('aria-valuetext','Alto');
 await demo.getByRole('tab',{name:'Histórico de uso',exact:true}).click();
 await expect(demo.locator('.kd-chart span')).toHaveCount(21);
 await demo.getByRole('tab',{name:'Relatar um erro',exact:true}).click();
 await demo.getByLabel('Assunto do relato').fill('Teste de exemplo');
 await demo.getByLabel('Descrição do relato').fill('O botão está acessível.');
 await demo.getByRole('button',{name:'Simular envio do relato'}).click();
 await expect(demo.getByText('Nenhum chamado ou e-mail foi enviado.',{exact:false})).toBeVisible();
 await demo.getByRole('button',{name:'Início',exact:true}).click();
 await demo.locator('.kd-model-button').click();
 await expect(demo.getByRole('dialog',{name:'Escolher modelo na demonstração'})).toBeVisible();
 await page.keyboard.press('Escape');
 await expect(demo.getByRole('dialog')).toHaveCount(0);
 await expect(demo.locator('.kd-model-button')).toBeFocused();
 await demo.getByRole('button',{name:'Uso da demonstração',exact:true}).hover();
 await expect(demo.getByRole('region',{name:'Resumo ilustrativo de uso'})).toBeVisible();
 await demo.getByRole('button',{name:'Minha chave',exact:true}).click();
 await expect(demo.getByText('US$ 0,32',{exact:true}).first()).toBeVisible();
 await demo.getByRole('button',{name:'Abrir Histórico de uso',exact:true}).click();
 await expect(demo.getByRole('heading',{name:'Saiba onde foi seu uso'})).toBeVisible();
});

test('limita rolagem ao card e mantém navegação móvel sem overflow horizontal',async({page},testInfo)=>{
 const demo=page.locator('[data-koda-demo]');
 await page.setViewportSize({width:390,height:844});
 await expect(demo.getByRole('button',{name:'Mostrar projetos da demonstração'})).toBeVisible();
 await demo.screenshot({path:testInfo.outputPath('demo-mobile.png')});
 await demo.getByRole('button',{name:'Mostrar projetos da demonstração'}).click();
 await demo.getByRole('button',{name:'Novo projeto',exact:true}).click();
 await expect(demo.getByLabel('Nome do projeto')).toBeVisible();
 await demo.getByRole('button',{name:'Voltar',exact:true}).click();
 await demo.getByRole('button',{name:'Relatório',exact:true}).click();
 const scroll=demo.locator('.kd-scroll');
 await scroll.evaluate(el=>{el.scrollTop=el.scrollHeight;});
 const box=await scroll.boundingBox();
 const before=await page.evaluate(()=>window.scrollY);
 await page.mouse.move(box!.x+box!.width/2,box!.y+box!.height/2);
 await page.mouse.wheel(0,500);
 await page.waitForTimeout(200);
 expect(await page.evaluate(()=>window.scrollY)).toBe(before);
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth)).toBe(true);
 await page.mouse.move(3,500);await page.mouse.wheel(0,500);await page.waitForTimeout(200);
 expect(await page.evaluate(()=>window.scrollY)).toBeGreaterThan(before);
});

test('movimento reduzido apresenta a resposta imediatamente',async({page})=>{
 await page.emulateMedia({reducedMotion:'reduce'});
 const demo=page.locator('[data-koda-demo]');
 await demo.getByLabel('Mensagem na demonstração',{exact:true}).fill('Uma pergunta qualquer.');
 await demo.getByLabel('Mensagem na demonstração',{exact:true}).press('Enter');
 await expect(demo.locator('.kd-message-assistant').last()).toContainText('Esta resposta foi preparada para a demonstração.',{timeout:1000});
 await expect(demo.getByRole('button',{name:'Cancelar resposta de demonstração'})).toHaveCount(0);
});

test('teclado revela abas móveis e preserva foco da conexão sem rolar o site',async({page})=>{
 await page.setViewportSize({width:390,height:844});
 const demo=page.locator('[data-koda-demo]');
 await demo.getByRole('button',{name:'Configurações',exact:true}).click();
 const tabs=demo.getByRole('tablist',{name:'Seções das configurações da demonstração'});
 const appearance=demo.getByRole('tab',{name:'Aparência',exact:true});
 await appearance.focus();
 const pageScroll=await page.evaluate(()=>window.scrollY);
 await page.keyboard.press('End');
 const last=demo.getByRole('tab',{name:'Relatar um erro',exact:true});
 await expect(last).toBeFocused();
 const visibleInside=async()=>{
  const strip=await tabs.boundingBox(),tab=await demo.getByRole('tab',{selected:true}).boundingBox();
  expect(tab!.x).toBeGreaterThanOrEqual(strip!.x);
  expect(tab!.x+tab!.width).toBeLessThanOrEqual(strip!.x+strip!.width);
 };
 await visibleInside();
 expect(await tabs.evaluate(el=>el.scrollLeft)).toBeGreaterThan(0);
 expect(await page.evaluate(()=>window.scrollY)).toBe(pageScroll);
 await page.keyboard.press('Home');
 await expect(appearance).toBeFocused();await visibleInside();
 expect(await tabs.evaluate(el=>el.scrollLeft)).toBe(0);
 await page.keyboard.press('ArrowRight');
 await expect(demo.getByRole('tab',{name:'IA e modelos',exact:true})).toBeFocused();await visibleInside();
 expect(await page.evaluate(()=>window.scrollY)).toBe(pageScroll);
 const connection=demo.getByLabel('Conexão de exemplo',{exact:true});
 await connection.focus();
 await connection.selectOption('own');
 await expect(connection).toBeFocused();
 await expect(connection).toHaveValue('own');
 await page.keyboard.press('Tab');
 await expect(demo.getByLabel('Modelo de exemplo',{exact:true})).toBeFocused();
 expect(await page.evaluate(()=>window.scrollY)).toBe(pageScroll);
});
