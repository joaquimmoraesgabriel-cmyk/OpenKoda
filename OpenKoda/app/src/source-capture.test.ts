import { describe, expect, it } from 'vitest';
import { SourcePreview, sourceCaptureAllowed, validSourcePreview } from './source-capture';
import { Assessment, ExecutionPlan } from './types';
const assessment={id:'assessment',kind:'code',status:'awaiting_approval'} as Assessment;
const plan={id:'plan',assessmentId:assessment.id,adapter:'source.rules.v1',digest:'plan-digest',status:'proposed',inputFiles:[{path:'a.ts',size:12,sha256:'a'.repeat(64)}],sourceScope:{manifestSha256:'manifest'}} as ExecutionPlan;
const preview:SourcePreview={assessmentId:assessment.id,planId:plan.id,planDigest:plan.digest,manifestSha256:'manifest',paths:['a.ts'],files:[{path:'a.ts',bytes:12,sourceSha256:'a'.repeat(64),redactedSha256:'b'.repeat(64),text:'[REDACTED]',redactionChanged:true}],totalSourceBytes:12,digest:'capture-digest',sourceFilesIncluded:true,sentToProvider:false,notice:'Cópia local.'};
describe('captura seletiva local',()=>{
  it('permite plano proposto, mas bloqueia autorização ativa em qualquer plano da avaliação',()=>{expect(sourceCaptureAllowed(assessment,[plan],true)).toBe(true);expect(sourceCaptureAllowed(assessment,[plan,{...plan,id:'other',adapter:'local.inventory.v1',status:'approved'}],true)).toBe(false);expect(sourceCaptureAllowed({...assessment,status:'unknown'},[plan],true)).toBe(false);expect(sourceCaptureAllowed(assessment,[plan],false)).toBe(false);});
  it('exige identidade da avaliação, manifest, seleção e hashes exatos na prévia',()=>{expect(validSourcePreview(preview,assessment.id,plan,['a.ts'])).toBe(true);for(const change of [{assessmentId:'other'},{manifestSha256:'other'},{planDigest:'other'},{paths:['extra.ts']},{sentToProvider:true},{files:[{...preview.files[0],sourceSha256:'changed'}]}])expect(validSourcePreview({...preview,...change},assessment.id,plan,['a.ts'])).toBe(false);});
  it('recusa duplicação, tamanho divergente ou NUL mesmo que a prévia anuncie captura local',()=>{for(const change of [{paths:['a.ts','a.ts']},{totalSourceBytes:32769},{totalSourceBytes:11},{files:[{...preview.files[0],text:'nul\0text'}]}])expect(validSourcePreview({...preview,...change},assessment.id,plan,['a.ts'])).toBe(false);});
});
