import { useEffect, useRef, useState } from 'react';
import { request } from '../api';
import { ClassificationSubject, ClassificationSubjects, dataChangeEvent } from '../data-policy';
import { errorMessage } from '../types';
import DataAccess from './DataAccess';
import { Notice } from './shared';

export default function RemediationContentReview({assessmentId,planId,digest,onReviewed}:{assessmentId:string;planId:string;digest:string;onReviewed:(complete:boolean)=>void}){
 const notify=useRef(onReviewed);notify.current=onReviewed;
 const [row,setRow]=useState<ClassificationSubject>();const [error,setError]=useState('');
 useEffect(()=>{let active=true;async function refresh(){notify.current(false);setRow(undefined);try{const inventory=await request<ClassificationSubjects>('data.classification.subjects',{assessmentId});const value=inventory.subjects.find(item=>item.subject.kind==='derived'&&item.subject.producer==='remediation'&&item.subject.objectId===planId);if(!value)throw new Error('O núcleo ainda não disponibilizou o registro desta proposta para revisão local.');if(active){setRow(value);setError('');}}catch(e){if(active)setError(errorMessage(e));}}void refresh();window.addEventListener(dataChangeEvent,refresh);return()=>{active=false;notify.current(false);window.removeEventListener(dataChangeEvent,refresh);};},[assessmentId,planId,digest]);
 return <section aria-label="Revisão do conteúdo da alteração"><Notice>Leia a representação completa da proposta pelas janelas abaixo antes de aprovar. Esta leitura não concede execução; a aprovação da alteração continua separada e vinculada ao plano.</Notice>{error&&<div role="alert"><Notice tone="warning">{error} A proposta foi preservada; sua aprovação permanece bloqueada até a revisão do conteúdo.</Notice></div>}{row&&<DataAccess assessmentId={assessmentId} subject={row.subject} sourceFingerprint={row.sourceFingerprint} onReviewChange={onReviewed}/>}</section>;
}
