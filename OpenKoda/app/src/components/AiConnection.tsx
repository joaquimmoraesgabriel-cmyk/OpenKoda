import { useEffect, useRef, useState } from 'react';
import { native, request } from '../api';
import { knownProvider, providers, type ProviderId, type ProviderStatus } from '../ai-provider';
import { Action, Settings, errorMessage, shortDate } from '../types';
import { Field, Notice } from './shared';
import AiBudgetReview, { useBudgetReview } from './AiBudgetReview';
import { ModelCatalogField } from './ProviderModels';
import { Plus, Search, X } from 'lucide-react';

type Props = {draft: Settings; dirty: boolean; change: (key: 'provider'|'endpoint'|'model', value: string) => void; action: Action; busy: boolean};

const providerIds = Object.keys(providers) as ProviderId[];
type KeyStatus = 'saved' | 'missing' | 'unknown';
const keyLabel = (status?: KeyStatus) => status === 'saved' ? 'Chave cadastrada' : status === 'missing' ? 'Sem chave' : 'Estado indisponível';

export default function AiConnection({saved, ...props}: Props & {saved: Settings}) {
  const { draft, busy, change } = props;
  const [keys, setKeys] = useState<Partial<Record<ProviderId, KeyStatus>>>({});
  const [filter, setFilter] = useState('');
  const dialog = useRef<HTMLDialogElement>(null);
  const active = knownProvider(draft.provider) ? draft.provider : null;
  const heading = useRef<HTMLHeadingElement>(null);
  const focusDetail = useRef(false);
  const [locked, setLocked] = useState(false);

  useEffect(() => {
    if (!native) return;
    let current = true;
    void Promise.all(providerIds.map(async id => {
      try {
        const result = await request<ProviderStatus>('ai.status', {provider: id});
        if (result.provider === id || (!result.provider && result.endpoint === providers[id].endpoint)) {
          return [id, result.hasApiKey ? 'saved' : 'missing'] as const;
        }
      } catch { /* The detail view exposes a failed status read when selected. */ }
      return [id, 'unknown'] as const;
    })).then(entries => {
      if (current) setKeys(existing => ({...Object.fromEntries(entries), ...existing}));
    });
    return () => {current = false;};
  }, []);
  useEffect(() => {
    if (focusDetail.current) {heading.current?.focus(); focusDetail.current = false;}
  }, [active]);
  function choose(id: ProviderId) {
    dialog.current?.close();
    if (id === active) {heading.current?.focus(); return;}
    focusDetail.current = true;
    change('provider', id);
    change('endpoint', providers[id].endpoint);
    change('model', id === saved.provider ? saved.model : '');
  }
  function markKey(id: ProviderId, present: boolean) {
    setKeys(current => ({...current, [id]: present ? 'saved' : 'missing'}));
  }

  const configured = providerIds.filter(id => id === active || id === saved.provider || keys[id] === 'saved');
  const choices = providerIds.filter(id => providers[id].name.toLocaleLowerCase('pt-BR').includes(filter.trim().toLocaleLowerCase('pt-BR')));
  return <div className="provider-workspace">
    <aside className="provider-rail" aria-label="Provedores configurados">
      <h3>Provedores</h3>
      <div className="provider-rail-list">{!active && <span className="provider-row"><strong>Provedor compatível</strong></span>}{configured.map(id => <button type="button" className="provider-row" key={id}
        aria-pressed={id === active} disabled={busy || locked} onClick={() => choose(id)}>
        <img src={providers[id].logo} alt="" width="25" height="25"/>
        <span><strong>{providers[id].name}</strong><small>{id === saved.provider ? 'Padrão da instalação' : keyLabel(keys[id])}</small></span>
      </button>)}</div>
      <button type="button" className="button provider-add" disabled={busy || locked}
        onClick={() => {setFilter(''); dialog.current?.showModal();}}><Plus size={15} aria-hidden="true"/>Adicionar provedor</button>
    </aside>
    <div className="provider-detail">
      {active && <header className="provider-identity">
        <img src={providers[active].logo} width="42" height="42" alt=""/>
        <div><h3 tabIndex={-1} ref={heading}>{providers[active].name}</h3>
          <span className="provider-state" data-present={keys[active] === 'saved'}>{native ? keyLabel(keys[active]) : 'Prévia do navegador'}</span></div>
      </header>}
      <ConnectionDetail key={draft.provider} {...props} onKey={markKey} onLock={setLocked}/>
    </div>
    <dialog className="provider-dialog" aria-labelledby="provider-dialog-title" ref={dialog}
      onClick={event => {if(event.target === event.currentTarget) event.currentTarget.close();}}>
      <div className="provider-dialog-content"><header><h3 id="provider-dialog-title">Adicionar provedor</h3>
        <button type="button" aria-label="Fechar" className="button" onClick={() => dialog.current?.close()}><X size={16}/></button></header>
        <label className="provider-search"><Search size={16} aria-hidden="true"/><input aria-label="Pesquisar provedores" type="search" autoFocus value={filter}
          placeholder="Pesquisar provedores" onKeyDown={event => {if(event.key === 'Enter') event.preventDefault();}}
          onChange={e => setFilter(e.target.value)}/></label>
        <div className="provider-options">{choices.map(id => <button type="button" className="provider-row" key={id} onClick={() => choose(id)}>
          <img src={providers[id].logo} alt="" width="27" height="27"/><span><strong>{providers[id].name}</strong><small>{new URL(providers[id].endpoint).hostname}</small></span>
          <small>{keys[id] === 'saved' ? 'Cadastrado' : 'API'}</small></button>)}
          {!native && 'provedor compatível'.includes(filter.trim().toLocaleLowerCase('pt-BR')) && <button type="button" className="provider-row" onClick={() => {
            dialog.current?.close(); change('provider', 'compatible'); change('endpoint', saved.provider === 'compatible' ? saved.endpoint : ''); change('model',saved.provider === 'compatible' ? saved.model : '');
          }}>Provedor compatível</button>}
          {!choices.length && (native || !'provedor compatível'.includes(filter.trim().toLocaleLowerCase('pt-BR'))) && <p role="status">Nenhum provedor corresponde à pesquisa.</p>}</div>
      </div>
    </dialog>
  </div>;
}

function ConnectionDetail({draft, dirty, change, action, busy, onKey, onLock}: Props & {
  onKey?: (id:ProviderId, present:boolean)=>void; onLock?: (locked:boolean)=>void;
}) {
  const [status, setStatus] = useState<ProviderStatus>();
  const [credential, setCredential] = useState('');
  const [catalogVersion, setCatalogVersion] = useState(0);
  const [message, setMessage] = useState('');
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const inFlight = useRef(false);
  const statusRequest = useRef(0);
  const provider = knownProvider(draft.provider) ? draft.provider : null;
  const budget = useBudgetReview('test', {}, undefined, JSON.stringify([draft.provider, draft.model, draft.endpoint, draft.hasApiKey, catalogVersion]));

  async function reload() {
    if (!native || !provider) return;
    const ticket = ++statusRequest.current;
    const result = await request<ProviderStatus>('ai.status', {provider});
    if (ticket !== statusRequest.current) return;
    if (result.provider && result.provider !== provider) throw new Error('A credencial consultada pertence a outro provedor.');
    setStatus(result);
    onKey?.(provider, result.hasApiKey);
  }
  useEffect(() => {
    void reload().catch(cause => setError(errorMessage(cause)));
    return () => {statusRequest.current++;};
  }, [provider, draft.model]);

  function chooseProvider(id: ProviderId) {
    change('provider', id);
    change('endpoint', providers[id].endpoint);
    change('model', '');
  }
  async function perform(name: 'ai.key.save'|'ai.key.remove'|'ai.test') {
    if (!provider || inFlight.current) return;
    inFlight.current = true; setPending(true); onLock?.(true); setError(''); setMessage('');
    try {
      if (name === 'ai.test') {
        const authorization = budget.authorization();
        budget.invalidate();
        await action(name, authorization);
        setMessage('Teste de conexão concluído.');
      } else {
        const payload = name === 'ai.key.save' ? {provider, key: credential} : {provider};
        setCredential('');
        await action(name, payload);
        setMessage(name === 'ai.key.save' ? 'Credencial salva no armazenamento protegido.' : 'Credencial removida.');
      }
      if (name !== 'ai.test') {
        setStatus(current => ({
          ...current, hasApiKey: name === 'ai.key.save', verified: false, model: draft.model, endpoint: providers[provider].endpoint,
        }));
        setCatalogVersion(v => v + 1);
      }
      await reload();
    } catch (cause) {setError(errorMessage(cause));}
    finally {
      inFlight.current = false; setPending(false); onLock?.(false);
      if (name === 'ai.test') void budget.refresh();
    }
  }
  const disabled = busy || pending;
  const verified = !dirty && status?.verified && status.endpoint === draft.endpoint && status.model === draft.model;
  return <section className="content-section"><h2>Conexão</h2><div className="form-group">
    {!native && !provider &&
      <Field label="Provedor"><select value={draft.provider} disabled={disabled} onChange={e => {
        if (knownProvider(e.target.value)) chooseProvider(e.target.value);
      }}><option value={draft.provider}>Provedor compatível</option>{Object.entries(providers).map(([id,item]) => <option value={id} key={id}>{item.name}</option>)}</select></Field>}
    <Field label="Endpoint" hint={provider ? 'Destino das chamadas a este provedor.' : 'Endereço HTTPS do serviço.'}>
      <input type="url" value={provider ? providers[provider].endpoint : draft.endpoint} readOnly={!!provider || native}
        onChange={e => change('endpoint', e.target.value)} maxLength={2048} placeholder="https://api.exemplo.local/v1"/>
    </Field>
    <Field label="Chave de API" hint={!native ? 'O armazenamento de credenciais está disponível no aplicativo desktop.' : status?.hasApiKey ? 'Chave cadastrada. Você pode removê-la ou substituí-la.' : 'Ao salvar a chave, o aplicativo consulta os modelos disponíveis.'}>
      <input type="password" value={credential} disabled={!native || disabled || !provider} autoComplete="new-password" spellCheck={false}
        onChange={e => setCredential(e.target.value)} placeholder={status?.hasApiKey ? 'Nova chave para substituir a credencial' : 'Chave de API'} maxLength={512}/>
      <div className="inline-actions"><button className="button" type="button" disabled={!native || disabled || !provider || !credential.trim()} onClick={() => void perform('ai.key.save')}>Salvar credencial</button>
        {status?.hasApiKey && <button className="button" type="button" disabled={disabled} onClick={() => void perform('ai.key.remove')}>Remover credencial</button>}</div>
    </Field>
    {provider ? <ModelCatalogField expanded key={catalogVersion} provider={provider} value={draft.model} disabled={disabled} hasKey={!!status?.hasApiKey} onChange={model => change('model',model)}/> :
      <Field label="Modelo"><input value={draft.model} maxLength={160} disabled={native} onChange={e => change('model',e.target.value)}/></Field>}
  </div><div className="inline-actions">
    <button className="button" type="button" disabled={!native || disabled || dirty || !status?.hasApiKey || !draft.model || budget.pending || !budget.ready} onClick={() => void perform('ai.test')}>Testar conexão</button>
    <span className="muted">{verified ? 'Conexão verificada' + (status?.checkedAt ? ' · ' + shortDate(status.checkedAt) : '') : 'Conexão ainda não testada'}</span>
  </div><AiBudgetReview budget={budget} disabled={disabled || dirty || !status?.hasApiKey}/>
    <p className="muted spaced">O teste envia uma mensagem curta ao modelo salvo e pode consumir créditos. Os dados dos projetos ficam no dispositivo durante o teste.</p>
    {message && <p role="status" className="saved-message">{message}</p>}
    {error && <div role="alert"><Notice tone="warning">{error}</Notice></div>}
  </section>;
}
