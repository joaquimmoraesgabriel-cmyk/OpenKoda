import { describe, expect, it } from 'vitest';
import { supportsResultInput, validBackendPreparation } from './workflow-types';
import type { AiPreparation, KaliPreparation, OperationIntent, WorkJobView } from './workflow-types';
const rawSha256='a'.repeat(64);const input={kind:'evidence' as const,id:'evidence',rawSha256};const intentInput={kind:'evidence' as const,evidenceId:input.id,expectedRawSha256:rawSha256};
const job={} as WorkJobView;
describe('inputs do plano',()=>{
 it('investigação derivada só pode alimentar revisão humana',()=>{for(const consumer of ['native','human_review','kali_evidence','ai_investigation'] as const)expect(supportsResultInput(consumer,'ai_investigation','investigation')).toBe(consumer==='human_review');expect(supportsResultInput('human_review','native','investigation')).toBe(false);});
 it('Kali e IA aceitam evidência de native e Kali; revisão humana não fabrica evidência',()=>{for(const consumer of ['human_review','kali_evidence','ai_investigation'] as const){expect(supportsResultInput(consumer,'native','evidence')).toBe(true);expect(supportsResultInput(consumer,'kali_evidence','evidence')).toBe(true);expect(supportsResultInput(consumer,'human_review','evidence')).toBe(false);expect(supportsResultInput(consumer,'ai_investigation','evidence')).toBe(false);}});
});
describe('preparação dos backends',()=>{
 const ai:AiPreparation={kind:'ai_investigation',job,inputs:[input],contextSelection:{evidenceIds:[input.id],contextTargetRefs:[]},requiresDataReview:true};const aiIntent:OperationIntent={kind:'ai_investigation',inputs:[intentInput],perspectives:['evidence']};
 const kali:KaliPreparation={kind:'kali_evidence',job,input,operation:'hash',environmentId:'environment',requiresDataReview:true,packetPreview:null};const kaliIntent:OperationIntent={kind:'kali_evidence',input:intentInput,operation:'hash'};
 it('aceita metadados exatos que ainda exigem revisão de dados',()=>{expect(validBackendPreparation(ai,aiIntent)).toBe(true);expect(validBackendPreparation(kali,kaliIntent)).toBe(true);});
 it('recusa hash alterado, papel derivado, duplicatas e seleção extra de IA',()=>{expect(validBackendPreparation({...ai,inputs:[{...input,rawSha256:'b'.repeat(64)}]},aiIntent)).toBe(false);expect(validBackendPreparation({...ai,inputs:[{kind:'derived',producer:'investigation',id:'x',fingerprint:rawSha256}]},aiIntent)).toBe(false);expect(validBackendPreparation({...ai,inputs:[input,input]},aiIntent)).toBe(false);expect(validBackendPreparation({...ai,contextSelection:{evidenceIds:['different'],contextTargetRefs:[]}},aiIntent)).toBe(false);});
 it('recusa operação ou ambiente divergente e tentativa de omitir revisão',()=>{expect(validBackendPreparation({...kali,operation:'scan'},kaliIntent)).toBe(false);expect(validBackendPreparation({...kali,environmentId:''},kaliIntent)).toBe(false);expect(validBackendPreparation({...kali,requiresDataReview:false},kaliIntent)).toBe(false);expect(validBackendPreparation({...ai,requiresDataReview:false},aiIntent)).toBe(false);});
});
