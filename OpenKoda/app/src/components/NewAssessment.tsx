import CombinedScope from './CombinedScope';
import type { Action } from '../types';
import { useEffect, useRef, useState } from 'react';
import { SlidersHorizontal } from 'lucide-react';
import { native, request } from '../api';
import { Kind, Payload, Project, Settings, errorMessage, kindLabels } from '../types';
import { ScopeMethod, ScopeOptions, ScopeWindow, validScopeCatalog, validScopeSelection, validScopeWindow } from '../scope';
import { Disclosure, Empty, Field, KeyValue, Notice } from './shared';
import { ScopeBodyView, ScopeFields, ScopeReasons } from './ScopeFields';
import { KindIcon } from './Projects';

type CreateIntent = { form: string; content: string; payload: Payload; scoped: boolean };

function SingleAssessment({projects,settings,initialProject,onCreate,onCancel,onProjects,busy}:{projects:Project[];settings:Settings;initialProject?:string;onCreate:(p:Payload,start:boolean)=>Promise<void>;onCancel:()=>void;onProjects:()=>void;busy:boolean}){
 const active=projects.filter(p=>!p.archived);const [projectId,setProjectId]=useState(initialProject??active[0]?.id??'');const project=active.find(p=>p.id===projectId);const [title,setTitle]=useState('');const [objective,setObjective]=useState('');const [review,setReview]=useState(false);const [methods,setMethods]=useState<ScopeMethod[]>([]);const [window,setWindow]=useState<ScopeWindow>({startsAt:null,endsAt:null});const [options,setOptions]=useState<ScopeOptions>();const [loading,setLoading]=useState(false);const [error,setError]=useState('');const [ack,setAck]=useState(false);const [pending,setPending]=useState(false);const generation=useRef(0);const inFlight=useRef(false);
 useEffect(()=>{if(project&&!title)setTitle(project.name);},[project,title]);
 async function loadOptions(){if(!native||!project)return;const call=++generation.current;setLoading(true);setOptions(undefined);setError('');setAck(false);setReview(false);try{const next=await request<ScopeOptions>('assessment.scopeOptions',{projectId:project.id});if(next.schema!=='openkoda.scope-options.v1'||next.projectId!==project.id||next.kind!==project.kind||next.target!==project.target||!validScopeCatalog(next.methodCatalog)||!next.projectDigest||!next.catalogVersion||typeof next.canCreate!=='boolean')throw new Error('As opções não correspondem ao projeto selecionado. Nenhuma autorização foi presumida.');if(call===generation.current)setOptions(next);}catch(e){if(call===generation.current)setError(errorMessage(e));}finally{if(call===generation.current)setLoading(false);}}
 useEffect(()=>{setMethods([]);setWindow({startsAt:null,endsAt:null});setReview(false);setAck(false);void loadOptions();return()=>{generation.current++;};},[projectId,project?.target,project?.kind,project?.archived]);
 const payload={projectId,title:title.trim(),objective:objective.trim(),kind:project?.kind,target:project?.target};const current=options?.projectId===projectId?options:undefined;const valid=!!project&&!!title.trim()&&!!objective.trim()&&(!native||!!current?.canCreate&&validScopeSelection(methods,current.methodCatalog)&&validScopeWindow(window));const disabled=busy||pending;
 const submitted = useRef<CreateIntent | null>(null);
 const form = JSON.stringify([payload, [...methods].sort(), window]);
 async function submit(intent:CreateIntent) {
   if (inFlight.current) return;
   inFlight.current=true;setPending(true);setError('');setAck(false);
   try {
     await onCreate(intent.payload,false);
   } catch(e) {
     const message=errorMessage(e);
     setReview(false);
     if(native)await loadOptions();
     setError(message);
   } finally {
     inFlight.current=false;setPending(false);
   }
 }
 async function save(scoped:boolean) {
   if(inFlight.current||!valid||scoped&&(!current||!ack))return;
   const body:Payload={...payload};
   if (scoped && current) body.scope={methods:[...methods].sort(),window,projectDigest:current.projectDigest,catalogVersion:current.catalogVersion,acknowledgeScope:true};
   const content=JSON.stringify(body);
   const previous=submitted.current;
   // A background catalog refresh does not change the request the user already submitted.
   const sameIntent = previous && (previous.content === content || (previous.form === form && previous.scoped === scoped));
   const intent = sameIntent ? previous : {form,content,scoped,payload:{...body,idempotencyKey:crypto.randomUUID()}};
   submitted.current=intent;
   await submit(intent);
 }
 const retry = submitted.current?.form === form ? submitted.current : null;
 const previousScope = retry?.payload.scope as {projectDigest:string;catalogVersion:string} | undefined;
 const catalogChanged = current && previousScope && (previousScope.projectDigest !== current.projectDigest || previousScope.catalogVersion !== current.catalogVersion);
 if(!active.length)return <Empty title="Adicione um projeto primeiro" description="A avaliação precisa de um alvo cadastrado para definir seu escopo." action={<button className="button primary" onClick={onProjects}>Adicionar projeto</button>}/>;
 return <div className="form-page"><div className="detail-heading"><div><h1>{review?'Revisar avaliação':'Nova avaliação'}</h1><p>{review?'Confira o objetivo e o escopo antes de salvar.':'Defina o trabalho, os métodos e seus limites.'}</p></div></div>{error&&<div role="alert"><Notice tone="warning">{error} Nenhuma criação será repetida automaticamente.</Notice>
   {retry&&<button type="button" className="button" disabled={disabled} onClick={()=>void submit(retry)}>Reenviar a mesma solicitação</button>}
   {catalogChanged&&<button type="button" className="button" disabled={disabled||!valid} onClick={()=>{submitted.current=null;setReview(true);setAck(false);setError('');}}>Iniciar outra solicitação com o escopo atualizado</button>}
 </div>}
 {!review?<form onSubmit={e=>{e.preventDefault();if(valid){setReview(true);setAck(false);}}}><div className="segmented" aria-label="Modalidade da avaliação">{Object.entries(kindLabels).map(([k,l])=><button key={k} type="button" className={project?.kind===k?'active':''} onClick={()=>{const p=active.find(p=>p.kind===k);if(p){setProjectId(p.id);setTitle(p.name);}}} disabled={disabled||!active.some(p=>p.kind===k)} title={!active.some(p=>p.kind===k)?'Cadastre um projeto desta modalidade':l}><KindIcon kind={k as Kind}/>{l}</button>)}</div>
 <div className="form-group"><Field label="Projeto"><select value={projectId} disabled={disabled} onChange={e=>{setProjectId(e.target.value);setTitle(active.find(p=>p.id===e.target.value)?.name??'');}} required>{active.map(p=><option key={p.id} value={p.id}>{p.name}</option>)}</select></Field><Field label="Título"><input value={title} disabled={disabled} onChange={e=>setTitle(e.target.value)} required maxLength={160}/></Field><Field label="Alvo"><input value={project?.target??''} readOnly/></Field><Field label="Objetivo"><textarea value={objective} disabled={disabled} onChange={e=>setObjective(e.target.value)} required maxLength={4000} rows={2} placeholder="Ex.: revisar autenticação e acesso aos dados"/></Field><Field label="Escopo"><input value="Um alvo, conforme o projeto selecionado" readOnly/></Field><Field label="Modo"><input value="Assistido" readOnly/></Field><Field label="Modelo"><input value={settings.model||'Não configurado'} readOnly/></Field></div>
 {native?<><button type="button" className="button" disabled={disabled||loading} onClick={()=>void loadOptions()}>Atualizar opções de escopo</button>{loading&&<p role="status">Consultando opções do projeto…</p>}{current&&<><ScopeReasons reasons={current.reasonCodes}/><ScopeFields catalog={current.methodCatalog} methods={methods} window={window} disabled={disabled||!current.canCreate} onMethods={setMethods} onWindow={setWindow}/>{!validScopeWindow(window)&&<Notice tone="warning">Preencha instantes válidos; o início deve preceder o término.</Notice>}</>}</>:<Notice>A prévia permite salvar rascunhos de demonstração. Escopo versionado e autorização de operações pertencem ao aplicativo desktop.</Notice>}
 <Disclosure label="Limites e opções avançadas" icon={<SlidersHorizontal size={19}/>}><KeyValue label="Tempo por operação">{settings.timeoutSeconds} segundos</KeyValue><KeyValue label="Limite de saída">{settings.outputLimitKiB} KiB</KeyValue><p className="muted">Os limites da instalação são configurados em Configurações. Um método selecionado não confirma disponibilidade do executor.</p></Disclosure><div className="form-actions"><button className="button" type="button" onClick={onCancel} disabled={disabled}>Cancelar</button><button className="button primary" type="submit" disabled={disabled||!valid}>Revisar avaliação</button></div></form>:<><KeyValue label="Projeto">{project?.name}</KeyValue><KeyValue label="Título">{title}</KeyValue>{project&&<ScopeBodyView body={{objective:objective.trim(),kind:project.kind,target:project.target,methods,window}} catalog={current?.methodCatalog??[]}/>}<Notice>Salvar apenas cria o rascunho. Preparar um plano, aprovar e executar são ações posteriores e separadas. Nenhum dado será enviado à IA nesta etapa.</Notice>{native&&<label className="check-field"><input type="checkbox" checked={ack} disabled={disabled} onChange={e=>setAck(e.target.checked)}/><span>Revisei o objetivo, os métodos e a janela desta avaliação. Cada operação ainda exige suas autorizações próprias.</span></label>}<div className="form-actions"><button className="button" onClick={()=>{setReview(false);setAck(false);}} disabled={disabled}>Voltar</button><button className="button" onClick={()=>void save(false)} disabled={disabled}>{native?'Salvar rascunho sem revisar escopo':'Salvar rascunho'}</button>{native&&<button className="button primary" onClick={()=>void save(true)} disabled={disabled||!ack||!valid}>Salvar avaliação com escopo revisado</button>}</div></>}
 </div>;
}

export default function NewAssessment(props:Parameters<typeof SingleAssessment>[0]&{action:Action;onCombinedCreated:(id:string)=>void}){
 const [combined,setCombined]=useState(false);
 return <>{native&&<div className="inline-actions" aria-label="Tipo de avaliação"><button type="button" className="button" aria-pressed={!combined} disabled={props.busy} onClick={()=>setCombined(false)}>Um alvo</button><button type="button" className="button" aria-pressed={combined} disabled={props.busy||!props.projects.some(p=>!p.archived)} onClick={()=>setCombined(true)}>Combinar fontes e alvos</button></div>}{combined?<CombinedScope projects={props.projects} initialProject={props.initialProject} action={props.action} busy={props.busy} onDone={props.onCombinedCreated} onCancel={()=>setCombined(false)}/>:<SingleAssessment {...props}/>}</>;
}
