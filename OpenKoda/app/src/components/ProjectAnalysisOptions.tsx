import { useEffect, useState } from 'react';
import { native } from '../api';
import { analysisContextLabels, defaultProjectOptions, loadProjectOptions, readProjectOptions, type ProjectAnalysisOptions as Options } from '../project-analysis-options';
import { errorMessage, type Action } from '../types';
import { Field } from './shared';
import './ProjectLaunch.css';

export default function ProjectAnalysisOptions({projectId,action,busy}:{projectId:string;action:Action;busy:boolean}) {
  const [saved,setSaved]=useState<Options>(),[draft,setDraft]=useState(defaultProjectOptions);
  const [error,setError]=useState(''),[loading,setLoading]=useState(native),[revision,refresh]=useState(0),[saving,setSaving]=useState(false),[message,setMessage]=useState('');
  useEffect(()=>{
    if(!native)return;
    let active=true;setLoading(true);setError('');
    void loadProjectOptions(projectId).then(options=>{if(active){setSaved(options);setDraft(options);}}).catch(cause=>{if(active)setError(errorMessage(cause));}).finally(()=>{if(active)setLoading(false);});
    return()=>{active=false;};
  },[projectId,revision]);
  function change(update:Partial<Options>){setDraft(value=>({...value,...update}));setMessage('');}
  const disabled=busy||loading||saving||!native||!saved;
  const changed=!!saved&&JSON.stringify(draft)!==JSON.stringify(saved);
  async function save(){
    if(disabled||!changed)return;
    setSaving(true);setError('');setMessage('');
    try{const options=readProjectOptions(await action('project.options.save',{projectId,options:draft}));setSaved(options);setDraft(options);setMessage('Opções salvas para as próximas análises e conversas.');window.dispatchEvent(new CustomEvent('openkoda:project-options-updated',{detail:projectId}));}
    catch(cause){setError(errorMessage(cause));}finally{setSaving(false);}
  }
  return <section className="content-section project-analysis-options" aria-label="Opções de análise do projeto">
    <h2>Opções de análise</h2><p className="muted">Defina o foco e as preferências para as próximas conversas. Análises iniciadas e autorizações anteriores mantêm seu escopo original.</p>
    {!native?<p>Edite estas opções no aplicativo desktop.</p>:loading?<p role="status">Carregando opções…</p>:null}
    <form onSubmit={event=>{event.preventDefault();void save();}}>
      <fieldset disabled={disabled}>
        <section className="launch-disclosure"><label><strong>Contexto adicional</strong><input type="checkbox" role="switch" checked={draft.additionalContextEnabled} onChange={event=>change({additionalContextEnabled:event.target.checked})}/></label>
          {draft.additionalContextEnabled&&<div>{analysisContextLabels.map((label,index)=><Field key={label} label={label}><textarea maxLength={2000} rows={2} value={draft.context[index]} onChange={event=>change({context:draft.context.map((value,i)=>i===index?event.target.value:value) as Options['context']})}/></Field>)}</div>}
        </section>
        <section className="launch-disclosure"><label><strong>Opções avançadas</strong><input type="checkbox" role="switch" checked={draft.advancedEnabled} onChange={event=>change({advancedEnabled:event.target.checked})}/></label>
          {draft.advancedEnabled&&<div>
            <Field label="Título das próximas conversas" hint="Em branco, cada conversa recebe um título automático."><input maxLength={160} value={draft.title} onChange={event=>change({title:event.target.value})}/></Field>
            <Field label="Limite para iniciar operações (minutos)" hint="Aplica um prazo às próximas conversas. Em branco, o escopo não tem prazo de término."><input type="number" min={1} max={1440} value={draft.minutes??''} onChange={event=>change({minutes:event.target.value===''?null:event.target.valueAsNumber})}/></Field>
            <label className="analysis-limit-toggle"><input type="checkbox" checked={draft.maxCostUsd!==null} onChange={event=>change({maxCostUsd:event.target.checked?5:null})}/>Personalizar limite de custo</label>
            {draft.maxCostUsd!==null&&<Field label="Limite da análise em USD" hint="Uma chamada em andamento pode ultrapassar esse teto estimado."><input type="number" min="0.01" max="100" step="0.01" value={Number.isNaN(draft.maxCostUsd)?'':draft.maxCostUsd} required onChange={event=>change({maxCostUsd:event.target.valueAsNumber})}/></Field>}
            <small>Sem limite personalizado, o OpenKoda usa o limite padrão de US$ 5 por análise.</small>
          </div>}
        </section>
      </fieldset>
      {error&&<p role="alert">{error} {!saved&&<button type="button" className="text-button" onClick={()=>refresh(value=>value+1)}>Tentar novamente</button>}</p>}
      {message&&<p role="status">{message}</p>}
      <div className="form-actions"><button type="submit" className="button primary" disabled={disabled||!changed}>{saving?'Salvando…':'Salvar opções de análise'}</button>{changed&&<button type="button" className="button quiet" disabled={saving} onClick={()=>{setDraft(saved!);setError('');}}>Descartar alterações</button>}</div>
    </form>
  </section>;
}
