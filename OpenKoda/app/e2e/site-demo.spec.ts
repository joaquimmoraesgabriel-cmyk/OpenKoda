import {test,expect} from '@playwright/test';
import {readFile} from 'node:fs/promises';
import {execFileSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

const origin='https://demo-koda.example.invalid';
let site:any;
test.beforeAll(async()=>{
  execFileSync(process.execPath,[fileURLToPath(new URL('../../site/build.mjs',import.meta.url))]);
  const source=await readFile(new URL('../../site/dist/server/index.js',import.meta.url),'utf8');
  site=(await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'))).default;
});
test.beforeEach(async({context})=>{
  await context.route('**/*',async route=>{
    const url=new URL(route.request().url());
    if(url.origin!==origin)throw Error('Origem inesperada: '+url.origin);
    const response=await site.fetch(new Request(url),{});
    await route.fulfill({status:response.status,headers:Object.fromEntries(response.headers),body:Buffer.from(await response.arrayBuffer())});
  });
});
test('demo real carrega em origem isolada e não acessa conta, armazenamento ou rede',async({page})=>{
  const requests:string[]=[];
  page.on('request',r=>requests.push(r.url()));
  await page.goto(origin);
  await page.evaluate(()=>localStorage.setItem('koda-test-private','preservar'));
  const iframe=page.locator('.koda-demo iframe');
  await iframe.scrollIntoViewIfNeeded();
  await expect(iframe).toHaveAttribute('sandbox','allow-scripts allow-forms');
  const frame=await (await iframe.elementHandle())!.contentFrame();
  await expect(frame!.locator('#root')).not.toBeEmpty();
  const isolated=await frame!.evaluate(async()=>{
    let parentDenied=false,cookiesDenied=false,networkDenied=false;
    try{void parent.document.body;}catch{parentDenied=true;}
    try{void document.cookie;}catch{cookiesDenied=true;}
    const privateValue=localStorage.getItem('koda-test-private');
    localStorage.setItem('koda-test-private','demo');
    try{await fetch('/conta/api/status',{method:'POST'});}catch{networkDenied=true;}
    return {parentDenied,cookiesDenied,networkDenied,privateValue};
  });
  expect(isolated).toEqual({parentDenied:true,cookiesDenied:true,networkDenied:true,privateValue:null});
  const formBlocked=await frame!.evaluate(()=>new Promise<boolean>(resolve=>{
    const timeout=setTimeout(()=>resolve(false),1500);
    document.addEventListener('securitypolicyviolation',event=>{
      if(event.violatedDirective==='form-action'){clearTimeout(timeout);resolve(true);}
    },{once:true});
    const form=document.createElement('form');form.method='POST';form.action='/conta/api/account';
    document.body.append(form);HTMLFormElement.prototype.submit.call(form);form.remove();
  }));
  expect(formBlocked).toBe(true);
  expect(await page.evaluate(()=>localStorage.getItem('koda-test-private'))).toBe('preservar');
  expect(requests.filter(url=>url.includes('/conta/api/'))).toHaveLength(0);
  await page.getByRole('button',{name:'Recomeçar',exact:true}).click();
  await expect(page.locator('.kd-status')).toHaveText('Exemplos restaurados.');
  const fresh=await (await iframe.elementHandle())!.contentFrame();
  expect(await fresh!.evaluate(()=>localStorage.getItem('koda-test-private'))).toBeNull();
  expect(await page.evaluate(()=>localStorage.getItem('koda-test-private'))).toBe('preservar');
});
for(const width of [375,390,768,1440]){
 test('janela legível sem alargar página em '+width+'px',async({page},testInfo)=>{
  await page.setViewportSize({width,height:900});
  await page.goto(origin);
  await page.getByRole('button',{name:'Entendi',exact:true}).click();
  const iframe=page.locator('.koda-demo iframe');
  await iframe.scrollIntoViewIfNeeded();
  const frame=await (await iframe.elementHandle())!.contentFrame();
  await expect(frame!.locator('#root')).not.toBeEmpty();
  await expect(page.locator('.kd-reset')).toBeVisible();
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
  expect(await frame!.evaluate(()=>document.documentElement.scrollWidth<=innerWidth+1)).toBe(true);
  if(width<=390){
    await frame!.getByRole('textbox',{name:'Mensagem da conversa',exact:true}).fill('Como conferir o acesso ao pedido?');
    await frame!.getByRole('button',{name:'Enviar mensagem',exact:true}).click();
    await expect(frame!.getByText(/Esta é uma resposta pré-definida da demonstração/)).toBeVisible();
    const conversation=frame!.locator('.desk-conversation');
    expect(await conversation.evaluate(el=>el.scrollWidth<=el.clientWidth+1)).toBe(true);
    expect(await conversation.evaluate(el=>el.scrollLeft)).toBe(0);
    await expect(frame!.getByRole('textbox',{name:'Mensagem da conversa',exact:true})).toBeInViewport();
  }
  if(width===1440){
    const before=await page.evaluate(()=>scrollY);
    await frame!.getByRole('region',{name:'Histórico da conversa',exact:true}).hover();
    await page.mouse.wheel(0,1200);
    await expect.poll(()=>page.evaluate(()=>scrollY)).toBe(before);
    await page.mouse.move(8,400);await page.mouse.wheel(0,500);
    await expect.poll(()=>page.evaluate(()=>scrollY)).toBeGreaterThan(before);
    await iframe.scrollIntoViewIfNeeded();
  }
  await page.locator('.koda-demo').screenshot({path:testInfo.outputPath('real-app-'+width+'.png')});
  await page.getByRole('link',{name:'Pular demonstração',exact:true}).click();
  await expect(page.locator('#depois-da-demonstracao')).toBeFocused();
 });
}
