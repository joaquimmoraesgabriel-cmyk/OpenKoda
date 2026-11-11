import { describe, expect, it } from 'vitest';
import { operationAdapters, validOperationProposal } from './ai-operation';
describe('sugestões limitadas de operação',()=>{
  it('limita opções por modalidade e conserva o identificador TCP exato',()=>{expect(operationAdapters('network')).toEqual(['network.tcp-connect.v1','kali.rg.signals.v1']);expect(operationAdapters('web')).not.toContain('source.rules.v1');expect(operationAdapters('files')).toContain('source.rules.v1');});
  it('não admite sugestão com identidade ou autoridade divergente',()=>{const proposal={id:'p',requestId:'r',responseId:'response',assessmentId:'a',contextDigest:'d',adapter:'source.rules.v1',rationale:'Observação a revisar.',question:null,model:'fixture',endpoint:'https://api.openai.com/v1',notice:'Não executado.',usage:null,toolsExecuted:false,requiresHumanReview:true,availabilityVerified:false};expect(validOperationProposal(proposal,'a')).toBe(true);for(const changes of [{adapter:'shell.execute'},{toolsExecuted:true},{requiresHumanReview:false},{availabilityVerified:true},{assessmentId:'other'}])expect(validOperationProposal({...proposal,...changes},'a')).toBe(false);});
});
