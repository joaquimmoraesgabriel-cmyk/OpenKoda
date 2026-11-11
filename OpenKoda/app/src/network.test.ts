import { describe, expect, it } from 'vitest';
import { parsePorts } from './network';

describe('seleção explícita de portas TCP',()=>{
  it('mantém a lista vazia sem presumir portas autorizadas',()=>{expect(parsePorts('  ')).toEqual({ports:[],error:'Informe pelo menos uma porta TCP.'});});
  it('aceita limites e delimitadores de uma lista explícita',()=>{expect(parsePorts('1, 443\n65535')).toEqual({ports:[1,443,65535],error:null});});
  it('rejeita portas repetidas inclusive representações numéricas equivalentes',()=>{expect(parsePorts('80,080').error).toContain('únicas');});
  it('limita a quantidade a 16 portas',()=>{expect(parsePorts(Array.from({length:16},(_,index)=>String(index+1)).join(',')).error).toBeNull();expect(parsePorts(Array.from({length:17},(_,index)=>String(index+1)).join(',')).error).toContain('16');});
  it('rejeita porta zero, fora do limite, intervalo e texto executável',()=>{for(const invalid of ['0','65536','-1','80.5','1-100','80;whoami','80,'])expect(parsePorts(invalid).ports).toEqual([]);});
});
