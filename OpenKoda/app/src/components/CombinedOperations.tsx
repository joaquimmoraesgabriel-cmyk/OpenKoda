import { useEffect, useState } from 'react';
import { Action, Assessment, errorMessage } from '../types';
import { ScopeMethod } from '../scope';
import { operationLabels } from '../ai-operation';
import { currentScopeRef, targetAllows } from '../combined-scope';
import { parsePorts } from '../network';
import { useScopeAccess } from './ScopeAccess';
import { OperationTarget, useOperationTarget } from './TargetSelection';
import { Field, Notice } from './shared';

const adapters=['local.inventory.v1','source.rules.v1','http.headers.v1','network.tcp-connect.v1'] as const;
export default function CombinedOperations({assessment,action,busy,projectActive}:{assessment:Assessment;action:Action;busy:boolean;projectActive:boolean}){
 const scope=useScopeAccess();const target=useOperationTarget();const [adapter,setAdapter]=useState('');const [ports,setPorts]=useState('');const [pending,setPending]=useState(false);const [error,setError]=useState('');
 useEffect(()=>{setAdapter('');setPorts('');setError('');},[target.key]);
 if(!target.combined)return null;
 const allowed=adapters.filter(a=>targetAllows(target.option,a));const ready=['draft','blocked','paused','partial','failed','canceled'].includes(assessment.status);
 const portSelection=parsePorts(ports);const parsed=portSelection.ports;const portsError=adapter==='network.tcp-connect.v1'?portSelection.error:'';
 async function prepare(){if(!target.option||!allowed.includes(adapter as typeof adapters[number])||!ready||adapter==='network.tcp-connect.v1'&&(!parsed.length||portsError))return;setPending(true);setError('');try{await action('assessment.targetPlan',{assessmentId:assessment.id,scopeRef:currentScopeRef(scope.status),targetRef:target.option.targetRef,adapter,...adapter==='network.tcp-connect.v1'?{ports:parsed}:{}});}catch(e){setError(errorMessage(e));}finally{setPending(false);}}
 return <section className="content-section combined-operation" aria-label="Operação em uma entrada"><h2>Preparar operação nesta avaliação</h2><OperationTarget selection={target} disabled={busy||pending||!projectActive}/>{target.option&&<><p className="break-word">Alvo selecionado: {target.option.entry.target}</p><Field label="Método da operação combinada"><select value={adapter} disabled={busy||pending||!projectActive} onChange={e=>{setAdapter(e.target.value);setError('');}}><option value="">Selecione um método</option>{allowed.map(a=><option key={a} value={a}>{operationLabels[a]}</option>)}</select></Field>{!allowed.length&&<Notice>Nenhum dos executores de coleta está permitido agora nesta entrada. Consulte a revisão, a janela e os métodos selecionados.</Notice>}</>}{adapter==='network.tcp-connect.v1'&&<Field label="Portas TCP da entrada" hint="De 1 a 16 portas distintas, separadas por vírgula. Máximo de 64 pares antes das exclusões."><input value={ports} disabled={busy||pending} onChange={e=>setPorts(e.target.value)} placeholder="443,8443"/></Field>}{adapter&&['local.inventory.v1','source.rules.v1'].includes(adapter)&&<Notice>O preflight confere somente metadados das fontes locais selecionadas para impedir sobreposição. A leitura de conteúdo fica restrita à entrada escolhida e respeita as exclusões.</Notice>}{portsError&&ports&&<p className="muted">{portsError}</p>}{!ready&&<Notice>Há uma operação pendente ou estado que exige revisão. Conclua, descarte ou reconcilie o trabalho atual antes de preparar outro.</Notice>}{error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}<button type="button" className="button primary" disabled={busy||pending||!projectActive||!target.ready||!ready||!allowed.includes(adapter as typeof adapters[number])||adapter==='network.tcp-connect.v1'&&(!parsed.length||!!portsError)} onClick={()=>void prepare()}>Preparar plano da entrada selecionada</button><p className="muted">Preparar não executa a operação. Revise o plano, seus destinos e omissões antes de aprovar.</p></section>;
}
