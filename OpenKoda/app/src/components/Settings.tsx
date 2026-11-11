import ProjectDataSettings from './ProjectDataSettings';
import { useEffect, useState } from 'react';
import { version } from '../../package.json';
import { Settings as SettingsIcon, Layers, ShieldCheck, Users, RefreshCw } from 'lucide-react';
import { Action, Project, Settings as SettingsModel } from '../types';
import { Field, Notice, KeyValue } from './shared';
import AiConnection from './AiConnection';
import AiLocalClosure from './AiLocalClosure';
import AiBudgetSettings from './AiBudgetSettings';
import Recovery from './Recovery';
import Retention from './Retention';
import ProjectPurge, { ProjectPurgeStatus } from './ProjectPurge';

const sections = [{name:'Geral',icon:SettingsIcon},{name:'IA e modelos',icon:Layers},{name:'Permissões',icon:Users},{name:'Dados e privacidade',icon:ShieldCheck},{name:'Atualizações',icon:RefreshCw}];
export default function Settings({settings,projects,action,busy,dataRequest=0,onPurgeStatus}:{settings:SettingsModel;projects:Project[];action:Action;busy:boolean;dataRequest?:number;onPurgeStatus:(value:ProjectPurgeStatus)=>void}) {
  const [section,setSection]=useState('IA e modelos');
  const [draft,setDraft]=useState(settings);
  const [saved,setSaved]=useState(false);
  const settingsRevision=JSON.stringify(settings);
  useEffect(()=>setDraft(settings),[settingsRevision]);
  useEffect(()=>{if(dataRequest)setSection('Dados e privacidade');},[dataRequest]);
  const dirty=JSON.stringify(draft)!==JSON.stringify(settings);
  function change<K extends keyof SettingsModel>(key:K,value:SettingsModel[K]) {setDraft(s=>({...s,[key]:value}));setSaved(false);}
  async function save(e:React.FormEvent) {e.preventDefault();const {hasApiKey,...payload}=draft;await action('settings.save',payload);setSaved(true);}
  return <div className="settings-layout"><aside className="settings-menu"><h1>Configurações</h1><nav aria-label="Categorias de configuração">{sections.map(({name,icon:Icon})=><button key={name} className={section===name?'active':''} aria-current={section===name?'page':undefined} onClick={()=>setSection(name)}><Icon size={21}/>{name}</button>)}</nav></aside><section className="settings-content" aria-label={section}><div className="detail-heading"><div><h1>{section}</h1><p>{section==='IA e modelos'?'Configure a conexão usada nas avaliações.':'Preferências da sua instalação OpenKoda.'}</p></div>{dirty&&<span className="badge">Alterações não salvas</span>}</div><form onSubmit={e=>void save(e).catch(()=>{})}>
    {section==='IA e modelos'&&<><AiConnection draft={draft} dirty={dirty} change={change} action={action} busy={busy}/><AiBudgetSettings action={action} busy={busy}/><AiLocalClosure action={action} busy={busy}/><section className="content-section"><h2>Uso e limites</h2><div className="form-group"><Field label="Modo padrão"><select value={draft.mode} onChange={e=>change('mode',e.target.value)}><option value="assisted">Assistido</option></select></Field><Field label="Tempo por operação (segundos)"><input type="number" min={10} max={3600} required value={draft.timeoutSeconds} onChange={e=>change('timeoutSeconds',e.target.valueAsNumber)}/></Field><Field label="Limite de saída (KiB)"><input type="number" min={16} max={4096} required value={draft.outputLimitKiB} onChange={e=>change('outputLimitKiB',e.target.valueAsNumber)}/></Field><KeyValue label="Dados enviados à IA"><button className="text-button" type="button" onClick={()=>setSection('Dados e privacidade')}>Revisar política</button></KeyValue></div></section><Notice>A troca de modelo vale para as próximas avaliações. Nenhum conteúdo é enviado ao salvar.</Notice></>}
    {section==='Geral'&&<section className="content-section"><h2>Interface</h2><div className="form-group"><Field label="Tema"><select value={draft.theme} onChange={e=>change('theme',e.target.value)}><option value="light">Claro</option><option value="dark">Escuro</option><option value="system">Usar tema do sistema</option></select></Field><Field label="Idioma"><select value={draft.language} onChange={e=>change('language',e.target.value)}><option value="pt-BR">Português (Brasil)</option></select></Field></div><p className="muted">A interface respeita a preferência do sistema para reduzir movimento. As datas seguem o fuso deste dispositivo.</p></section>}
    {section==='Permissões'&&<section className="content-section"><h2>Autoridade de execução</h2><KeyValue label="Modo disponível">Assistido</KeyValue><KeyValue label="Execução de ferramentas">Aguardando executor verificado</KeyValue><Notice>Cadastrar um alvo não autoriza operações. Cada avaliação deve ter escopo e efeitos revisados antes da execução.</Notice><p className="muted">Políticas avançadas, elevação e manutenção terão revisão própria quando seus executores estiverem disponíveis.</p></section>}
    {section==='Dados e privacidade'&&<section className="content-section"><h2>Armazenamento local</h2><p>Projetos, avaliações e evidências permanecem no armazenamento desta instalação. A prévia do navegador usa dados separados.</p><div className="form-group"><Field label="Retenção desejada (dias)" hint="Prazo de referência para revisão. Salvar não agenda nem exclui arquivos; a exclusão depende de seleção e confirmação abaixo."><input type="number" min={1} max={3650} value={draft.retentionDays} required onChange={e=>change('retentionDays',e.target.valueAsNumber)}/></Field></div><Notice>Cada mensagem exige a revisão do contexto e o consentimento de envio. A IA recebe somente o conteúdo selecionado nessa revisão; não executa ferramentas automaticamente.</Notice></section>}
    {section==='Dados e privacidade'&&<ProjectDataSettings projects={projects} action={action} busy={busy}/>}
    {section==='Dados e privacidade'&&<Recovery projects={projects} action={action} busy={busy}/>}
    {section==='Dados e privacidade'&&<Retention action={action} busy={busy}/>}
    {section==='Dados e privacidade'&&<ProjectPurge projects={projects} action={action} busy={busy} onStatus={onPurgeStatus}/>}
    {section==='Atualizações'&&<section className="content-section"><h2>Versão instalada</h2><KeyValue label="OpenKoda">{version} · Compilação local</KeyValue><KeyValue label="Canal de atualização">Local, sem assinatura · Instalação manual</KeyValue><Notice>A instalação manual da versão 0.1.0 anterior foi verificada. A versão exibida identifica esta compilação e não atesta a validação do instalador correspondente. Não há atualização automática de aplicativo ou Kali.</Notice></section>}
    {['Geral','IA e modelos','Dados e privacidade'].includes(section)&&<div className="form-actions start"><button className="button primary" disabled={busy} type="submit">{busy?'Salvando…':'Salvar alterações'}</button>{saved&&!dirty&&<span role="status" className="saved-message">Alterações salvas</span>}</div>}
  </form></section></div>;
}
