import { describe,expect,it } from 'vitest';
import { EvidenceView, validEvidenceView } from './data-policy';

function projected():EvidenceView {
 const eligible={allowedByShape:true,requiresDataReview:true as const,reasonCodes:[]};
 return {id:'evidence',name:'Representação',createdAt:'2026-11-08T00:00:00Z',rawSha256:'original-hash',rawBytes:10000,hasRaw:true,classification:{sensitivity:'unclassified',classificationRef:null,sourceFingerprint:'source-fingerprint',secretMaterial:false,contributorRefs:[],reasonCodes:[]},preview:'é',previewBytes:2,previewTruncated:true,projectionSha256:'preview-hash',projection:{schema:'openkoda.projection.v1',redactorVersion:'openkoda.redactor.v1',mode:'no_credential_configured',redacted:false,omittedFields:[],reasonCodes:[]},originRefs:[],originKind:'unknown',structuredView:null,eligibility:{ai:eligible,kaliHash:eligible,kaliScan:eligible,rawRead:eligible}};
}
describe('fronteira pública de evidências',()=>{
 it('aceita preview limitado sem exigir o original no renderer',()=>{expect(validEvidenceView(projected())).toBe(true);});
 it('recusa o DTO canônico mesmo quando possui todos os campos da view',()=>{expect(validEvidenceView({...projected(),content:'original'})).toBe(false);expect(validEvidenceView({...projected(),sha256:'ambiguous-hash'})).toBe(false);});
 it('mede o preview em bytes UTF-8 e recusa contador ou limite inconsistente',()=>{expect(validEvidenceView({...projected(),previewBytes:1})).toBe(false);expect(validEvidenceView({...projected(),preview:'é'.repeat(1025),previewBytes:2050})).toBe(false);expect(validEvidenceView({...projected(),preview:null,previewBytes:2})).toBe(false);});
 it('elegibilidade de formato exige contrato explícito de revisão de dados',()=>{const e=projected();expect(validEvidenceView({...e,eligibility:{...e.eligibility,ai:{allowedByShape:true,reasonCodes:[],requiresDataReview:false}}})).toBe(false);});
});
