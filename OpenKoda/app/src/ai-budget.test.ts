import { describe, expect, it } from 'vitest';
import { usd, usdMicros, validBudgetReview } from './ai-budget';
describe('valores e revisões monetárias',()=>{
  it('compara decimais sem arredondamento binário e limita entradas',()=>{expect(usdMicros('0.000001')).toBe(1n);expect(usdMicros('1.000001')).toBe(1000001n);expect(usdMicros('1000000')).toBe(1000000000000n);for(const input of ['-1','1,50','1e3','0.0000001','1000000.000001',' 1','NaN',''])expect(usdMicros(input)).toBeNull();});
  it('não transforma custo ausente em zero',()=>{expect(usd(null)).toBe('Não informado');expect(usd(undefined)).toBe('Não informado');expect(usd('0.000000')).toBe('USD 0.000000');});
  it('recusa revisão sem preço explícito, prazo ou limites válidos',()=>{const r={id:'review',digest:'digest',epochId:'epoch',policyRevision:'policy',model:'fixture',endpoint:'https://fixture.test/v1',priceRevision:'price',notice:'Estimativa manual.',expiresAt:new Date(Date.now()+60000).toISOString(),reservedUsd:'0.000100',rates:{inputUsdPerMillion:'1.00',outputUsdPerMillion:'2.00',sourceNote:'Fixture manual'},requests:1,maxOutputTokens:2048,estimatedInputTokens:1024};expect(validBudgetReview(r)).toBe(true);for(const change of [{reservedUsd:null},{rates:null},{expiresAt:'invalid'},{requests:0},{maxOutputTokens:0}])expect(validBudgetReview({...r,...change})).toBe(false);});
});
