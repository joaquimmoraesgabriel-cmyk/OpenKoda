import { useRef, useState } from 'react';
import { ListChecks } from 'lucide-react';
import { operationLabels, type OperationProposal } from '../ai-operation';
import { request } from '../api';
import { parsePorts } from '../network';
import { scopePlanMatches, type ScopeRef, type ScopeStatus } from '../scope';
import { errorMessage, type Action } from '../types';

export function ChatVerification({proposal, scopeRef, action, disabled, onPrepared}:{
  proposal:OperationProposal;
  scopeRef:ScopeRef;
  action:Action;
  disabled:boolean;
  onPrepared:()=>void;
}){
  const [ports,setPorts]=useState('');
  const [working,setWorking]=useState(false);
  const [error,setError]=useState('');
  const gate=useRef(false);
  const tcp=proposal.adapter==='network.tcp-connect.v1';
  const target=proposal.operationTargetRef;
  async function prepare(){
    if(gate.current||disabled)return;
    gate.current=true;
    setWorking(true);
    setError('');
    try{
      const parsed=tcp?parsePorts(ports):{ports:[],error:null};
      if(parsed.error)throw new Error(parsed.error);
      const current=await request<ScopeStatus>('assessment.scopeStatus',{assessmentId:proposal.assessmentId});
      if(!scopePlanMatches(scopeRef,current))throw new Error('O escopo mudou. Peça uma nova sugestão.');
      const portOptions=tcp?{ports:parsed.ports}:{};
      if(target){
        await action('assessment.targetPlan',{assessmentId:proposal.assessmentId,scopeRef,targetRef:target,adapter:proposal.adapter,...portOptions});
      }else{
        const name=tcp?'assessment.networkPlan':proposal.adapter==='source.rules.v1'?'assessment.sourcePlan':'assessment.plan';
        await action(name,{id:proposal.assessmentId,...portOptions});
      }
      onPrepared();
    }catch(cause){setError(errorMessage(cause));}
    finally{gate.current=false;setWorking(false);}
  }
  return <section className="chat-approval" aria-label="Verificação sugerida">
    <ListChecks size={18}/><h3>{operationLabels[proposal.adapter]}</h3><p>{proposal.rationale}</p>
    {proposal.question&&<p>{proposal.question}</p>}
    {tcp&&<label>Portas da verificação TCP<input value={ports} onChange={e=>setPorts(e.target.value)} placeholder="Ex.: 80, 443"/></label>}
    {error&&<p role="alert">{error}</p>}
    <div><button type="button" className="button" disabled={working} onClick={onPrepared}>Dispensar</button>
      <button type="button" className="button primary" disabled={disabled||working} onClick={()=>void prepare()}>Preparar esta verificação</button></div>
  </section>;
}

