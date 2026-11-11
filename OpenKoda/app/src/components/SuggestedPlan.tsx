import { useRef, useState } from 'react';
import { request } from '../api';
import { type OperationProposal, operationLabels } from '../ai-operation';
import { type Action, type Assessment, errorMessage } from '../types';
import { KeyValue } from './shared';
import { useScopeAccess } from './ScopeAccess';
import { samePlanRef, sameScopeRef, validMutation, validPlanView, workReviewCurrent } from '../workflow-types';
import type { MutationResult, PlanPreparation, PlanView, SuggestionSource } from '../workflow-types';

type Review = { before: PlanView|null; after: PlanPreparation; key: string };
function sameSource(a: SuggestionSource|undefined, b: SuggestionSource) {
  return !!a && a.requestId===b.requestId && a.proposalId===b.proposalId && a.contextDigest===b.contextDigest
    && a.evidenceId===b.evidenceId && a.evidenceSha256===b.evidenceSha256 && a.adapter===b.adapter
    && sameScopeRef(a.scopeRef,b.scopeRef) && samePlanRef(a.base.head,b.base.head)
    && a.base.generation===b.base.generation && a.base.lastEvent===b.base.lastEvent;
}
function matchesPreview(after:PlanPreparation, source:SuggestionSource, before:PlanView|null) {
  const proposal=after?.proposal, nodes=proposal?.body?.nodes;
  if (!proposal || !sameSource(proposal.suggestionSource,source) || !sameScopeRef(proposal.body.scopeRef,source.scopeRef)
      || proposal.assessmentId!==source.scopeRef.assessmentId || proposal.body.assessmentId!==proposal.assessmentId
      || proposal.previousRevisionId!==(source.base.head?.revisionId??null) || proposal.number!==(source.base.head?.number??0)+1
      || before && proposal.planId!==before.id
      || proposal.body.limits.maxAttemptsTotal!==(before?.revision.body.limits.maxAttemptsTotal??2)
      || proposal.body.limits.maxCaptureBytesTotal!==(before?.revision.body.limits.maxCaptureBytesTotal??1048576)
      || after.supersededJobIds?.length!==0 || nodes?.length!==2 || !workReviewCurrent(after.review)) return false;
  const [human,scan]=nodes;
  if (human.intent.kind!=='human_review' || human.intent.inputs.length!==1 || human.dependsOn.length!==0
      || scan.id===human.id || scan.dependsOn.length!==1 || scan.dependsOn[0]!==human.id
      || scan.intent.kind!=='native' || scan.intent.adapter!==source.adapter || scan.intent.targetRef!==null || scan.intent.ports!==null) return false;
  const input=human.intent.inputs[0];
  return input.kind==='evidence' && input.evidenceId===source.evidenceId && input.expectedRawSha256===source.evidenceSha256;
}

export default function SuggestedPlan({proposal,assessment,selectedEvidence,action,disabled}:{
  proposal:OperationProposal;assessment:Assessment;selectedEvidence:string;action:Action;disabled:boolean;
}) {
  const scope=useScopeAccess();
  const [review,setReview]=useState<Review>();const [ack,setAck]=useState(false);
  const [pending,setPending]=useState(false);const lock=useRef(false);const [saved,setSaved]=useState<number>();
  const [error,setError]=useState('');
  const source=proposal.planChange?.source;
  const evidence=assessment.evidence.find(e=>e.id===selectedEvidence);
  if (!source?.base || !source.scopeRef || !evidence || !scope.matches(source.scopeRef)
      || source.requestId!==proposal.requestId || source.proposalId!==proposal.id || source.contextDigest!==proposal.contextDigest
      || source.adapter!==proposal.adapter || source.evidenceId!==evidence.id || source.evidenceSha256!==evidence.rawSha256
      || source.scopeRef.assessmentId!==assessment.id) return null;
  const expected=source;
  async function prepare() {
    if (lock.current || disabled) return;
    lock.current=true;setPending(true);setError('');setReview(undefined);setAck(false);
    try {
      const {plan}=await request<{plan:PlanView|null}>('work.plan.get',{assessmentId:assessment.id,revisionId:null});
      if (plan && (!validPlanView(plan,assessment.id)||plan.control!=='active'||plan.outcome!=='completed')
          || !samePlanRef(plan?.head??null,expected.base.head) || (plan?.generation??0)!==expected.base.generation
          || (plan?.eventCursor.afterSequence??0)!==expected.base.lastEvent) throw new Error('O plano mudou desde a sugestão.');
      const after=await request<PlanPreparation>('work.plan.prepareSuggestion',{assessmentId:assessment.id,requestId:proposal.requestId,proposalId:proposal.id});
      if (!matchesPreview(after,expected,plan)) throw new Error('A revisão recebida diverge da evidência ou da operação sugerida.');
      setReview({before:plan,after,key:crypto.randomUUID()});
    } catch (cause) {setError(errorMessage(cause));}
    finally {lock.current=false;setPending(false);}
  }
  async function commit() {
    if (!review||!ack||disabled||lock.current||!workReviewCurrent(review.after.review)) return;
    lock.current=true;setPending(true);setAck(false);setError('');
    try {
      const result=await action<MutationResult<PlanView>>('work.plan.commit',{review:review.after.review,idempotencyKey:review.key,acknowledgePlan:true});
      if (!validMutation(result,'work.plan.commit') || !validPlanView(result.value,assessment.id)
          || result.value.revision.digest!==review.after.proposal.digest || !sameSource(result.value.revision.suggestionSource,expected)) {
        throw new Error('Consulte o plano de ação para conferir a revisão registrada.');
      }
      setSaved(result.value.revision.number);setReview(undefined);
    } catch (cause) {setError(errorMessage(cause));}
    finally {lock.current=false;setPending(false);}
  }
  return <section aria-label="Alteração do plano sugerida pela IA">
    {saved ? <p role="status">Revisão {saved} salva. Abra o plano de ação para revisar a evidência.</p>
    : !review ? <button type="button" className="button" disabled={disabled||pending} onClick={()=>void prepare()}>Comparar alteração sugerida no plano</button>
    : <div className="review-sheet"><h4>Comparação da revisão proposta</h4>
      {review.before ? <><p>A revisão {review.before.head.number} fica no histórico com as operações concluídas.</p>
        <ul>{review.before.revision.body.nodes.map(n=><li key={n.id}>{n.purpose}</li>)}</ul></> : <p>Esta avaliação receberá seu primeiro plano.</p>}
      <p>A revisão {review.after.proposal.number} acrescenta dois passos ao histórico.</p>
      <ol><li>Revisar a evidência {evidence.name}.<p className="mono break-word">{source.evidenceSha256}</p></li>
        <li>{operationLabels[source.adapter]} no alvo {assessment.target}, após a decisão de prosseguir na revisão humana.</li></ol>
      <KeyValue label="Limites do plano">{review.after.proposal.body.limits.maxAttemptsTotal} tentativas · {review.after.proposal.body.limits.maxCaptureBytesTotal.toLocaleString('pt-BR')} bytes de captura</KeyValue>
      <label className="check-field"><input type="checkbox" checked={ack} disabled={disabled||pending} onChange={e=>setAck(e.target.checked)}/><span>Revisei a evidência, a dependência e os limites desta alteração.</span></label>
      <div className="inline-actions"><button type="button" className="button primary" disabled={disabled||pending||!ack||!workReviewCurrent(review.after.review)} onClick={()=>void commit()}>Salvar alteração no plano</button>
        <button type="button" className="button" disabled={pending} onClick={()=>{setReview(undefined);setAck(false);setError('');}}>Fechar comparação</button></div>
    </div>}{error&&<p role="alert">{error}</p>}
  </section>;
}
