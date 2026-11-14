import {expect,test} from './fixtures';
import type { Page } from '@playwright/test';
import {emptySnapshot} from '../src/preview';

async function desktop(page:Page,provider='openrouter',subscribed=false){
  const snapshot=emptySnapshot(),at='2026-11-14T08:00:00Z';
  snapshot.settings={...snapshot.settings,provider,model:'example/model',hasApiKey:true};
  snapshot.projects=[{id:'fixture',kind:'code',name:'Projeto de teste',target:'D:/Fixture',description:'',createdAt:at}];
  snapshot.assessments=[{id:'chat',projectId:'fixture',kind:'code',title:'Conversa de teste',target:'D:/Fixture',objective:'',createdAt:at,updatedAt:at,status:'draft',coverage:[],evidence:[],events:[],summary:'',aiSelection:{provider:provider as 'openrouter',model:'example/model'}}];
  await page.addInitScript(({snapshot,subscribed})=>{
    const date=new Date(),pad=(n:number)=>String(n).padStart(2,'0'),today=[date.getFullYear(),pad(date.getMonth()+1),pad(date.getDate())].join('-');
    const bridge={overviewCalls:0,overviewFailure:false,budget:{enabled:true,degraded:false,unknownCharges:[],unknownChargesTotal:0,installation:{limitUsd:'10',pricedUsageUsd:'8',heldUsd:'1',remainingUsd:'1'},recentCharges:[{model:'example/model',endpoint:'https://openrouter.ai/api/v1',pricedUsageUsd:'0.025'}],policy:{prices:[]}},
      account:subscribed?{plan:{id:'plus'},subscription:{status:'active',mode:'production',periodEnd:new Date(Date.now()+86400000).toISOString()},usage:{monthlyLimitMicros:20000000,monthlyUsedMicros:1000000,windowLimitMicros:5000000,windowUsedMicros:500000}}:null};
    Object.assign(window,{usageFixture:bridge,isTauri:true,__TAURI_INTERNALS__:{invoke:async(_cmd:string,args:{request:{action:string;payload:Record<string,unknown>}})=>{
      const {action,payload}=args.request;
      switch(action){
        case 'snapshot':return snapshot;
        case 'ai.history':return {messages:[],requests:[]};
        case 'ai.chat.status':return {requestId:null,stopping:false};
        case 'security.scan.get':return {scan:null};
        case 'security.status':return {available:false};
        case 'ai.budget.status':return bridge.budget;
        case 'ai.budget.history':return {rows:[{day:today,provider:'openrouter',model:'example/model',assessmentId:'chat',requests:2,input:120,output:40,partial:0}]};
        case 'ai.status':return {hasApiKey:true,provider:payload.provider,enabledModels:['example/model']};
        case 'account.session':return {user:subscribed?{id:'fixture',email:'fixture@example.invalid'}:null};
        case 'account.overview':bridge.overviewCalls++;if(bridge.overviewFailure)throw new Error('Servidor temporariamente indisponível');return bridge.account;
        default:return {};
      }
    }}});
  },{snapshot,subscribed});
  await page.goto('/');
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toBeVisible();
}

test('barra compacta, divisor por arrasto e teclado preservam largura e conversa',async({page})=>{
  await desktop(page);
  await page.getByRole('textbox',{name:'Mensagem da conversa'}).fill('Rascunho preservado');
  const nav=page.getByRole('navigation',{name:'Navegação principal'});
  expect((await nav.boundingBox())!.width).toBeLessThan(65);
  const settings=nav.getByRole('button',{name:'Configurações',exact:true});await settings.hover();
  await expect(settings.locator('.rail-hint')).toBeVisible();
  const divider=page.getByRole('separator',{name:'Largura da lista de projetos'});
  const box=(await divider.boundingBox())!;
  await page.mouse.move(box.x+2,box.y+80);await page.mouse.down();await page.mouse.move(box.x+82,box.y+80,{steps:7});await page.mouse.up();
  await expect(divider).toHaveAttribute('aria-valuenow','320');
  await divider.press('ArrowLeft');await expect(divider).toHaveAttribute('aria-valuenow','304');
  await expect(page.getByRole('textbox',{name:'Mensagem da conversa'})).toHaveValue('Rascunho preservado');
  await page.reload();await expect(divider).toHaveAttribute('aria-valuenow','304');
  await divider.press('Home');await expect(divider).toHaveAttribute('aria-valuenow','185');
  await divider.press('End');await expect(divider).toHaveAttribute('aria-valuenow','420');
  await page.setViewportSize({width:390,height:800});await expect(divider).toBeHidden();
  await expect.poll(()=>page.evaluate(()=>document.documentElement.scrollWidth<=window.innerWidth+1)).toBe(true);
  await page.getByRole('button',{name:'Projetos',exact:true}).click();
  await expect(page.getByRole('complementary',{name:'Projetos e conversas'})).toBeVisible();
});

test('anel usa orçamento real, filtra provedores e abre histórico uma vez',async({page},testInfo)=>{
  await desktop(page);
  const button=page.getByRole('button',{name:'Abrir histórico de uso',exact:true});
  await expect(button).toHaveAttribute('data-low','true');await button.hover();
  const popup=page.getByRole('region',{name:'Seu uso de IA'});
  await expect(popup).toBeVisible();await expect(popup.getByText('10% disponível')).toBeVisible();
  await expect(popup.locator('.usage-mini-calendar [role=img]')).toHaveCount(21);
  const cell=await popup.locator('.usage-mini-calendar [role=img]').first().boundingBox();expect(Math.abs(cell!.width-cell!.height)).toBeLessThan(1);
  await expect(popup.getByRole('row',{name:/example\/model.*2/})).toBeVisible();
  await page.screenshot({path:testInfo.outputPath('usage-ring-desktop.png')});
  await popup.getByRole('button',{name:'OpenAI',exact:true}).click();
  await expect(popup.getByText('Ainda não há chamadas registradas para este provedor.')).toBeVisible();
  const strip=popup.getByRole('group',{name:'Provedores do resumo de uso'}),stripBox=(await strip.boundingBox())!;
  await page.mouse.move(stripBox.x+230,stripBox.y+14);await page.mouse.down();await page.mouse.move(stripBox.x+80,stripBox.y+14,{steps:8});await page.mouse.up();
  expect(await strip.evaluate(el=>el.scrollLeft)).toBeGreaterThan(80);
  await button.focus();await page.keyboard.press('Escape');await expect(popup).toBeHidden();
  await button.click();await expect(page.getByRole('heading',{name:'Histórico de uso',exact:true})).toBeVisible();
});

test('assinatura separa cota e chave pessoal, desconhecido nunca dispara alerta',async({page})=>{
  await desktop(page,'openkoda',true);
  const button=page.getByRole('button',{name:'Abrir histórico de uso',exact:true});
  await expect(button).toHaveAttribute('data-low','false');await button.hover();
  const popup=page.getByRole('region',{name:'Seu uso de IA'});
  await expect(popup.getByText('90% disponível')).toBeVisible();
  await popup.getByRole('button',{name:'OpenRouter',exact:true}).click();
  await expect(popup.getByText('10% disponível')).toBeVisible();
  await expect(page.locator('html')).toHaveAttribute('data-usage-low','false');
  await page.evaluate(()=>{const f=Reflect.get(window,'usageFixture');f.account.usage.windowUsedMicros=4750000;window.dispatchEvent(new Event('openkoda-budget-changed'));});
  await expect(button).toHaveAttribute('data-low','true');
  await page.emulateMedia({reducedMotion:'reduce'});
  expect(await page.locator('.project-composer').evaluate(el=>getComputedStyle(el,'::after').animationName)).toBe('none');
  await page.evaluate(()=>{const f=Reflect.get(window,'usageFixture');f.account.subscription.mode='mock';window.dispatchEvent(new Event('openkoda-budget-changed'));});
  await expect(button).toHaveAttribute('data-known','false');await expect(button).toHaveAttribute('data-low','false');
  await page.setViewportSize({width:390,height:800});
  await page.evaluate(()=>{document.documentElement.style.zoom='1.25';});
  await button.focus();
  await expect.poll(async()=>{const b=await popup.boundingBox();return !!b&&b.x>=0&&b.y>=0&&b.x+b.width<=391&&b.y+b.height<=801;}).toBe(true);
});


test('janela de assinatura renova sem hover e retorno de suspensão revalida a cota',async({page})=>{
  await page.clock.install();
  await desktop(page,'openkoda',true);
  const ring=page.getByRole('button',{name:'Abrir histórico de uso',exact:true});
  await expect(ring).toHaveAttribute('data-known','true');
  await page.evaluate(()=>{
    const f=Reflect.get(window,'usageFixture');f.account.usage.windowUsedMicros=4750000;
    f.account.usage.windowEnd=new Date(Date.now()+60000).toISOString();
    window.dispatchEvent(new Event('openkoda-budget-changed'));
  });
  await expect(ring).toHaveAttribute('data-low','true');
  const before=await page.evaluate(()=>Reflect.get(window,'usageFixture').overviewCalls);
  await page.evaluate(()=>{const f=Reflect.get(window,'usageFixture');f.account.usage.windowUsedMicros=0;f.account.usage.windowEnd=new Date(Date.now()+604800000).toISOString();});
  await page.clock.fastForward(60001);
  await expect(ring).toHaveAttribute('data-low','false');
  await expect(ring).toHaveAttribute('data-known','true');
  expect(await page.evaluate(()=>Reflect.get(window,'usageFixture').overviewCalls)).toBe(before+1);
  await page.evaluate(()=>{
    const f=Reflect.get(window,'usageFixture');f.account.usage.windowUsedMicros=4750000;
    f.account.usage.windowEnd=new Date(Date.now()+60000).toISOString();
    window.dispatchEvent(new Event('openkoda-budget-changed'));
  });
  await expect(ring).toHaveAttribute('data-low','true');
  const wakeAt=await page.evaluate(()=>Date.now()+60001);
  await page.clock.setSystemTime(wakeAt);
  await page.evaluate(()=>{const f=Reflect.get(window,'usageFixture');f.account.usage.windowUsedMicros=0;f.account.usage.windowEnd=new Date(Date.now()+604800000).toISOString();window.dispatchEvent(new Event('focus'));});
  await expect(ring).toHaveAttribute('data-low','false');
});

test('janela vencida ou falha no refresh fica desconhecida sem repetição automática',async({page})=>{
  await page.clock.install();
  await desktop(page,'openkoda',true);
  const ring=page.getByRole('button',{name:'Abrir histórico de uso',exact:true});
  await expect(ring).toHaveAttribute('data-known','true');
  await page.evaluate(()=>{const f=Reflect.get(window,'usageFixture');f.account.usage.windowUsedMicros=4750000;f.account.usage.windowEnd=new Date(Date.now()+60000).toISOString();window.dispatchEvent(new Event('openkoda-budget-changed'));});
  await expect(ring).toHaveAttribute('data-low','true');
  const before=await page.evaluate(()=>Reflect.get(window,'usageFixture').overviewCalls);
  await page.clock.fastForward(60001);
  await expect(ring).toHaveAttribute('data-known','false');
  await page.clock.fastForward(3600000);
  expect(await page.evaluate(()=>Reflect.get(window,'usageFixture').overviewCalls)).toBe(before+1);
  await page.evaluate(()=>{const f=Reflect.get(window,'usageFixture');f.account.usage.windowEnd=new Date(Date.now()+60000).toISOString();window.dispatchEvent(new Event('focus'));});
  await expect(ring).toHaveAttribute('data-known','true');
  await page.evaluate(()=>{Reflect.get(window,'usageFixture').overviewFailure=true;});
  const beforeFailure=await page.evaluate(()=>Reflect.get(window,'usageFixture').overviewCalls);
  await page.clock.fastForward(60001);
  await expect(ring).toHaveAttribute('data-known','false');
  await page.clock.fastForward(3600000);
  expect(await page.evaluate(()=>Reflect.get(window,'usageFixture').overviewCalls)).toBe(beforeFailure+1);
});
