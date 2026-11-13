import { conversationTitle } from '../chat-draft';
import { useEffect, useRef, useState } from 'react';
import { ArrowLeft, ArrowRight, FolderOpen, Github, Globe2, Network, SlidersHorizontal } from 'lucide-react';
import { native, request } from '../api';
import { knownProvider, selectionLabel, type AiSelection, type ProviderStatus } from '../ai-provider';
import { stageOpeningAnalysis } from '../chat-opening';
import { analysisFileAccessNotice, codeAnalysisProviderNotice, defaultScanLimit, SensitiveScanConfirmation, startSecurityScan, supportsSecurityModel } from '../security-scan';
import { parsePorts } from '../network';
import type { ScopeTargetRef } from '../combined-scope';
import type { ScopeRef } from '../scope';
import { errorMessage, type Action, type Execution, type ExecutionPlan, type Kind, type Payload, type Project, type Snapshot } from '../types';
import { ChatModelPicker } from './ChatModelPicker';
import ProjectTargetField from './ProjectTargetField';
import { Field, Notice } from './shared';
import './ProjectLaunch.css';
import { cancelGitHubImport, getGitHubImport, githubProjectName, importGitHubRepository, type GitHubImport } from '../github-project';
import { analysisContextLabels, type ProjectAnalysisOptions } from '../project-analysis-options';

interface Props {
  sourceOnly?:boolean;
  snapshot: Snapshot;
  action: Action;
  busy: boolean;
  initialProject?: Project;
  initialKind?: Kind;
  initialSource?:'local'|'github';
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

export default function ProjectLaunch({sourceOnly=false,snapshot, action, busy, initialProject, initialKind='code', initialSource='local', onCancel, onCreated,onModels}:Props) {
  const [kind,setKind] = useState<Kind>(sourceOnly?'code':initialProject?.kind ?? initialKind);
  const [source,setSource]=useState<'local'|'github'>(initialProject?'local':initialSource);
  const [githubUrl,setGitHubUrl]=useState('');
  const [imported,setImported]=useState<GitHubImport>();
  const [cancellingImport,setCancellingImport]=useState(false);
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
  const [scanLimit,setScanLimit] = useState(String(defaultScanLimit));
  const [customLimit,setCustomLimit]=useState(false);
  const [saved,setSaved] = useState<LaunchResult>();
  const [sensitiveReview,setSensitiveReview]=useState<{form:string;accepted:boolean}>();
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
  const confirmed=useRef<{content:string;created:LaunchResult}|undefined>(undefined);
  const creationRequest=useRef<{content:string;payload:Payload}|undefined>(undefined);
  const importFlow=useRef<AbortController|undefined>(undefined);
  const local=kind==='code' || kind==='files';
  const securityScan=native && local && supportsSecurityModel(selection);
  const unsupportedScan=native && local && !!selection && !supportsSecurityModel(selection);
  const needsModel=sourceOnly && !supportsSecurityModel(selection);
  const name=initialProject?.name ?? (source==='github'?githubProjectName(githubUrl):projectName(root,kind)).slice(0,120);
  const filtered=tools?.filter(tool=>compatibleTool(tool,kind)) ?? [];
  const selectedRipgrep=ripgrep && filtered.some(t=>t.adapter==='kali.rg.signals.v1' && t.adapterImplemented);
  const contextLabels=analysisContextLabels;
  const goal=[description.trim() || defaultGoals[kind],...(extra?context.flatMap((v,i)=>v.trim()?[contextLabels[i]+' — '+v.trim()]:[]):[])].join('\n\n');
  const parsedPorts=parsePorts(ports);
  const scanForm=JSON.stringify([source,githubUrl,root,kind,narrow?relative:null,goal,selection,advanced&&customLimit?scanLimit:null]);
  const review=sensitiveReview?.form===scanForm?sensitiveReview:undefined;
  const terminalImport=source==='github'&&!!imported&&['failed','cancelled','interrupted'].includes(imported.status);

  useEffect(()=>{
    if (!native || !advanced || sourceOnly) return;
    let mounted=true;
    setCatalogError('');
    void request<{tools:Tool[];inventoryVerified:boolean}>('packages.catalog').then(catalog=>{
      if(!mounted)return;
      if (!catalog.inventoryVerified) throw new Error('O inventário do Kali ainda não foi verificado.');
      setTools(catalog.tools);
    }).catch(cause=>{if(mounted){setTools(undefined);setCatalogError(errorMessage(cause));}});
    return ()=>{mounted=false;};
  },[advanced,attempt,sourceOnly]);

  async function launch(newDownload=false) {
    if (submittingRef.current || busy) return;
    if(terminalImport){
      if(!newDownload)return;
      // A confirmed terminal import can be retried only by this explicit action.
      // An unknown/lost receipt keeps the original key and cannot start a new clone.
      intent.current=null;creationRequest.current=undefined;setImported(undefined);
    }
    setError('');
    if (!(source==='github'?githubUrl.trim():root.trim()) || !name.trim() || narrow && !relative.trim()) {setError('Preencha o alvo da análise.');return;}
    if (goal.length>4000) {setError('Descrição e contexto juntos podem conter até 4.000 caracteres.');return;}
    if (kind==='network' && advanced && parsedPorts.error) {setError(parsedPorts.error);return;}
    if (unsupportedScan||needsModel) {setError(codeAnalysisProviderNotice);return;}
    const maxCostUsd=advanced&&customLimit?Number(scanLimit):defaultScanLimit;
    if (securityScan && (!Number.isFinite(maxCostUsd) || maxCostUsd<0.01 || maxCostUsd>100)) {setError('Informe um limite da análise entre US$ 0,01 e US$ 100.');return;}
    const relativeTarget=narrow && local ? source==='github'?relative.trim().replaceAll('\\','/'):projectEntry(root,relative) : null;
    if(relativeTarget!==null && (!relativeTarget || relativeTarget.split('/').some(part=>!part||part==='.'||part==='..'||part.includes(':')))) {
      setError('Escolha um arquivo ou pasta dentro da pasta do projeto.');return;
    }
    const customCap=Number(scanLimit),duration=Number(minutes);
    const analysisOptions:ProjectAnalysisOptions={advancedEnabled:advanced,additionalContextEnabled:extra,context:context as ProjectAnalysisOptions['context'],maxCostUsd:customLimit&&Number.isFinite(customCap)&&customCap>=.01&&customCap<=100?customCap:null,minutes:minutes&&Number.isInteger(duration)&&duration>=1&&duration<=1440?duration:null,title};
    const body = {projectId:initialProject?.id ?? null,name,kind,root:root.trim(),relativeTarget,
      title:(advanced && title.trim()) || conversationTitle(description.trim() || defaultGoals[kind]),objective:goal,aiSelection:selection,ports:kind==='network'?(advanced?parsedPorts.ports:[443]):[],useRipgrep:advanced && selectedRipgrep,
      minutes:advanced && minutes?Number(minutes):null,...(sourceOnly?{analysisOptions}:{})};
    // Optional future preferences do not create a second project when an admitted
    // analysis needs another consent/cost review. Scope changes remain distinct.
    const {analysisOptions:_options,...creation}=body;
    const content=JSON.stringify([creation,source,githubUrl.trim()]);
    if (intent.current?.content!==content) intent.current={content,key:crypto.randomUUID()};
    const flow=new AbortController();importFlow.current=flow;
    const requireActive=()=>{if(flow.signal.aborted)throw new Error('Criação cancelada. Se a cópia já foi baixada, ela permanece no armazenamento gerenciado. Nenhuma análise foi iniciada.');};
    submittingRef.current=true;setSubmitting(true);
    try {
      if (native && selection) {
        const status=await request<ProviderStatus>('ai.status',{provider:selection.provider});
        if (!status.hasApiKey || status.provider!==selection.provider || status.enabledModels && !status.enabledModels.includes(selection.model)) {
          throw new Error('Este modelo deixou de estar ativo. Selecione outro modelo nas configurações.');
        }
      }
      let repository:GitHubImport|undefined;
      if(source==='github'&&!initialProject&&confirmed.current?.content!==content){
        if(!native)throw new Error('Baixe o repositório pelo aplicativo desktop.');
        repository=await importGitHubRepository(githubUrl,intent.current.key);setImported(repository);
        const deadline=Date.now()+240000;
        while(repository.status==='downloading'){
          if(Date.now()>deadline)throw new Error('O download ainda não teve confirmação. Consulte novamente antes de repetir.');
          await new Promise(done=>setTimeout(done,900));repository=await getGitHubImport(repository.id);setImported(repository);
        }
        if(repository.status!=='ready')throw new Error(repository.message||'O download do repositório não foi concluído.');
      }
      requireActive();
      let created:LaunchResult;
      if(confirmed.current?.content===content){
        created=confirmed.current.created;
        if(sourceOnly)await action('project.options.save',{projectId:created.projectId,options:analysisOptions});
      }else{
        if(creationRequest.current?.content!==content)creationRequest.current={content,payload:{...body,...(repository?{root:repository.path,name:repository.name,githubImportId:repository.id}:{}),idempotencyKey:intent.current.key}};
        created=await action<LaunchResult>('project.launch',creationRequest.current.payload);
        confirmed.current={content,created};
        if(sourceOnly&&JSON.stringify(creationRequest.current.payload.analysisOptions)!==JSON.stringify(analysisOptions))await action('project.options.save',{projectId:created.projectId,options:analysisOptions});
      }
      setSaved(created);
      if(flow.signal.aborted)throw new Error('Criação cancelada antes da análise. O projeto que já havia sido criado foi preservado.');
      if(securityScan && selection){
        await startSecurityScan({projectId:created.projectId,assessmentId:created.assessmentId,selection,idempotencyKey:intent.current.key,prompt:goal,maxCostUsd,target:relativeTarget??undefined,acknowledgeSensitive:review?.accepted,signal:flow.signal});
        onCreated(created.projectId,created.assessmentId);
        return;
      }
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
    } catch (cause) {
      if(cause instanceof SensitiveScanConfirmation)setSensitiveReview({form:scanForm,accepted:false});
      setError(errorMessage(cause));
    }
    finally {if(importFlow.current===flow)importFlow.current=undefined;submittingRef.current=false;setSubmitting(false);}
  }
  const locked=busy||submitting;
  async function cancelImport(){
    if(!imported||imported.status!=='downloading'||cancellingImport)return;
    // The user's intent wins even if the host has just completed the clone.
    importFlow.current?.abort();
    setCancellingImport(true);setError('');
    try{setImported(await cancelGitHubImport(imported.id));}catch(cause){setError(errorMessage(cause));}finally{setCancellingImport(false);}
  }
  function chooseSource(next:'local'|'github'){setSource(next);setImported(undefined);setError('');}
  function chooseMode(next:Kind) {
    setKind(next);setRoot('');setRelative('');setNarrow(false);setTitle('');setRipgrep(false);setError('');
  }
  return <section className="launch-page">
    <header><button className="button quiet" onClick={onCancel} disabled={locked}><ArrowLeft size={16}/>Voltar</button>
      <h1>{initialProject?'Nova conversa':'Novo projeto'}</h1></header>
    <form onSubmit={event=>{event.preventDefault();void launch();}}>
      <fieldset disabled={locked}><legend className="launch-legend">{sourceOnly?'Código do projeto':'Tipo de análise'}</legend>
        {sourceOnly&&!initialProject&&<div className="launch-modes launch-sources" role="group" aria-label="Origem do código">
          <button type="button" aria-pressed={source==='local'} onClick={()=>chooseSource('local')}><FolderOpen size={18}/>Pasta local</button>
          <button type="button" aria-pressed={source==='github'} onClick={()=>chooseSource('github')}><Github size={18}/>GitHub</button>
        </div>}
        {!sourceOnly&&<div className="launch-modes">{modes.map(({kind:id,label,Icon})=><button key={id} type="button"
          disabled={!!initialProject} aria-pressed={(local?'code':kind)===id} onClick={()=>chooseMode(id)}>
          <Icon size={18}/>{label}</button>)}</div>}
        {source==='github'&&!initialProject?<Field label="Link do repositório no GitHub" hint="Informe um repositório público. O OpenKoda baixa uma cópia do código, sem executar o projeto."><input type="url" required value={githubUrl} maxLength={512} placeholder="https://github.com/usuario/repositorio" onChange={event=>{setGitHubUrl(event.target.value);setImported(undefined);}}/></Field>
          :<ProjectTargetField kind={kind} label={local?'Pasta do projeto':kind==='web'?'URL do site ou da API':'IP ou rede CIDR'} value={root}
            onChange={setRoot} disabled={!!initialProject || locked}/>}
        {name && <p className="launch-name">Projeto <strong>{name}</strong></p>}
        {local && <><Field label="Alvo"><select value={narrow?'subset':'all'} onChange={e=>setNarrow(e.target.value==='subset')}>
          <option value="all">Todo o código</option><option value="subset">Uma pasta ou arquivo</option></select></Field>
          {narrow && <Field label="Caminho dentro do projeto"><input required value={relative} maxLength={4096} placeholder="src/autenticacao.ts" onChange={e=>setRelative(e.target.value)}/></Field>}</>}
        <div className="launch-model"><strong>Modelo de IA</strong>
          <div><ChatModelPicker value={selection} onChange={setSelection} disabled={locked} onModels={onModels} forAnalysis={local}/>
          {selection && !sourceOnly && <button className="button quiet" type="button" onClick={()=>setSelection(null)}>Usar só a análise nativa</button>}</div></div>
        {(unsupportedScan||needsModel) && <Notice>{codeAnalysisProviderNotice}</Notice>}
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
            {securityScan&&<><label className="analysis-limit-toggle"><input type="checkbox" checked={customLimit} onChange={event=>setCustomLimit(event.target.checked)}/>Personalizar limite de custo</label>
              {customLimit&&<Field label="Limite da análise em USD" hint="O motor decide quando iniciar novas chamadas. Uma chamada em andamento pode ultrapassar este teto estimado."><input type="number" min="0.01" max="100" step="0.01" required value={scanLimit} onChange={event=>setScanLimit(event.target.value)}/></Field>}
              <small>Sem limite personalizado, o OpenKoda usa o limite padrão de US$ 5 por análise.</small></>}
            <Field label="Limite para iniciar operações (minutos)" hint="Em branco, o escopo não tem prazo de término."><input type="number" min={1} max={1440}
              value={minutes} onChange={e=>setMinutes(e.target.value)}/></Field>
            {kind==='network' && <Field label="Portas TCP"><input value={ports} onChange={e=>setPorts(e.target.value)} maxLength={200}/></Field>}
            {!sourceOnly&&<><h3>Ferramentas do Kali</h3>
            {catalogError?<p role="status">{catalogError} <button type="button" className="text-link" onClick={()=>setAttempt(n=>n+1)}>Verificar novamente</button></p>
            :!native?<p>O inventário está disponível no aplicativo desktop.</p>:!tools?<p role="status">Consultando ferramentas instaladas…</p>
            :!filtered.length?<p>Nenhuma ferramenta instalada com suporte a esta análise.</p>
            :filtered.map(tool=><label className="launch-tool" key={tool.package}><input type="checkbox"
              checked={selectedRipgrep} onChange={e=>setRipgrep(e.target.checked)}/>
              <span>{tool.package}<small>Escolha uma evidência na conversa para analisar no Kali.</small></span></label>)}</>}</div>}</section>
        <p className="launch-summary">{securityScan && selection
          ? 'Ao criar, o OpenKoda analisará o código selecionado com '+selectionLabel(selection)+'. O provedor poderá receber arquivos e contexto do projeto. A análise pode consumir créditos.'
          : unsupportedScan||needsModel ? 'Selecione um modelo de texto para criar o projeto e iniciar a análise.'
          : native ? local ? 'Ao criar, o OpenKoda verificará padrões de segurança nos arquivos selecionados, sem executar o código.'
          : kind==='web'?'Ao criar, o OpenKoda enviará uma requisição HEAD à URL informada, sem seguir redirecionamentos.'
            :'Ao criar, o OpenKoda tentará conexões TCP nas portas '+(advanced?ports:'443')+' do alvo informado, sem enviar conteúdo.'
          :'A prévia salva a conversa. A execução usa o aplicativo desktop.'}
          {native && selection && !local && ' A descrição, os metadados do escopo e o resumo da operação serão enviados a '+selectionLabel(selection)+' para análise. A chamada pode consumir créditos.'}</p>
        {native&&local&&<p className="launch-summary">{analysisFileAccessNotice}</p>}
        {error && <div role="alert"><Notice tone="warning">{error}</Notice></div>}
        {review&&selection&&<label className="check-field"><input type="checkbox" checked={review.accepted} onChange={e=>setSensitiveReview({form:scanForm,accepted:e.target.checked})}/>Autorizo enviar a fonte sensível desta avaliação a {selectionLabel(selection)} para esta análise.</label>}
        {error && saved && <button className="button" type="button" onClick={()=>onCreated(saved.projectId,saved.assessmentId)}>Abrir conversa criada</button>}
        {terminalImport&&<div className="inline-actions"><p>O download anterior terminou sem uma cópia pronta. Inicie outra tentativa para este mesmo repositório.</p><button type="button" className="button" disabled={locked||unsupportedScan||needsModel} onClick={()=>void launch(true)}>Tentar novo download</button></div>}
        <button type="submit" className="button primary launch-submit" disabled={locked || terminalImport || unsupportedScan || needsModel || !!review&&!review.accepted || !(source==='github'?githubUrl.trim():root.trim()) || narrow&&!relative.trim()}>
          {submitting?imported?.status==='downloading'?'Baixando repositório…':'Criando…':initialProject?'Criar conversa':'Criar projeto'}<ArrowRight size={16}/></button>
      </fieldset>
      {imported?.status==='downloading'&&<div className="github-import-progress" role="status"><p>{imported.message||'Baixando a cópia do repositório…'}</p><button type="button" className="button" disabled={cancellingImport} onClick={()=>void cancelImport()}>{cancellingImport?'Cancelando…':'Cancelar download'}</button></div>}
    </form>
  </section>;
}
