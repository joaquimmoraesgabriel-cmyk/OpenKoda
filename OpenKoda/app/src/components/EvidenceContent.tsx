import { Evidence, CandidateDraft } from '../types';
import { sensitivityLabels } from '../data-labels';
import DataAccess, { ProjectionNotice } from './DataAccess';
import SourceResults from './SourceResults';
import { KeyValue, Notice } from './shared';

export default function EvidenceContent({evidence,assessmentId,sourceResult,onCandidate}:{evidence:Evidence;assessmentId:string;sourceResult:boolean;onCandidate?:(draft:CandidateDraft)=>void}){
 return <><KeyValue label="Sensibilidade">{sensitivityLabels[evidence.classification.sensitivity]}</KeyValue><KeyValue label="Original preservado">{evidence.rawBytes.toLocaleString('pt-BR')} bytes</KeyValue><p className="mono break-word">SHA-256 do original: {evidence.rawSha256}</p><ProjectionNotice projection={evidence.projection}/>{evidence.classification.secretMaterial&&<Notice tone="warning">Material de segredo identificado pelo núcleo. A classificação não libera valores de credencial.</Notice>}{sourceResult&&<SourceResults evidence={evidence} assessmentId={assessmentId} onCandidate={onCandidate}/>}
  <section aria-label="Prévia redigida da evidência"><h4>Prévia da representação</h4>{evidence.preview===null?<p className="muted">Conteúdo omitido nesta consulta.</p>:<pre>{evidence.preview||'(representação vazia)'}</pre>}<p className="muted">{evidence.previewBytes.toLocaleString('pt-BR')} bytes apresentados{evidence.previewTruncated?' · prévia limitada; há conteúdo omitido':''}. A prévia não substitui o original.</p>{evidence.projectionSha256&&<p className="mono break-word">SHA-256 somente desta prévia: {evidence.projectionSha256}</p>}</section><DataAccess assessmentId={assessmentId} evidence={evidence}/>
 </>;
}
