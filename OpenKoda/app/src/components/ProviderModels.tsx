import { useContext, useEffect, useId, useRef, useState } from 'react';
import { AssessmentModelContext } from '../assessment-model-context';
import { Check, LoaderCircle, RefreshCw } from 'lucide-react';
import { native, request } from '../api';
import { errorMessage, type Assessment, type Settings } from '../types';
import { knownProvider, providers, selectionLabel, type AiSelection, type ProviderId, type ProviderStatus } from '../ai-provider';
import { Field } from './shared';
import './provider-models.css';

export function ProviderField({value, onChange, disabled, label = 'Provedor', onCompatible, available}: {
  value: ProviderId; onChange: (provider: ProviderId) => void; disabled?: boolean; label?: string; onCompatible?: () => void;
  available?: ProviderId[];
}) {
  return <div className="provider-field">
    <Field label={label}>
      <select value={value} disabled={disabled} onChange={e => {
        if (knownProvider(e.target.value)) onChange(e.target.value);
        else if (e.target.value === 'compatible') onCompatible?.();
      }}>
        {available && !available.includes(value) && <option value={value} disabled>{providers[value].name} · indisponível</option>}
        {(available ?? Object.keys(providers) as ProviderId[]).map(id => <option key={id} value={id}>{providers[id].name}</option>)}
        {onCompatible && <option value="compatible">Provedor compatível</option>}
      </select>
      <img src={providers[value].logo} alt="" width="24" height="24"/>
    </Field>
  </div>;
}

type Catalog = {phase: 'idle' | 'loading' | 'no-key' | 'ready' | 'failed'; models: string[]; message?: string};
const idle: Catalog = {phase: 'idle', models: []};

export function ModelCatalogField({provider, value, onChange, disabled, label = 'Modelo', allowManual = false, expanded = false, hasKey, enabledModels}: {
  provider: ProviderId; value: string; onChange: (model: string) => void;
  disabled?: boolean; label?: string; allowManual?: boolean; expanded?: boolean; hasKey?: boolean;
  enabledModels?: string[] | null;
}) {
  const [attempt, setAttempt] = useState(0);
  const [catalog, setCatalog] = useState<Catalog>(idle);
  const [filter, setFilter] = useState('');
  const listId = useId();
  useEffect(() => {
    let active = true;
    setFilter('');
    if (!native || (allowManual && !attempt)) {setCatalog(idle); return;}
    if (hasKey === false) {setCatalog({phase: 'no-key', models: []}); return;}
    if (enabledModels) {setCatalog({phase: 'ready', models: enabledModels}); return;}
    setCatalog({phase: 'loading', models: []});
    async function load() {
      // Strict Mode can unmount the first effect before its request starts.
      await Promise.resolve();
      if (!active) return;
      let available = hasKey;
      if (available === undefined) {
        const status = await request<ProviderStatus>('ai.status', {provider});
        if (status.provider && status.provider !== provider) throw new Error('O provedor consultado mudou. Atualize o catálogo.');
        available = status.hasApiKey;
      }
      if (!active) return;
      if (!available) {setCatalog({phase: 'no-key', models: []}); return;}
      const result = await request<{models: string[]}>('ai.models', {provider});
      if (!active) return;
      if (!Array.isArray(result.models) || result.models.some(model => typeof model !== 'string' || !model.trim())) {
        throw new Error('O provedor retornou um catálogo de modelos inválido.');
      }
      setCatalog({phase: 'ready', models: [...new Set(result.models)].sort()});
    }
    void load().catch(error => {if (active) setCatalog({phase: 'failed', models: [], message: errorMessage(error)});});
    return () => {active = false;};
  }, [provider, hasKey, attempt, allowManual, enabledModels]);

  const hint = !native ? 'O catálogo da sua chave está disponível no aplicativo desktop.'
    : catalog.phase === 'no-key' ? 'Cadastre a chave deste provedor em IA e modelos.'
    : enabledModels ? enabledModels.length ? 'Modelos ativados em IA e modelos.' : 'Conclua a ativação deste provedor em IA e modelos.'
    : catalog.phase === 'ready' && !catalog.models.length ? 'O provedor não retornou modelos para esta chave.'
    : catalog.phase === 'ready' ? catalog.models.length.toLocaleString('pt-BR') + ' modelo(s) no catálogo.'
    : catalog.phase === 'loading' ? 'Consultando o catálogo…' : undefined;
  const search = filter.trim().toLowerCase();
  const matching = search ? catalog.models.filter(id => id.toLowerCase().includes(search)) : catalog.models;
  const CatalogIcon = catalog.phase === 'loading' ? LoaderCircle : catalog.phase === 'ready' ? Check : RefreshCw;
  return <div className="provider-model-field">
    {!allowManual && catalog.models.length > (expanded ? 1 : 20) && <Field label="Filtrar modelos">
      <input type="search" value={filter} disabled={disabled} placeholder="Digite parte do nome" onChange={e => setFilter(e.target.value)}/>
    </Field>}
    <Field label={label} hint={hint}>
      {allowManual ? <input value={value} disabled={disabled} maxLength={160} list={listId} onChange={e => onChange(e.target.value)}/>
        : <select value={value} size={expanded && catalog.models.length ? Math.min(6, Math.max(2, catalog.models.length+1)) : undefined}
          disabled={disabled || !catalog.models.length} onChange={e => onChange(e.target.value)}>
          <option value="">Selecione um modelo</option>
          {value && !matching.includes(value) && <option value={value} disabled={!catalog.models.includes(value)}>{value}{catalog.phase === 'ready' && !catalog.models.includes(value) ? enabledModels ? ' · desativado' : ' · fora do catálogo atual' : ''}</option>}
          {matching.map(id => <option value={id} key={id}>{id}</option>)}
        </select>}
      {allowManual && <datalist id={listId}>{catalog.models.map(id => <option value={id} key={id}/>)}</datalist>}
    </Field>
    {!allowManual && <p className="catalog-capabilities">
      As execuções do OpenKoda usam geração de texto. O catálogo também pode incluir modelos de outras modalidades.
      {provider === 'openrouter' && ' Alguns modelos do OpenRouter incluem serviços remotos próprios, como busca na web. O acesso às ferramentas locais depende do escopo e da aprovação da operação.'}
    </p>}
    {search && <small role="status">{matching.length ? matching.length + ' modelo(s) correspondente(s).' : 'Nenhum modelo corresponde ao filtro.'}</small>}
    {native && !enabledModels && <div className="model-catalog-tools">
      <button className="button catalog-refresh" type="button" aria-busy={catalog.phase === 'loading'} disabled={disabled || catalog.phase === 'loading' || hasKey === false} onClick={() => setAttempt(n => n + 1)}>
        <CatalogIcon size={15} aria-hidden="true"/>
        {catalog.phase === 'loading' ? 'Consultando modelos…' : catalog.phase === 'failed' ? 'Tentar consultar novamente' : 'Consultar modelos'}
      </button>
      {catalog.phase === 'failed' && <span role="alert">{catalog.message}</span>}
    </div>}
  </div>;
}

type AssessmentSelectionProps = {
  settings: Settings; value: AiSelection | null; onChange: (value: AiSelection | null) => void; disabled: boolean;
};
function ActiveProviderFields({value, onChange, disabled}: {value: AiSelection; onChange: (value: AiSelection) => void; disabled: boolean}) {
  const [keys, setKeys] = useState<Partial<Record<ProviderId, ProviderStatus>>>();
  const [partial, setPartial] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!native) return;
    let active = true;
    const ids = Object.keys(providers) as ProviderId[];
    async function read() {
      await Promise.resolve();
      if (!active) return;
      const statuses = await Promise.allSettled(ids.map(async id => {
        const status = await request<ProviderStatus>('ai.status', {provider:id});
        if (status.provider !== id && (status.provider || status.endpoint !== providers[id].endpoint)) {
          throw new Error('Provedor divergente.');
        }
        return [id, status] as const;
      }));
      if (!active) return;
      setKeys(Object.fromEntries(statuses.flatMap(result => result.status === 'fulfilled' ? [result.value] : [])));
      setPartial(statuses.some(result => result.status === 'rejected'));
    }
    void read();
    return () => {active = false;};
  }, [attempt]);
  if (!keys) return <p role="status">Consultando os provedores do perfil…</p>;
  const available = (Object.keys(keys) as ProviderId[]).filter(id => keys[id]?.hasApiKey && keys[id]?.enabledModels?.length !== 0);
  const status = keys[value.provider];
  return <>
    <ProviderField value={value.provider} available={available} disabled={disabled || !available.length} onChange={provider => {
      const enabled = keys[provider]?.enabledModels;
      onChange({provider, model: enabled?.length === 1 ? enabled[0] : ''});
    }}/>
    <ModelCatalogField key={value.provider} provider={value.provider} value={value.model} disabled={disabled}
      hasKey={!!status?.hasApiKey} enabledModels={status?.enabledModels} onChange={model => onChange({...value, model})}/>
    {partial && <div className="model-catalog-tools"><span role="status">Alguns provedores não puderam ser consultados.</span>
      <button className="button" type="button" onClick={() => setAttempt(n => n + 1)}>Recarregar provedores</button></div>}
  </>;
}

export function AssessmentModelFields({settings, value, onChange, disabled}: AssessmentSelectionProps) {
  return <div className="assessment-model-fields">
    <Field label="Modelo da avaliação">
      <select value={value ? 'custom' : 'default'} disabled={!native || disabled} onChange={e => {
        onChange(e.target.value === 'default' ? null : knownProvider(settings.provider) ? {provider: settings.provider, model: settings.model} : {provider: 'openai', model: ''});
      }}>
        <option value="default">Padrão da instalação · {selectionLabel(settings)}</option>
        <option value="custom">Escolher provedor e modelo</option>
      </select>
    </Field>
    {value && <ActiveProviderFields value={value} disabled={disabled} onChange={onChange}/>}
  </div>;
}

export function ExecutionModelPicker({assessment, disabled}: {assessment: Assessment; disabled: boolean}) {
  const context = useContext(AssessmentModelContext);
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState<AiSelection | null>(null);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const inFlight = useRef(false);
  if (!context) return null;
  const {settings, action} = context;
  const saved = assessment.aiSelection ?? null;
  const changed = value?.provider !== saved?.provider || value?.model !== saved?.model;
  async function save() {
    if (inFlight.current || !changed || (value && !value.model)) return;
    inFlight.current = true; setPending(true); setError('');
    try {
      await action('ai.selection.save', {assessmentId: assessment.id, aiSelection: value});
      setEditing(false);
    } catch (cause) {setError(errorMessage(cause));}
    finally {inFlight.current = false; setPending(false);}
  }
  return <section className="execution-model-picker" aria-label="Modelo desta avaliação">
    <div className="execution-model-summary">
      <div><strong>Modelo desta avaliação</strong><p>{selectionLabel(saved ?? settings)}</p></div>
      {!editing && <button type="button" className="button" disabled={disabled} onClick={() => {setValue(saved); setError(''); setEditing(true);}}>Alterar modelo</button>}
    </div>
    {editing && <>
      <AssessmentModelFields settings={settings} value={value} disabled={disabled || pending} onChange={setValue}/>
      <div className="inline-actions">
        <button className="button primary" type="button" disabled={disabled || pending || !changed || !!value && !value.model} onClick={() => void save()}>Aplicar nesta avaliação</button>
        <button className="button" type="button" disabled={pending} onClick={() => setEditing(false)}>Descartar escolha</button>
      </div>
      {error && <p role="alert">{error}</p>}
    </>}
  </section>;
}
