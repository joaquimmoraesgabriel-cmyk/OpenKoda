import { useEffect, useState } from 'react';
import { native, request } from '../api';
import { Action, Settings, errorMessage, shortDate } from '../types';
import { Field, Notice } from './shared';
import AiBudgetReview, { useBudgetReview } from './AiBudgetReview';

interface ProviderStatus {hasApiKey:boolean;verified:boolean;checkedAt?:string;model:string;endpoint:string}
export default function AiConnection({draft,dirty,change,action,busy}:{draft:Settings;dirty:boolean;change:<K extends keyof Settings>(key:K,value:Settings[K])=>void;action:Action;busy:boolean}) {
  const [status,setStatus]=useState<ProviderStatus>();
  const [key,setKey]=useState(''); const [models,setModels]=useState<string[]>([]);
  const [message,setMessage]=useState(''); const [pending,setPending]=useState(false);
  const [error,setError]=useState('');
  const budget=useBudgetReview('test',{},undefined,JSON.stringify([draft.model,draft.endpoint,draft.hasApiKey]));
  async function reload(){if(native)setStatus(await request<ProviderStatus>('ai.status'));}
  useEffect(()=>{void reload().catch(e=>setError(errorMessage(e)));},[draft.endpoint,draft.model]);
  async function perform(name:string,payload:Record<string,unknown>={}) {
    setPending(true);setError('');setMessage('');
    try {
      const authorization=name==='ai.test'?budget.authorization():{};if(name==='ai.test')budget.invalidate();
      const result=await action<Record<string,unknown>>(name,{...payload,...authorization});
      await reload();
      if(name==='ai.models')setModels(result.models as string[]);
      setMessage(name==='ai.key.save'?'Credencial salva no armazenamento protegido.':name==='ai.key.remove'?'Credencial removida.':name==='ai.models'?'Lista de modelos consultada.':'Teste de conexão concluído.');
    }catch(e){setError(errorMessage(e));}
    finally {setPending(false);if(name==='ai.key.save')setKey('');if(name==='ai.test')void budget.refresh();}
  }
  const disabled=busy||pending;
  const verified=!dirty&&status?.verified&&status.endpoint===draft.endpoint&&status.model===draft.model;
  return <section className="content-section"><h2>Conexão</h2><div className="form-group">
    <Field label="Provedor"><select value={draft.provider} onChange={e=>change('provider',e.target.value)}><option value="compatible" disabled={native}>Provedor compatível{native?' (ainda não disponível)':''}</option><option value="openai">OpenAI</option></select></Field>
    <Field label="Endpoint" hint="Endereço HTTPS do serviço. Salvar não inicia uma conexão."><input type="url" value={draft.endpoint} onChange={e=>change('endpoint',e.target.value)} maxLength={2048} placeholder="https://api.exemplo.local/v1"/></Field>
    <Field label="Chave de API" hint={native?(status?.hasApiKey?'Uma credencial está cadastrada. O valor não é recuperado pela interface.':'Cadastre uma credencial após salvar a conexão.'):'Credenciais só podem ser cadastradas no aplicativo desktop.'}><input type="password" value={key} disabled={!native||disabled||dirty} autoComplete="new-password" spellCheck={false} onChange={e=>setKey(e.target.value)} placeholder={status?.hasApiKey?'Nova chave para substituir a credencial':'Chave de API'} maxLength={512}/><div className="inline-actions"><button className="button" type="button" disabled={!native||disabled||dirty||!key.trim()} onClick={()=>void perform('ai.key.save',{key})}>Salvar credencial</button>{status?.hasApiKey&&<button className="button" type="button" disabled={disabled||dirty} onClick={()=>void perform('ai.key.remove')}>Remover credencial</button>}</div></Field>
    <Field label="Modelo"><input value={draft.model} onChange={e=>change('model',e.target.value)} maxLength={160} placeholder="Identificador do modelo" list="provider-models"/><datalist id="provider-models">{models.map(model=><option value={model} key={model}/>)}</datalist></Field>
  </div><div className="inline-actions"><button className="button" type="button" disabled={!native||disabled||dirty||!status?.hasApiKey} onClick={()=>void perform('ai.models')}>Consultar modelos</button><button className="button" type="button" disabled={!native||disabled||dirty||!status?.hasApiKey||!draft.model||budget.pending||!budget.ready} onClick={()=>void perform('ai.test')}>{pending?'Conectando…':'Testar conexão'}</button><span className="muted">{verified?`Conexão verificada${status.checkedAt?' · '+shortDate(status.checkedAt):''}`:'Conexão ainda não testada'}</span></div>
  <AiBudgetReview budget={budget} disabled={disabled||dirty||!status?.hasApiKey}/>{dirty&&<p className="muted spaced">Salve as alterações antes de cadastrar a chave, consultar modelos ou testar a conexão.</p>}
  <p className="muted spaced">A integração atual usa a OpenAI em https://api.openai.com/v1. O teste faz uma chamada explícita ao provedor e pode consumir créditos. Nenhum arquivo ou evidência de projeto é enviado no teste.</p>
  {message&&<p role="status" className="saved-message">{message}</p>}{error&&<Notice tone="warning">{error}</Notice>}
  </section>;
}
