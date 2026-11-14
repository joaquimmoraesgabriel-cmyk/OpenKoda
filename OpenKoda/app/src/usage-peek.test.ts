import { describe,it,expect } from 'vitest';
import type { BudgetStatus } from './ai-budget';
import type { AccountView } from './commercial';
import { activityCells,usageLimit,recentModelCosts } from './usage-peek';

const budget={enabled:true,degraded:false,installation:{limitUsd:'10',pricedUsageUsd:'8',heldUsd:'1',remainingUsd:'1'}} as BudgetStatus;
const account={plan:{id:'plus'},subscription:{status:'active',mode:'production',periodEnd:new Date(Date.now()+86400000).toISOString()},usage:{monthlyLimitMicros:20000000,monthlyUsedMicros:19000000,windowLimitMicros:5000000,windowUsedMicros:1000000}} as AccountView;
describe('resumo de uso mantém a origem dos limites',()=>{
  it('chave própria considera reserva e limite local, independente da assinatura',()=>{
    expect(usageLimit('openrouter',budget,account)).toMatchObject({kind:'personal',ratio:.1,low:true,used:8,held:1,remaining:1});
  });
  it('assinatura considera a menor disponibilidade entre janela e ciclo',()=>{
    expect(usageLimit('openkoda',budget,account)).toMatchObject({kind:'subscription',ratio:.05,low:true,used:1,remaining:1,cycleRemaining:1});
  });
  it('não inventa cota sem medição, assinatura simulada ou plano vencido',()=>{
    expect(usageLimit('openai',{...budget,enabled:false})).toMatchObject({ratio:null,low:false,limit:null});
    expect(usageLimit('openai',{...budget,degraded:true})).toMatchObject({ratio:null,low:false});
    expect(usageLimit('openkoda',budget,{...account,subscription:{status:'active',mode:'mock'}})).toMatchObject({available:false,ratio:null,low:false});
    expect(usageLimit('openkoda',budget,{...account,subscription:{status:'expired'}})).toMatchObject({available:false,ratio:null});
    expect(usageLimit('openkoda',budget,{...account,usage:{windowLimitMicros:1000}})).toMatchObject({ratio:null,used:null});
  });
  it('calendário tem exatamente 21 dias e isola provedores',()=>{
    const cells=activityCells([{day:'2026-11-14',provider:'openai',model:'a',assessmentId:null,requests:1,input:null,output:null,partial:1},
      {day:'2026-11-13',provider:'openrouter',model:'b',assessmentId:null,requests:3,input:100,output:5,partial:0}],'openai','2026-11-14');
    expect(cells).toHaveLength(21);expect(cells[0].day).toBe('2026-10-25');expect(cells.at(-1)?.level).toBe(4);expect(cells.at(-2)?.level).toBe(0);
  });
  it('custo recente não atribui provedor a um lançamento sem endpoint',()=>{
    const b={...budget,recentCharges:[{model:'one',endpoint:'https://api.openai.com/v1',pricedUsageUsd:'0.12'},{model:'unknown',pricedUsageUsd:'4'},{model:'other',endpoint:'https://openrouter.ai/api/v1',pricedUsageUsd:'1'}]} as BudgetStatus;
    expect([...recentModelCosts(b,'openai')]).toEqual([['one',{cost:.12,unknown:0}]]);
  });
});
