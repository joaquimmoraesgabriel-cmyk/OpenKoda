import { useState } from 'react';
import type { ScopeMethod } from '../scope';
import { currentScopeRef, targetAllows } from '../combined-scope';
import { kindLabels } from '../types';
import { useScopeAccess } from './ScopeAccess';
import { Field, Notice } from './shared';

export function useContextTargets(method?:ScopeMethod){
 const scope=useScopeAccess();const combined=!!scope.status?.revision?.body.targetSet;
 const [selection,setSelection]=useState<{revision:string;ids:string[]}>({revision:'',ids:[]});
 const ids=selection.revision===scope.revision?selection.ids:[];
 const options=scope.status?.targetOptions??[];
 const selected=options.filter(o=>ids.includes(o.entry.entryId));
 const ready=!combined||selected.length>0&&selected.length===ids.length&&selected.every(o=>!method||targetAllows(o,method));
 return {combined,options,ids,selected,ready,key:JSON.stringify([scope.revision,ids]),payload:combined?{contextTargetRefs:selected.map(o=>o.targetRef)}:{},setIds:(ids:string[])=>setSelection({revision:scope.revision,ids}),method};
}
export function ContextTargets({selection,label,disabled=false}:{selection:ReturnType<typeof useContextTargets>;label:string;disabled?:boolean}){
 if(!selection.combined)return null;
 return <fieldset className="context-selection" disabled={disabled}><legend>{label}</legend><p className="muted">Selecione as entradas deste contexto. Evidências de uma entrada não selecionada serão recusadas pelo núcleo; nenhuma fonte original é lida por esta seleção.</p>{selection.options.map(o=><label className="check-field" key={o.entry.entryId}><input type="checkbox" checked={selection.ids.includes(o.entry.entryId)} disabled={!!selection.method&&!targetAllows(o,selection.method)} onChange={e=>selection.setIds(e.target.checked?[...selection.ids,o.entry.entryId]:selection.ids.filter(id=>id!==o.entry.entryId))}/><span>{o.entry.label} · {kindLabels[o.entry.kind]}<small className="break-word">{o.entry.target}</small></span></label>)}{!selection.ready&&<Notice>Escolha ao menos uma entrada com o método permitido para continuar.</Notice>}</fieldset>;
}
export function useOperationTarget(method?:ScopeMethod){
 const scope=useScopeAccess();const combined=!!scope.status?.revision?.body.targetSet;const [selection,setSelection]=useState({revision:'',id:''});
 const id=selection.revision===scope.revision?selection.id:'';const options=scope.status?.targetOptions??[];const option=options.find(o=>o.entry.entryId===id);
 const ready=!combined||!!option&&(!method||targetAllows(option,method));
 return {combined,id,option,options,ready,key:JSON.stringify([scope.revision,id]),setId:(id:string)=>setSelection({revision:scope.revision,id}),payload:combined&&option?{scopeRef:currentScopeRef(scope.status),targetRef:option.targetRef}:{},method};
}
export function OperationTarget({selection,label='Entrada desta operação',disabled=false}:{selection:ReturnType<typeof useOperationTarget>;label?:string;disabled?:boolean}){
 if(!selection.combined)return null;
 return <Field label={label} hint="A escolha vale somente para esta revisão. Nenhuma entrada é escolhida automaticamente."><select value={selection.id} disabled={disabled} onChange={e=>selection.setId(e.target.value)}><option value="">Selecione uma entrada</option>{selection.options.map(o=><option key={o.entry.entryId} value={o.entry.entryId} disabled={!!selection.method&&!targetAllows(o,selection.method)}>{o.entry.label} · {kindLabels[o.entry.kind]} · {o.entry.target}</option>)}</select></Field>;
}
