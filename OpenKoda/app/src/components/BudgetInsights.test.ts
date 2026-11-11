import { describe, expect, it } from 'vitest';
import type { BudgetCharge } from '../ai-budget';
import { readableAmount, usageDays } from './BudgetInsights';

const today = new Date(2026, 10, 11, 15);
it('formata dólares com centavos e conserva frações de microdólar registradas',()=>{
  expect(readableAmount(null)).toBe('Não informado');
  expect(readableAmount('0.000000')).toBe('USD 0,00');
  expect(readableAmount('900000.000001')).toBe('USD 900.000,000001');
  expect(readableAmount('0.250000')).toBe('USD 0,25');
});
function charge(date:Date, input:number|null, output:number|null, state='priced'): BudgetCharge {
  return {chargeId:'fixture',model:'synthetic',createdAt:date.toISOString(),state,inputTokens:input,outputTokens:output,reservedUsd:null,assumedChargeUsd:null,pricedUsageUsd:null};
}
describe('atividade de tokens dos registros de custo',()=>{
  it('separa zero informado de dados incompletos e ignora tentativas sem envio',()=>{
    const [day] = usageDays([
      charge(today, 0, null), charge(today, null, null, 'unknown'), charge(today, 18, 2), charge(today, 99, 99, 'not_dispatched'),
    ],1,today);
    expect(day).toMatchObject({input:18,output:2,inputs:2,outputs:1,partial:2});
  });
  it('filtra por dia local, excluindo o futuro e datas anteriores ao intervalo',()=>{
    const days = usageDays([
      charge(new Date(2026,10,4,23,59),90,80), charge(new Date(2026,10,5,0,0),5,1),
      charge(new Date(2026,10,11,15,1),60,40),
    ],7,today);
    expect(days).toHaveLength(7);
    expect(days[0]).toMatchObject({input:5,output:1,partial:0});
    expect(days[6]).toMatchObject({inputs:0,outputs:0});
  });
});
