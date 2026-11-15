import {test,expect} from './fixtures';
import {emptySnapshot} from '../src/preview';
import type {Page} from '@playwright/test';

// Renderer contract only: every IPC reply is synthetic. These tests neither
// authenticate, purchase a plan, fetch a release nor install an executable.
async function setup(page:Page,enabled=true,ready=false){
  await page.addInitScript(({snapshot,enabled,ready})=>{
    const calls:{command:string;input?:Record<string,unknown>;action?:string}[]=[];
    let state={phase:ready?'ready':'current',currentVersion:'1.1.2',version:ready?'1.1.3':null,notes:null,
      downloadedBytes:0,totalBytes:null,message:'Estado sintético do atualizador.',deferred:false,backupPath:null,
      channel:'stable',experimentalOptIn:false,channelsAvailable:enabled};
    Object.assign(window,{isTauri:true,updaterCalls:calls,__TAURI_INTERNALS__:{invoke:async(command:string,args:any)=>{
      if(command==='desktop_update'){
        const input=args.input;calls.push({command,input});
        if(input.action==='channel')state={...state,channel:input.channel,experimentalOptIn:input.experimentalOptIn,phase:'idle',version:null};
        if(input.action==='check'&&state.channel!=='stable')state={...state,phase:'error',message:'Este canal não está disponível para sua assinatura.'};
        return {...state};
      }
      if(command!=='request')return null;
      const {action}=args.request;calls.push({command,action});
      if(action==='snapshot')return snapshot;
      if(action==='account.status')return {state:'signedOut',user:null,account:null};
      if(action==='ai.budget.status')return {enabled:false,degraded:false};
      if(action==='ai.budget.history')return {rows:[]};
      if(action==='ai.status')return {hasApiKey:false,enabledModels:[]};
      return {};
    }}});
  },{snapshot:emptySnapshot(),enabled,ready});
  await page.goto('/');
  await page.getByRole('button',{name:'Configurações',exact:true}).click();
  await page.getByRole('button',{name:'Atualizações',exact:true}).click();
  return page.locator('.desktop-update-settings');
}
test('canal experimental requer opt-in e não altera assinatura nem envia credenciais',async({page})=>{
  const settings=await setup(page);
  await settings.getByLabel('Versões que deseja receber').selectOption('experimental');
  const choose=settings.getByRole('button',{name:'Usar este canal'});
  await expect(choose).toBeDisabled();
  await settings.getByRole('checkbox').check();
  await choose.click();
  await settings.getByRole('button',{name:'Verificar atualizações'}).click();
  await expect(settings.getByRole('status')).toHaveText('Este canal não está disponível para sua assinatura.');
  const calls=await page.evaluate(()=>Reflect.get(window,'updaterCalls'));
  expect(calls.filter((c:any)=>c.input?.action==='channel').map((c:any)=>c.input)).toEqual([{action:'channel',channel:'experimental',experimentalOptIn:true}]);
  expect(calls.some((c:any)=>['account.checkout','account.simulation','account.cancel','ai.ask'].includes(c.action))).toBe(false);
  expect(calls.filter((c:any)=>c.input).every((c:any)=>Object.keys(c.input).every(key=>['action','channel','experimentalOptIn'].includes(key)))).toBe(true);
});
test('origem ainda não liberada mantém opções desativadas',async({page})=>{
  const settings=await setup(page,false);
  await expect(settings.getByLabel('Versões que deseja receber')).toBeDisabled();
  await expect(settings.getByRole('button',{name:'Verificar atualizações'})).toBeEnabled();
});
test('oferta já baixada permite verificar novamente antes da instalação',async({page})=>{
  const settings=await setup(page,true,true);
  await expect(settings.getByRole('button',{name:'Instalar e reiniciar'})).toBeEnabled();
  await settings.getByRole('button',{name:'Verificar novamente'}).click();
  const calls=await page.evaluate(()=>Reflect.get(window,'updaterCalls'));
  expect(calls.some((c:any)=>c.input?.action==='install')).toBe(false);
  expect(calls.filter((c:any)=>c.input?.action==='check').length).toBeGreaterThanOrEqual(2);
});
