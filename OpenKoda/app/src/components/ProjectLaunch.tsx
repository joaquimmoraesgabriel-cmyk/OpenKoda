import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, ArrowRight, FolderOpen, Globe2, Network, SlidersHorizontal } from 'lucide-react';
import { native, request } from '../api';
import { knownProvider, selectionLabel, type AiSelection, type ProviderStatus } from '../ai-provider';
import { stageOpeningAnalysis } from '../chat-opening';
import { parsePorts } from '../network';
import type { ScopeTargetRef } from '../combined-scope';
import type { ScopeRef } from '../scope';
import { errorMessage, type Action, type Execution, type ExecutionPlan, type Kind, type Project, type Snapshot } from '../types';
import { ChatModelPicker } from './ChatModelPicker';
import ProjectTargetField from './ProjectTargetField';
import { Field, Notice } from './shared';
import './ProjectLaunch.css';

interface Props {
  snapshot: Snapshot;
  action: Action;
  busy: boolean;
  initialProject?: Project;
  onCancel: () => void;
  onCreated: (projectId: string, assessmentId: string) => void;
  onModels?: () => void;
}
type LaunchResult = {projectId:string;assessmentId:string;targetRef:ScopeTargetRef|null;scopeRef:ScopeRef;replayed:boolean};
type Tool = {package:string;installed:boolean|null;adapter:string;adapterImplemented?:boolean};
const modes = [
  {kind:'code', label:'Análise local', Icon:FolderOpen},
  {kind:'web', label:'Análise Web / API', Icon:Globe2},
  {kind:'network', label:'Análise de rede', Icon:Network},
] as const;
const defaultGoals = {
  code:'Verificar possíveis riscos de segurança no código selecionado.',
  files:'Verificar possíveis riscos de segurança nos arquivos selecionados.',
  web:'Revisar os cabeçalhos de segurança da URL informada.',
  network:'Verificar a conectividade nas portas autorizadas.',
};
const initialTitles:Record<Kind,string> = {
  code:'Análise inicial do código',
  files:'Revisão dos arquivos',
  web:'Revisão do site ou da API',
  network:'Verificação da rede',
};
export function projectName(root:string, kind:Kind) {
  const value = root.trim();
  if (kind==='web') {
    try {return new URL(value).host;} catch {return '';}
  }
  if (kind==='network') return value;
  return value.split(/[\\/]+/).filter(Boolean).at(-1) ?? '';
}
function compatibleTool(tool:Tool, kind:Kind) {
  return tool.installed===true && tool.adapterImplemented===true && tool.adapter==='kali.rg.signals.v1' &&
    (kind==='code' || kind==='files');
}
function projectEntry(root:string, input:string) {
  const directory=root.trim().replaceAll('\\','/').replace(/\/+$/,'')+'/';
  const path=input.trim().replaceAll('\\','/').replace(/\/+$/,'');
  const underRoot=/^[a-z]:\//i.test(directory)
    ? path.toLowerCase().startsWith(directory.toLowerCase()) : path.startsWith(directory);
  return underRoot ? path.slice(directory.length) : path;
}

async function startOperation(action:Action, created:LaunchResult, kind:Kind, ports:number[]) {
  const adapter = kind==='network'?'network.tcp-connect.v1':kind==='web'?'http.headers.v1':'source.rules.v1';
  const {assessmentId, targetRef, scopeRef} = created;
  const plan=await action<ExecutionPlan>('assessment.launchPlan',{assessmentId,scopeRef,targetRef,...(kind==='network'?{ports}:{})});
  if (!plan?.id || !plan.digest || plan.assessmentId!==assessmentId || plan.adapter!==adapter) {
    throw new Error('O plano preparado não corresponde à análise selecionada.');
  }
  const decision={id:plan.id,digest:plan.digest};
  await action('plan.approve',decision);
  return action<Execution>('plan.run',decision);
}

export default function ProjectLaunch({snapshot, action, busy, initialProject, onCancel, onCreated,onModels}:Props) {
  const [kind,setKind] = useState<Kind>(initialProject?.kind ?? 'code');
  const [root,setRoot] = useState(initialProject?.target ?? '');
  const [narrow,setNarrow] = useState(false);
  const [relative,setRelative] = useState('');
  const [description,setDescription] = useState('');
  const [extra,setExtra] = useState(false);
  const [context,setContext] = useState(['','','']);
  const [advanced,setAdvanced] = useState(false);
  const [title,setTitle] = useState('');
  const [minutes,setMinutes] = useState('');
  const [ports,setPorts] = useState('443');
  const [ripgrep,setRipgrep] = useState(false);
  const [tools,setTools] = useState<Tool[]>();
  const [catalogError,setCatalogError] = useState('');
  const [attempt,setAttempt] = useState(0);
  const [chosenModel,setSelection] = useState<AiSelection|null>();
  const configuredModel = snapshot.settings.hasApiKey && knownProvider(snapshot.settings.provider) && snapshot.settings.model
    ? {provider:snapshot.settings.provider,model:snapshot.settings.model}:null;
  const selection = chosenModel===undefined ? configuredModel : chosenModel;
  const [submitting,setSubmitting] = useState(false);
  const [error,setError] = useState('');
  const submittingRef = useRef(false);
  const intent = useRef<{content:string;key:string}|null>(null);
  const local=kind==='code' || kind==='files';
  const name=initialProject?.name ?? projectName(root,kind).slice(0,120);
  const filtered=tools?.filter(tool=>compatibleTool(tool,kind)) ?? [];
  const selectedRipgrep=ripgrep && filtered.some(t=>t.adapter==='kali.rg.signals.v1' && t.adapterImplemented);
  const contextLabels=['Vetores de ataque','Áreas de foco','Contexto adicional de segurança'];
  const goal=[description.trim() || defaultGoals[kind],...(extra?context.flatMap((v,i)=>v.trim()?[contextLabels[i]+' — '+v.trim()]:[]):[])].join('\n\n');
  const parsedPorts=parsePorts(ports);

  useEffect(()=>{
    if (!native || !advanced) return;
    let mounted=true;
    setCatalogError('');
    void request<{tools:Tool[];inventoryVerified:boolean}>('packages.catalog').then(catalog=>{
      if(!mounted)return;
      if (!catalog.inventoryVerified) throw new Error('O inventário do Kali ainda não foi verificado.');
      setTools(catalog.tools);
    }).catch(cause=>{if(mounted){setTools(undefined);setCatalogError(errorMessage(cause));}});
    return ()=>{mounted=false;};
  },[advanced,attempt]);

  async function launch() {
    if (submittingRef.current || busy) return;
    setError('');
    if (!root.trim() || !name.trim() || narrow && !relative.trim()) {setError('Preencha o alvo da análise.');return;}
    if (goal.length>4000) {setError('Descrição e contexto juntos podem conter até 4.000 caracteres.');return;}
    if (kind==='network' && advanced && parsedPorts.error) {setError(parsedPorts.error);return;}
    const relativeTarget=narrow && local ? projectEntry(root,relative) : null;
    if(relativeTarget!==null && (!relativeTarget || relativeTarget.split('/').some(part=>!part||part==='.'||part==='..'||part.includes(':')))) {
      setError('Escolha um arquivo ou pasta dentro da pasta do projeto.');return;
    }
    const body = {projectId:initialProject?.id ?? null,name,kind,root:root.trim(),relativeTarget,
      title:(advanced && title.trim()) || initialTitles[kind],objective:goal,aiSelection:selection,ports:kind==='network'?(advanced?parsedPorts.ports:[443]):[],useRipgrep:advanced && selectedRipgrep,
      minutes:advanced && minutes?Number(minutes):null};
    const content=JSON.stringify(body);
    if (intent.current?.content!==content) intent.current={content,key:crypto.randomUUID()};
    submittingRef.current=true;setSubmitting(true);
    try {
      if (native && selection) {
        const status=await request<ProviderStatus>('ai.status',{provider:selection.provider});
        if (!status.hasApiKey || status.provider!==selection.provider || status.enabledModels && !status.enabledModels.includes(selection.model)) {
          throw new Error('Este modelo deixou de estar ativo. Selecione outro modelo ou use só a análise nativa.');
        }
      }
      const created=await action<LaunchResult>('project.launch',{...body,idempotencyKey:intent.current.key});
      onCreated(created.projectId,created.assessmentId);
      if (!native) return;
      if (created.replayed) {
        const restored=await request<Snapshot>('snapshot');
        if (restored.assessments.find(a=>a.id===created.assessmentId)?.status!=='draft' ||
            restored.plans?.some(p=>p.assessmentId===created.assessmentId)) return;
      }
      try {
        const execution=await startOperation(action,created,kind,body.ports);
        if (selection && ['completed','failed','partial'].includes(execution.status)) stageOpeningAnalysis(created.assessmentId,goal);
      } catch (cause) {
        // Preparation failures are recorded on the assessment by the native executor.
        const state=await request<Snapshot>('snapshot');
        const current=state.assessments.find(a=>a.id===created.assessmentId);
        if (selection && current?.status==='blocked') stageOpeningAnalysis(created.assessmentId,goal);
        throw cause;
      }
    } catch (cause) {setError(errorMessage(cause));}
    finally {submittingRef.current=false;setSubmitting(false);}
  }
  const locked=busy||submitting;
  function chooseMode(next:Kind) {
    setKind(next);setRoot('');setRelative('');setNarrow(false);setTitle('');setRipgrep(false);setError('');
  }
  return <section className="launch-page">
    <header><button className="button quiet" onClick={onCancel} disabled={locked}><ArrowLeft size={16}/>Voltar</button>
      <h1>{initialProject?'Nova conversa':'Novo projeto'}</h1></header>
    <form onSubmit={event=>{event.preventDefault();void launch();}}>
      <fieldset disabled={locked}><legend className="launch-legend">Tipo de análise</legend>
        <div className="launch-modes">{modes.map(({kind:id,label,Icon})=><button key={id} type="button"
          disabled={!!initialProject} aria-pressed={(local?'code':kind)===id} onClick={()=>chooseMode(id)}>
          <Icon size={18}/>{label}</button>)}</div>
        <ProjectTargetField kind={kind} label={local?'Pasta do projeto':kind==='web'?'URL do site ou da API':'IP ou rede CIDR'} value={root}
          onChange={setRoot} disabled={!!initialProject || locked}/>
        {name && <p className="launch-name">Projeto <strong>{name}</strong></p>}
        {local && <><Field label="Alvo"><select value={narrow?'subset':'all'} onChange={e=>setNarrow(e.target.value==='subset')}>
          <option value="all">Todo o código</option><option value="subset">Uma pasta ou arquivo</option></select></Field>
          {narrow && <Field label="Caminho dentro do projeto"><input required value={relative} maxLength={4096} placeholder="src/autenticacao.ts" onChange={e=>setRelative(e.target.value)}/></Field>}</>}
        <div className="launch-model"><strong>Modelo de IA</strong>
          <div><ChatModelPicker value={selection} onChange={setSelection} disabled={locked} onModels={onModels}/>
          {selection && <button className="button quiet" type="button" onClick={()=>setSelection(null)}>Usar só a análise nativa</button>}</div></div>
        <Field label="Descrição"><textarea rows={3} value={description} maxLength={4000} onChange={e=>setDescription(e.target.value)}
          placeholder={defaultGoals[kind]}/></Field>
        <section className="launch-disclosure"><label><strong>Contexto adicional</strong><input type="checkbox" role="switch" checked={extra}
          onChange={e=>setExtra(e.target.checked)}/></label>
          {extra && <div>{contextLabels.map((label,i)=><Field key={label} label={label}><textarea rows={2}
            value={context[i]} onChange={e=>setContext(old=>old.map((s,n)=>n===i?e.target.value:s))} maxLength={2000}
            placeholder={['Ex.: contorno da autenticação','Ex.: uploads e área administrativa','Ex.: existem dois níveis de acesso'][i]}/></Field>)}</div>}</section>
        <section className="launch-disclosure"><label><strong><SlidersHorizontal size={16}/>Opções avançadas</strong><input type="checkbox" role="switch"
          checked={advanced} onChange={e=>setAdvanced(e.target.checked)}/></label>
          {advanced && <div><Field label="Título da conversa"><input value={title} onChange={e=>setTitle(e.target.value)} placeholder={initialTitles[kind]} maxLength={160}/></Field>
            <Field label="Limite para iniciar operações (minutos)" hint="Em branco, o escopo não tem prazo de término."><input type="number" min={1} max={1440}
              value={minutes} onChange={e=>setMinutes(e.target.value)}/></Field>
            {kind==='network' && <Field label="Portas TCP"><input value={ports} onChange={e=>setPorts(e.target.value)} maxLength={200}/></Field>}
            <h3>Ferramentas do Kali</h3>
            {catalogError?<p role="status">{catalogError} <button type="button" className="text-link" onClick={()=>setAttempt(n=>n+1)}>Verificar novamente</button></p>
            :!native?<p>O inventário está disponível no aplicativo desktop.</p>:!tools?<p role="status">Consultando ferramentas instaladas…</p>
            :!filtered.length?<p>Nenhuma ferramenta instalada com suporte a esta análise.</p>
            :filtered.map(tool=><label className="launch-tool" key={tool.package}><input type="checkbox"
              checked={selectedRipgrep} onChange={e=>setRipgrep(e.target.checked)}/>
              <span>{tool.package}<small>Escolha uma evidência na conversa para analisar no Kali.</small></span></label>)}</div>}</section>
        <p className="launch-summary">{native ? local ? 'Ao criar, o OpenKoda verificará padrões de segurança nos arquivos selecionados, sem executar o código.'
          : kind==='web'?'Ao criar, o OpenKoda enviará uma requisição HEAD à URL informada, sem seguir redirecionamentos.'
            :'Ao criar, o OpenKoda tentará conexões TCP nas portas '+(advanced?ports:'443')+' do alvo informado, sem enviar conteúdo.'
          :'A prévia salva a conversa. A execução usa o aplicativo desktop.'}
          {native && selection && ' A descrição, os metadados do escopo e o resumo da operação serão enviados a '+selectionLabel(selection)+' para análise. A chamada pode consumir créditos.'}</p>
        {error && <div role="alert"><Notice tone="warning">{error}</Notice></div>}
        <button type="submit" className="button primary launch-submit" disabled={locked || !root.trim() || narrow&&!relative.trim()}>
          {submitting?'Criando…':initialProject?'Criar conversa':'Criar projeto'}<ArrowRight size={16}/></button>
      </fieldset>
    </form>
  </section>;
}
