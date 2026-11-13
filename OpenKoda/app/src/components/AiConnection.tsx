import { useEffect, useRef, useState } from 'react';
import { Check, ChevronRight, Eye, EyeOff, Plus, Search, X } from 'lucide-react';
import { native, request } from '../api';
import { knownProvider, providers, personalProviders, type ProviderId, type ProviderStatus } from '../ai-provider';
import { type Action, type Settings, errorMessage, shortDate } from '../types';
import { Field, Notice } from './shared';
import AiBudgetReview, { useBudgetReview } from './AiBudgetReview';
import './provider-models.css';
import './provider-setup.css';

type Props = {saved: Settings; draft: Settings; change: (key: 'provider'|'endpoint'|'model', value: string) => void; action: Action; busy: boolean};
const ids = personalProviders;
const fold = (value: string) => value.trim().toLocaleLowerCase('pt-BR');
type StatusMap = Partial<Record<ProviderId, ProviderStatus>>;

async function readStatus(provider: ProviderId) {
  const result = await request<ProviderStatus>('ai.status', {provider});
  if (typeof result?.hasApiKey !== 'boolean' || (result.provider ? result.provider !== provider : result.endpoint !== providers[provider].endpoint)) {
    throw new Error('Não foi possível confirmar o estado deste provedor.');
  }
  return result;
}

function stateLabel(status?: ProviderStatus, configured = true) {
  if (!status) return 'Estado indisponível';
  if (!status.hasApiKey) return 'Sem chave';
  if (!configured || status.enabledModels?.length === 0) return 'Aguardando ativação';
  return 'Configurado';
}

function isConfigured(id:ProviderId, status:ProviderStatus|undefined, saved:Settings) {
  if (!status?.hasApiKey) return false;
  return status.enabledModels == null ? saved.provider===id && !!saved.model.trim() : status.enabledModels.length>0;
}

export default function AiConnection(props: Props) {
  if (!native) return <PreviewConnection {...props}/>;
  return <NativeProviders saved={props.saved} action={props.action} busy={props.busy}/>;
}

function PreviewConnection({draft, change, busy}: Props) {
  const id = draft.provider;
  return <section className="content-section"><h3>Conexão</h3>
    <Field label="Provedor"><select value={id} disabled={busy} onChange={e => {
      const provider = e.target.value;
      change('provider', provider);
      change('endpoint', knownProvider(provider) ? providers[provider].endpoint : '');
      change('model', '');
    }}>
      <option value="compatible">Provedor compatível</option>
      {ids.map(id => <option value={id} key={id}>{providers[id].name}</option>)}
    </select></Field>
    <Field label="Endpoint"><input type="url" value={draft.endpoint} readOnly={knownProvider(id)} onChange={e=>change('endpoint',e.target.value)}/></Field>
    <Field label="Modelo"><input value={draft.model} maxLength={160} onChange={e=>change('model',e.target.value)}/></Field>
    <p className="muted">Abra o aplicativo desktop para conectar sua chave e consultar o catálogo.</p>
  </section>;
}

type Step = {page: 'providers'} | {page: 'key'|'models'; provider: ProviderId};
type Catalog = {models: string[]; revision: string};

function NativeProviders({saved, action, busy}: Pick<Props,'saved'|'action'|'busy'>) {
  const [active, setActive] = useState<ProviderId>(knownProvider(saved.provider) ? saved.provider : 'openai');
  const [statuses, setStatuses] = useState<StatusMap>({});
  const [step, setStep] = useState<Step | null>(null);
  const [preferDefault,setPreferDefault] = useState(false);
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');
  const [pending, setPending] = useState(false);
  const [reading, setReading] = useState(true);
  const inFlight = useRef(false);
  const isDefault = saved.provider === active;
  const status = statuses[active];
  const activated = isConfigured(active,status,saved);
  const provider = providers[active];
  const configured = ids.filter(id => id===active || id===saved.provider || statuses[id]?.hasApiKey);

  useEffect(() => {
    let mounted = true;
    void Promise.all(ids.map(async id => {
      try { return [id, await readStatus(id)] as const; }
      catch { return [id, undefined] as const; }
    })).then(results => {
      if (mounted) { setStatuses(previous => ({...Object.fromEntries(results), ...previous})); setReading(false); }
    });
    return () => { mounted=false; };
  }, []);

  function observe(id: ProviderId, result: ProviderStatus) {
    setStatuses(previous=>({...previous,[id]:result}));
  }
  function open(next: Step) { setNotice(''); setError(''); setStep(next); }
  function select(id: ProviderId) { setActive(id); setNotice(''); setError(''); }
  async function disconnect() {
    if (inFlight.current) return;
    inFlight.current=true; setPending(true); setError(''); setNotice('');
    try {
      await action('ai.key.remove', {provider:active});
      observe(active, await readStatus(active));
      setNotice('Credencial removida.');
    } catch (cause) {setError(errorMessage(cause));}
    finally {inFlight.current=false; setPending(false);}
  }
  async function refreshStatus() {
    setPending(true); setError('');
    try {observe(active, await readStatus(active));}
    catch (cause) {setError(errorMessage(cause));}
    finally {setPending(false);}
  }
  const disabled = busy || pending || reading;
  return <div className="provider-workspace">
    <aside className="provider-rail" aria-label="Provedores configurados">
      <h3>Provedores</h3>
      <div className="provider-rail-list">{configured.map(id => <button type="button" className="provider-row" key={id}
        aria-pressed={id===active} disabled={disabled} onClick={()=>select(id)}>
        <img src={providers[id].logo} width="26" height="26" alt=""/>
        <span><strong>{providers[id].name}</strong><small>{id===saved.provider && isConfigured(id,statuses[id],saved) ? 'Padrão da instalação' : stateLabel(statuses[id],isConfigured(id,statuses[id],saved))}</small></span>
      </button>)}</div>
      <button type="button" className="button provider-add" disabled={disabled} onClick={()=>open({page:'providers'})}>
        <Plus size={15} aria-hidden="true"/>Adicionar provedor</button>
    </aside>
    <div className="provider-detail">
      <header className="provider-identity"><img src={provider.logo} width="42" height="42" alt=""/>
        <div><h3>{provider.name}</h3><span className="provider-state" data-present={activated}>{reading ? 'Consultando…' : stateLabel(status,activated)}</span></div>
      </header>
      <section className="content-section"><h2>Conexão</h2>
        {!status && !reading && <button className="button" type="button" disabled={disabled} onClick={()=>void refreshStatus()}>Verificar estado da conexão</button>}
        {status && !status.hasApiKey && <><p className="muted">Conecte sua chave para escolher os modelos que deseja usar nas avaliações.</p>
          <button className="button primary" type="button" disabled={disabled} onClick={()=>open({page:'key',provider:active})}>Conectar com chave de API</button></>}
        {status?.hasApiKey && <>
          <div className="provider-connection-actions">
            <span className="provider-host">{new URL(provider.endpoint).hostname}</span>
            <button type="button" className="button" disabled={disabled} onClick={()=>open({page:'key',provider:active})}>Trocar chave</button>
            <button type="button" className="button" disabled={disabled} onClick={()=>void disconnect()}>Desconectar</button>
          </div>
          <div className="provider-selected-summary"><h3>Modelos nas avaliações</h3>
            <p className="muted">{status.enabledModels?.length ? status.enabledModels.length + (status.enabledModels.length===1?' modelo selecionado':' modelos selecionados')
              : !activated ? 'Sua chave está salva. Escolha os modelos para ativar o provedor.' : 'O catálogo está disponível para esta chave.'}</p>
            {isDefault && activated && <p className="provider-default-model"><Check size={15} aria-hidden="true"/>Padrão da instalação <strong>{saved.model}</strong></p>}
            <div className="provider-connection-actions">
              <button className={activated?'button':'button primary'} type="button" disabled={disabled} onClick={()=>open({page:'models',provider:active})}>
                {activated ? 'Gerenciar modelos' : 'Escolher modelos e ativar'}</button>
              {activated && !isDefault && <button type="button" className="button" disabled={disabled} onClick={()=>{setPreferDefault(true);open({page:'models',provider:active});}}>Definir como padrão</button>}
            </div>
            {!!status.enabledModels?.length && <ul className="provider-enabled-list" aria-label="Modelos selecionados">{status.enabledModels.map(id => <li key={id}>{id}</li>)}</ul>}
          </div>
          {isDefault && activated && <ConnectionTest key={active} status={status} model={saved.model} busy={disabled} action={action} onTested={()=>readStatus(active).then(result=>observe(active,result))}/>}
        </>}
      </section>
      {notice && <p role="status" className="saved-message">{notice}</p>}{error && <p role="alert">{error}</p>}
    </div>
    {step && <ProviderDialog initial={step} statuses={statuses} saved={saved} preferDefault={preferDefault} action={action}
      observe={observe} onKeySaved={id=>setActive(id)} onClose={()=>{setStep(null);setPreferDefault(false);}}
      onComplete={(id, result)=>{setActive(id);observe(id,result);setNotice('Modelos disponíveis nas avaliações.');setStep(null);setPreferDefault(false);}}/>}
  </div>;
}

type DialogProps = {
  initial:Step; statuses:StatusMap; saved:Settings; preferDefault:boolean; action:Action;
  observe:(provider:ProviderId,status:ProviderStatus)=>void; onKeySaved:(provider:ProviderId)=>void;
  onClose:()=>void; onComplete:(provider:ProviderId,status:ProviderStatus)=>void;
};

function ProviderDialog({initial, statuses, saved, preferDefault, action, observe, onKeySaved, onClose, onComplete}: DialogProps) {
  const [step,setStep] = useState(initial);
  const [query,setQuery] = useState('');
  const [key,setKey] = useState('');
  const [showKey,setShowKey] = useState(false);
  const [catalog,setCatalog] = useState<Catalog>();
  const [chosen,setChosen] = useState<string[]>([]);
  const [defaultModel,setDefaultModel] = useState('');
  const [makeDefault,setMakeDefault] = useState(false);
  const [error,setError] = useState('');
  const [loading,setLoading] = useState(false);
  const [saving,setSaving] = useState(false);
  const [attempt,setAttempt] = useState(0);
  const dialog = useRef<HTMLDialogElement>(null);
  const heading = useRef<HTMLHeadingElement>(null);
  const submitting = useRef(false);
  const provider = step.page==='providers' ? null : step.provider;
  const item = provider ? providers[provider] : null;
  const isDefault = provider === saved.provider;
  const current = provider ? statuses[provider] : undefined;
  const pendingActivation = !!provider && !isConfigured(provider,current,saved);
  useEffect(() => {
    const el=dialog.current;
    el?.showModal();
    return () => {el?.close();};
  }, []);
  useEffect(() => {heading.current?.focus();}, [step.page]);
  useEffect(() => {
    if (!provider || step.page!=='models') return;
    const id = provider;
    let mounted=true;
    setCatalog(undefined);setError('');setLoading(true);setQuery('');
    async function fetchModels() {
      // React Strict Mode can retire the first mount before it sends a request.
      await Promise.resolve();
      if (!mounted) return;
      try {
        const result=await request<{models:string[];provider:ProviderId;credentialRevision:string}>('ai.models',{provider:id});
        if (!mounted) return;
        if (result.provider!==id || typeof result.credentialRevision!=='string' || !result.credentialRevision
          || !Array.isArray(result.models) || result.models.some(model=>typeof model!=='string' || !model.trim())) {
          throw new Error('Não foi possível confirmar a lista de modelos desta chave. Consulte o catálogo novamente.');
        }
        const models=[...new Set(result.models)].sort();
        const legacyModel=isDefault && models.includes(saved.model) ? saved.model : '';
        const enabled=current?.enabledModels ?? (legacyModel ? [legacyModel] : []);
        const selected=models.filter(model=>enabled.includes(model));
        if (!selected.length && models.length===1) selected.push(models[0]);
        const defaultAvailable = knownProvider(saved.provider) && isConfigured(saved.provider,statuses[saved.provider],saved);
        setChosen(selected);
        setMakeDefault(isDefault || preferDefault || !defaultAvailable);
        setDefaultModel(selected.includes(saved.model) && isDefault ? saved.model : selected[0] ?? '');
        setCatalog({models,revision:result.credentialRevision});
      } catch (cause) {if (mounted) setError(errorMessage(cause));}
      finally {if (mounted) setLoading(false);}
    }
    void fetchModels();
    return ()=>{mounted=false;};
  }, [step.page, provider, attempt]);

  function go(next:Step) {
    setKey('');setShowKey(false);setQuery('');setError('');setCatalog(undefined);setStep(next);
  }
  async function saveKey() {
    if (!provider || submitting.current || !key.trim()) return;
    submitting.current=true;setSaving(true);setError('');
    try {
      await action('ai.key.save',{provider,key:key.trim(),prepareActivation:true});
      setKey('');setShowKey(false);
      onKeySaved(provider);
      // The save succeeded even if the provider is temporarily offline. Keep the key resumable.
      observe(provider,{provider,hasApiKey:true,verified:false,model:'',endpoint:providers[provider].endpoint,enabledModels:[],credentialRevision:null});
      go({page:'models',provider});
    } catch (cause) {setKey(''); setError(errorMessage(cause));}
    finally {submitting.current=false;setSaving(false);}
  }
  function toggle(model:string) {
    const next=chosen.includes(model) ? chosen.filter(id=>id!==model) : [...chosen,model];
    setChosen(next);
    if (!next.includes(defaultModel)) setDefaultModel(next[0] ?? '');
  }
  async function finish() {
    if (!provider || submitting.current || !catalog || !chosen.length || (makeDefault && !chosen.includes(defaultModel))) return;
    submitting.current=true;setSaving(true);setError('');
    try {
      const result=await action<ProviderStatus>('ai.provider.configure',{provider,credentialRevision:catalog.revision,models:chosen,defaultModel:makeDefault?defaultModel:null});
      onComplete(provider,result);
    } catch (cause) {setError(errorMessage(cause));}
    finally {submitting.current=false;setSaving(false);}
  }
  const title=step.page==='providers' ? 'Adicionar provedor' : (step.page==='key' ? 'Conectar ' : 'Escolher modelos · ') + item?.name;
  const matching=step.page==='providers' ? ids.filter(id=>fold(providers[id].name).includes(fold(query))) : [];
  const visible=catalog?.models.filter(model=>fold(model).includes(fold(query))) ?? [];
  return <dialog className="provider-dialog provider-setup-dialog" ref={dialog} aria-labelledby="provider-setup-title"
    onCancel={event=>{event.preventDefault();if(!submitting.current)onClose();}}>
    <div className="provider-dialog-content"><header className="provider-setup-heading">
      <h3 ref={heading} tabIndex={-1} id="provider-setup-title">{title}</h3>
      <button className="button" type="button" aria-label="Fechar" disabled={saving} onClick={onClose}><X size={16} aria-hidden="true"/></button>
    </header>
    {step.page!=='providers' && <ol className="provider-steps" aria-label="Etapas da conexão">
      <li><Check size={13} aria-hidden="true"/>Provedor</li>
      <li aria-current={step.page==='key'?'step':undefined}>{step.page==='models' && <Check size={13} aria-hidden="true"/>}Chave de API</li>
      <li aria-current={step.page==='models'?'step':undefined}>Modelos</li>
    </ol>}
    <div className="provider-setup-body">
      {step.page==='providers' && <>
        <label className="provider-search"><Search size={16} aria-hidden="true"/><input type="search" aria-label="Pesquisar provedores" placeholder="Pesquisar provedores" value={query} onChange={e=>setQuery(e.target.value)}/></label>
        <div className="provider-options">{matching.map(id=><button type="button" className="provider-row" key={id} onClick={()=>go({page:statuses[id]?.hasApiKey?'models':'key',provider:id})}>
          <img src={providers[id].logo} width="28" height="28" alt=""/><span><strong>{providers[id].name}</strong><small>{new URL(providers[id].endpoint).hostname}</small></span>
          <small>{statuses[id]?.hasApiKey ? stateLabel(statuses[id],isConfigured(id,statuses[id],saved)) : 'Chave de API'}</small><ChevronRight size={15} aria-hidden="true"/></button>)}</div>
        {!matching.length && <p role="status">Nenhum provedor corresponde à pesquisa.</p>}
      </>}
      {step.page==='key' && item && <form id="provider-key-form" onSubmit={event=>{event.preventDefault();void saveKey();}}>
        <div className="provider-endpoint"><img src={item.logo} width="30" height="30" alt=""/><div><strong>{item.name}</strong><small>{item.endpoint}</small></div></div>
        <label htmlFor="provider-key-input">Chave de API</label>
        <div className="provider-key-input"><input id="provider-key-input" type={showKey?'text':'password'} autoComplete="new-password" spellCheck={false}
          autoCapitalize="none" value={key} maxLength={512} disabled={saving} onChange={e=>setKey(e.target.value)}/>
          <button className="button" type="button" aria-label={showKey?'Ocultar chave':'Mostrar chave'} aria-pressed={showKey}
            onClick={()=>setShowKey(!showKey)}>{showKey?<EyeOff size={16}/>:<Eye size={16}/>}</button>
        </div><p className="muted">A chave fica no cofre deste dispositivo. O próximo passo consulta o catálogo de {item.name}.</p>
        {current?.hasApiKey && !key && provider && <button className="button" type="button" disabled={saving} onClick={()=>go({page:'models',provider})}>Usar a chave já salva</button>}
      </form>}
      {step.page==='models' && <>
        {loading && <p role="status">Buscando os modelos da sua chave…</p>}
        {!loading && !catalog?.models.length && <div className="provider-catalog-empty">
          {catalog && <p role="status">O provedor não retornou modelos para esta chave.</p>}
          <button className="button" type="button" onClick={()=>setAttempt(n=>n+1)}>Tentar novamente</button>
        </div>}
        {!!catalog?.models.length && <>
          <div className="provider-model-heading"><h4>Modelos para avaliações</h4><span>{chosen.length} de {catalog.models.length} selecionados</span></div>
          <p className="muted">As avaliações usam modelos de texto. O catálogo também pode incluir modelos de imagem, áudio e embeddings.</p>
          <label className="provider-search"><Search size={16} aria-hidden="true"/><input type="search" aria-label="Buscar modelos" placeholder="Buscar modelos" value={query} onChange={e=>setQuery(e.target.value)}/></label>
          <div className="provider-checklist" role="group" aria-label="Modelos do provedor">
            {visible.map(model=><label key={model}><input type="checkbox" checked={chosen.includes(model)} disabled={saving} onChange={()=>toggle(model)}/><span>{model}</span></label>)}
            {!visible.length && <p role="status">Nenhum modelo corresponde ao filtro.</p>}
          </div>
          <div className="provider-default-choice">
            {isDefault ? <span>Padrão da instalação</span> : <label><input type="checkbox" checked={makeDefault} disabled={saving} onChange={e=>setMakeDefault(e.target.checked)}/>Usar como padrão da instalação</label>}
            {makeDefault && <Field label="Modelo padrão"><select value={defaultModel} disabled={saving || !chosen.length} onChange={e=>setDefaultModel(e.target.value)}>
              {!chosen.length && <option value="">Marque um modelo acima</option>}{chosen.map(id=><option key={id} value={id}>{id}</option>)}
            </select></Field>}
          </div>
        </>}
      </>}
      {error && <div role="alert" className="provider-setup-error"><Notice tone="warning">{error}</Notice>
        {catalog && <button className="button" type="button" disabled={saving} onClick={()=>setAttempt(n=>n+1)}>Consultar catálogo novamente</button>}
      </div>}
    </div>
    <footer className="provider-setup-footer">
      {provider && <button className="button quiet" type="button" disabled={saving} onClick={()=>go(step.page==='models'?{page:'key',provider}:{page:'providers'})}>Voltar</button>}
      <div><button className="button" type="button" disabled={saving} onClick={onClose}>Cancelar</button>
        {step.page==='key' && <button className="button primary" type="submit" form="provider-key-form" disabled={saving || !key.trim()}>{saving?'Salvando…':'Salvar chave e buscar modelos'}</button>}
        {step.page==='models' && <button className="button primary" type="button" disabled={saving || loading || !catalog || !chosen.length || (makeDefault&&!defaultModel)} onClick={()=>void finish()}>
          {saving?'Salvando…':pendingActivation?'Ativar provedor':'Salvar modelos'}</button>}
      </div>
    </footer></div>
  </dialog>;
}

function ConnectionTest({status, model, busy, action, onTested}: {status:ProviderStatus; model:string; busy:boolean; action:Action; onTested:()=>Promise<void>}) {
  const [error,setError]=useState('');
  const [pending,setPending]=useState(false);
  const inFlight=useRef(false);
  const budget=useBudgetReview('test',{},undefined,JSON.stringify([status.provider,status.credentialRevision,model]));
  async function test() {
    if (inFlight.current) return;
    inFlight.current=true; setPending(true);setError('');
    try {
      const authorization=budget.authorization();budget.invalidate();
      await action('ai.test',authorization);
      await onTested();
    } catch (cause) {setError(errorMessage(cause));}
    finally {inFlight.current=false;setPending(false);void budget.refresh();}
  }
  return <div className="provider-test"><div className="inline-actions">
    <button type="button" className="button" disabled={busy || pending || !model || budget.pending || !budget.ready} onClick={()=>void test()}>Testar conexão</button>
    <span className="muted">{status.verified && status.model===model ? 'Conexão verificada' + (status.checkedAt ? ' · '+shortDate(status.checkedAt):'') : 'Conexão ainda não testada'}</span>
  </div><p className="muted">O teste envia uma mensagem curta ao modelo padrão e pode consumir créditos.</p>
  <AiBudgetReview budget={budget} disabled={busy || pending || !model}/>
  {error && <p role="alert">{error}</p>}</div>;
}
