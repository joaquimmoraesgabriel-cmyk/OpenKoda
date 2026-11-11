import { useEffect, useState } from 'react';
import { request } from '../api';
import type { Assessment, Finding } from '../types';
import { errorMessage } from '../types';
import type { ScopeTargetRef } from '../combined-scope';
import { sameTargetRef } from '../combined-scope';
import { useScopeAccess } from './ScopeAccess';
import { Field, Notice } from './shared';

export interface Origin {targetRefs:ScopeTargetRef[];verified:boolean;reason?:string;originKind?:'user_association'|'host_record'|'evidence_union'}
interface Origins {assessmentId:string;combined:boolean;evidence:(Origin&{evidenceId:string})[];findings:(Origin&{findingId:string})[]}
export function useEvidenceOrigins(assessment:Assessment){
 const scope=useScopeAccess();const combined=!!scope.status?.revision?.body.targetSet;
 const [state,setState]=useState<{key:string;data?:Origins;error?:string}>({key:''});
 const [reload,setReload]=useState(0);
 const key=JSON.stringify([assessment.id,assessment.updatedAt,assessment.evidence.map(e=>[e.id,e.rawSha256]),scope.revision]);
 useEffect(()=>{let active=true;if(combined){setState({key});void request<Origins>('evidence.origins',{assessmentId:assessment.id}).then(data=>{if(data.assessmentId!==assessment.id||!data.combined||!Array.isArray(data.evidence)||!Array.isArray(data.findings)||[...data.evidence,...data.findings].some(item=>typeof item.verified!=='boolean'||!Array.isArray(item.targetRefs)||item.targetRefs.some(ref=>!ref||typeof ref.entryId!=='string'||!ref.entryId||typeof ref.entryDigest!=='string'||!ref.entryDigest)))throw new Error('A origem retornada não corresponde à avaliação.');if(active)setState({key,data});}).catch(error=>{if(active)setState({key,error:errorMessage(error)});});}return()=>{active=false;};},[key,combined,reload]);
 return {combined,options:scope.status?.targetOptions??[],data:state.key===key?state.data:undefined,error:state.key===key?state.error:undefined,refresh:()=>{setState({key});setReload(value=>value+1);}};
}
export function OriginView({origin,options}:{origin?:Origin;options:ReturnType<typeof useEvidenceOrigins>['options']}){
 if(!origin)return <p className="muted">Origem ainda não confirmada pelo núcleo.</p>;
 return <div className="evidence-origin"><p><strong>{origin.verified?(origin.originKind==='user_association'?'Associação declarada pelo usuário; conteúdo não verificado':'Vínculo de escopo conferido nos registros'):'Origem não verificada'}</strong></p><p className="muted">O vínculo não comprova a veracidade do conteúdo nem valida um achado.</p>{origin.verified&&origin.targetRefs.map(ref=><p key={ref.entryId} className="break-word">{options.find(o=>sameTargetRef(o.targetRef,ref))?.entry.label??ref.entryId}</p>)}{origin.verified&&!origin.targetRefs.length&&<p className="muted">Registro comum, sem comprovação de acesso a uma entrada.</p>}</div>;
}
export function FindingOrigin({assessment,findingId}:{assessment:Assessment;findingId:string}){const origins=useEvidenceOrigins(assessment);if(!origins.combined)return null;return <section aria-label="Origem do achado"><h3>Origem do achado</h3>{origins.error&&<Notice tone="warning">{origins.error}</Notice>}<OriginView origin={origins.data?.findings.find(f=>f.findingId===findingId)} options={origins.options}/></section>;}
export function ScopedFindings({assessment,findings,onOpen}:{assessment:Assessment;findings:Finding[];onOpen:(id:string)=>void}){
 const origins=useEvidenceOrigins(assessment);const [filter,setFilter]=useState('all');
 const visible=findings.filter(f=>filter==='all'||origins.data?.findings.some(origin=>origin.findingId===f.id&&origin.verified&&origin.targetRefs.some(ref=>ref.entryId===filter)));
 return <div className="content-section"><p className="muted">{findings.filter(f=>f.status==='validated').length} validados · {findings.filter(f=>f.status==='candidate').length} candidatos</p>{origins.combined&&<><Field label="Filtrar achados por origem"><select value={filter} onChange={e=>setFilter(e.target.value)}><option value="all">Todas as origens</option>{origins.options.map(o=><option key={o.entry.entryId} value={o.entry.entryId}>{o.entry.label}</option>)}</select></Field>{origins.error&&<Notice tone="warning">{origins.error}</Notice>}</>}{visible.map(f=><div key={f.id}><button className="history-row" onClick={()=>onOpen(f.id)}><strong>{f.title}</strong><span>{f.status==='candidate'?'Candidato':'Revisado'}</span></button>{origins.combined&&<OriginView origin={origins.data?.findings.find(origin=>origin.findingId===f.id)} options={origins.options}/>}</div>)}{!visible.length&&<p className="muted">Nenhum achado com esta origem informada pelo núcleo.</p>}</div>;
}
