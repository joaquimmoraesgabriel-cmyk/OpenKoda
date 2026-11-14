import type { BudgetStatus } from './ai-budget';
import type { AccountView } from './commercial';
import { knownProvider, providers } from './ai-provider';
import { groupUsage, localDay, offsetDay, type UsageRow } from './usage-history';

export interface UsageLimit {
  kind:'personal'|'subscription'; available:boolean; ratio:number|null; low:boolean;
  limit:number|null; used:number|null; remaining:number|null; held:number|null;
  caption:string; resetAt?:string; cycleRemaining?:number|null;
}
const amount=(value:unknown):number|null=>{
  if(typeof value!=='number'&&typeof value!=='string'||value==='')return null;
  const n=Number(value);return Number.isFinite(n)&&n>=0?n:null;
};
const fraction=(left:number|null,total:number|null)=>left!==null&&total!==null&&total>0?Math.min(1,Math.max(0,left/total)):null;
export function usageLimit(provider:string|undefined,budget?:BudgetStatus,account?:AccountView|null):UsageLimit {
  if(provider==='openkoda'){
    const usable=!!account?.plan&&account.subscription?.status==='active'&&account.subscription.mode!=='mock'&&typeof account.subscription.periodEnd==='string'&&Date.parse(account.subscription.periodEnd)>Date.now();
    const u=usable?account.usage:undefined;
    const limit=amount(u?.windowLimitMicros),used=amount(u?.windowUsedMicros);
    const cycleLimit=amount(u?.monthlyLimitMicros??u?.monthLimitMicros),cycleUsed=amount(u?.monthlyUsedMicros??u?.monthUsedMicros);
    const remaining=limit!==null&&used!==null?Math.max(0,limit-used):null;
    const cycleRemaining=cycleLimit!==null&&cycleUsed!==null?Math.max(0,cycleLimit-cycleUsed):null;
    const weekly=fraction(remaining,limit),monthly=fraction(cycleRemaining,cycleLimit);
    const ratio=weekly===null?null:monthly===null?weekly:Math.min(weekly,monthly);
    return {kind:'subscription',available:usable,ratio,low:ratio!==null&&ratio<=.15,
      limit:limit===null?null:limit/1e6,used:used===null?null:used/1e6,remaining:remaining===null?null:Math.min(remaining,cycleRemaining??remaining)/1e6,held:null,
      cycleRemaining:cycleRemaining===null?null:cycleRemaining/1e6,resetAt:u?.windowEnd,
      caption:usable?'Cota da assinatura · janela atual':'Cota da assinatura indisponível'};
  }
  const b=budget?.installation;
  const available=budget?.enabled===true&&budget.degraded!==true&&!!b;
  const limit=available?amount(b.limitUsd):null,used=amount(b?.pricedUsageUsd),held=amount(b?.heldUsd);
  const remaining=available?amount(b.remainingUsd):null;
  const ratio=fraction(remaining,limit);
  return {kind:'personal',available,ratio,low:ratio!==null&&ratio<=.15,limit,used,held,remaining,
    caption:budget?.degraded?'Orçamento temporariamente indisponível':budget?.enabled?'Orçamento local · todos os provedores':'Chave própria · sem limite local ativo'};
}
export function activityCells(rows:UsageRow[],provider:string,today=localDay()){
  const days=groupUsage(rows.filter(r=>r.provider===provider),row=>row.day);
  const cells=Array.from({length:21},(_,i)=>{const day=offsetDay(today,i-20);return {day,totals:days.get(day)};});
  const peak=Math.max(1,...cells.map(c=>c.totals?.requests??0));
  return cells.map(c=>({...c,level:c.totals?.requests?Math.max(1,Math.ceil(Math.sqrt(c.totals.requests/peak)*4)):0}));
}
export function chargeProvider(endpoint?:string){
  if(!endpoint)return undefined;
  try {const url=new URL(endpoint);return Object.keys(providers).find(id=>knownProvider(id)&&new URL(providers[id].endpoint).origin===url.origin);}catch{return undefined;}
}
export function recentModelCosts(budget:BudgetStatus|undefined,provider:string){
  const result=new Map<string,{cost:number;unknown:number}>();
  for(const charge of budget?.recentCharges??[]){
    if(chargeProvider(charge.endpoint)!==provider)continue;
    const entry=result.get(charge.model)??{cost:0,unknown:0};
    const cost=amount(charge.pricedUsageUsd);
    if(cost===null)entry.unknown++;else entry.cost+=cost;
    result.set(charge.model,entry);
  }
  return result;
}
