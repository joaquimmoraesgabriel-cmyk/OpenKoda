import { Assessment, ExecutionPlan } from './types';

export interface SourcePreview {
  assessmentId:string;planId:string;planDigest:string;manifestSha256:string;paths:string[];
  files:{path:string;bytes:number;sourceSha256:string;redactedSha256:string;text:string;redactionChanged:boolean}[];
  totalSourceBytes:number;digest:string;sourceFilesIncluded:true;sentToProvider:false;notice:string;
}
export function sourceCaptureAllowed(assessment:Assessment,plans:ExecutionPlan[],projectActive:boolean){
  return projectActive&&(['code','files'].includes(assessment.kind)||plans.some(p=>p.assessmentId===assessment.id&&p.adapter==='source.rules.v1'&&p.targetRef))&&!['preparing','planning','queued','running','pausing','validating','canceling','unknown'].includes(assessment.status)&&!plans.some(p=>p.assessmentId===assessment.id&&['approved','running','canceling','unknown'].includes(p.status));
}
export function validSourcePreview(value:unknown,assessmentId:string,plan:ExecutionPlan,paths:string[]):value is SourcePreview {
  if(!value||typeof value!=='object'||!plan.sourceScope)return false;
  const p=value as SourcePreview;const expected=[...paths].sort();
  if(p.assessmentId!==assessmentId||p.planId!==plan.id||p.planDigest!==plan.digest||p.manifestSha256!==plan.sourceScope.manifestSha256||p.sourceFilesIncluded!==true||p.sentToProvider!==false||typeof p.digest!=='string'||!p.digest||typeof p.notice!=='string'||!Array.isArray(p.paths)||!Array.isArray(p.files)||p.files.length<1||p.files.length>8||p.files.length!==expected.length||new Set(p.paths).size!==p.paths.length||JSON.stringify([...p.paths].sort())!==JSON.stringify(expected)||!Number.isSafeInteger(p.totalSourceBytes)||p.totalSourceBytes>32768)return false;
  if(JSON.stringify(p.files.map(f=>f?.path).sort())!==JSON.stringify(expected))return false;
  return p.files.every(f=>{const source=plan.inputFiles.find(input=>input.path===f.path);return source&&f.bytes===source.size&&f.sourceSha256===source.sha256&&typeof f.redactedSha256==='string'&&/^[a-f0-9]{64}$/i.test(f.redactedSha256)&&typeof f.text==='string'&&!f.text.includes('\0')&&typeof f.redactionChanged==='boolean';})&&p.files.reduce((sum,file)=>sum+file.bytes,0)===p.totalSourceBytes;
}
