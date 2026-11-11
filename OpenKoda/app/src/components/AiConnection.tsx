import { useEffect, useRef, useState } from 'react';
import { native, request } from '../api';
import { knownProvider, providers, type ProviderId, type ProviderStatus } from '../ai-provider';
import { Action, Settings, errorMessage, shortDate } from '../types';
import { Field, Notice } from './shared';
import AiBudgetReview, { useBudgetReview } from './AiBudgetReview';
import { ModelCatalogField, ProviderField } from './ProviderModels';

type Props = {draft: Settings; dirty: boolean; change: (key: 'provider'|'endpoint'|'model', value: string) => void; action: Action; busy: boolean};

export default function AiConnection({draft, dirty, change, action, busy}: Props) {
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
    inFlight.current = true; setPending(true); setError(''); setMessage('');
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
      inFlight.current = false; setPending(false);
      if (name === 'ai.test') void budget.refresh();
    }
  }
  const disabled = busy || pending;
  const verified = !dirty && status?.verified && status.endpoint === draft.endpoint && status.model === draft.model;
  return <section className="content-section"><h2>Conexão</h2><div className="form-group">
    {provider ? <ProviderField value={provider} disabled={disabled} onChange={chooseProvider} onCompatible={native ? undefined : () => {
      change('provider','compatible'); change('endpoint',''); change('model','');
    }}/> :
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
    {provider ? <ModelCatalogField key={catalogVersion} provider={provider} value={draft.model} disabled={disabled} hasKey={!!status?.hasApiKey} onChange={model => change('model',model)}/> :
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
