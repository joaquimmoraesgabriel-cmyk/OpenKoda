import { describe,it,expect } from 'vitest';
import { OperationProposal,operationMatchesTargets } from './ai-operation';
const first={entryId:'a',entryDigest:'digest-a'},second={entryId:'b',entryDigest:'digest-b'};
const proposal={contextTargetRefs:[first,second],operationTargetRef:first} as OperationProposal;
describe('sugestão ligada à seleção humana',()=>{
 it('aceita ordem canônica diferente, preservando o conjunto e o alvo operacional',()=>{expect(operationMatchesTargets(proposal,[second,first],first)).toBe(true);});
 it('recusa troca, duplicação, ausência ou digest antigo',()=>{expect(operationMatchesTargets(proposal,[first,second],second)).toBe(false);expect(operationMatchesTargets({...proposal,contextTargetRefs:[first,first]},[first,second],first)).toBe(false);expect(operationMatchesTargets({...proposal,operationTargetRef:undefined},[first,second],first)).toBe(false);expect(operationMatchesTargets({...proposal,operationTargetRef:{...first,entryDigest:'old'}},[first,second],first)).toBe(false);expect(operationMatchesTargets(proposal,[second],first)).toBe(false);});
 it('mantém o retorno V1 sem referências novas',()=>{expect(operationMatchesTargets({} as OperationProposal)).toBe(true);expect(operationMatchesTargets(proposal)).toBe(false);});
});
