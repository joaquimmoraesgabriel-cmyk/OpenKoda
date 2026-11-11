import { useEffect, useRef, useState, type FormEvent } from 'react';
import { Boxes, CircleDollarSign, Database, HardDriveDownload, History, Layers, LifeBuoy, Monitor, RefreshCw, Search, Settings2, ShieldCheck, SlidersHorizontal } from 'lucide-react';
import { version } from '../../package.json';
import { native } from '../api';
import { appearanceFrom, appearanceKeys, saveAppearance, useDeviceAppearance, type DeviceAppearance } from '../appearance';
import { Action, Assessment, Project, Settings as SettingsModel, errorMessage } from '../types';
import { Field, Notice, KeyValue } from './shared';
import { AppearanceSettings, KeyboardTips } from './AppearanceSettings';
import AiConnection from './AiConnection';
import AiLocalClosure from './AiLocalClosure';
import AiBudgetSettings from './AiBudgetSettings';
import GeneralSettings from './GeneralSettings';
import WindowBehaviorSettings from './WindowBehaviorSettings';
import UsageHistory from './UsageHistory';
import ProjectDataSettings from './ProjectDataSettings';
import ProjectPurge, { ProjectPurgeStatus } from './ProjectPurge';
import Recovery from './Recovery';
import Retention from './Retention';
import SupportPackage from './SupportPackage';
import './settings.css';

const groups = [
  { name: 'Preferências', items: [
    { name: 'Geral', icon: Settings2 }, { name: 'Aparência', icon: Monitor },
  ]},
  { name: 'Área de trabalho', items: [
    { name: 'Dados e privacidade', icon: ShieldCheck }, { name: 'Armazenamento', icon: Database },
    { name: 'Backup e recuperação', icon: HardDriveDownload }, { name: 'Permissões', icon: SlidersHorizontal },
  ]},
  { name: 'Integrações', items: [
    { name: 'IA e modelos', icon: Layers }, { name: 'Uso e orçamento', icon: CircleDollarSign }, {name: 'Histórico de uso', icon: History},
  ]},
  { name: 'Instalação', items: [
    { name: 'Diagnóstico e suporte', icon: LifeBuoy }, { name: 'Atualizações', icon: RefreshCw },
  ]},
] as const;

type Section = typeof groups[number]['items'][number]['name'];
type Draft = SettingsModel & DeviceAppearance;
type EditableKey = Exclude<keyof Draft, 'hasApiKey'>;
const fields: Partial<Record<Section, EditableKey[]>> = {
  'Aparência': ['theme', ...appearanceKeys],
  'IA e modelos': ['provider', 'endpoint', 'model'],
  'Armazenamento': ['retentionDays'],
  'Permissões': ['timeoutSeconds', 'outputLimitKiB'],
};

type Props = {
  settings: SettingsModel; projects: Project[]; action: Action; busy: boolean;
  dataRequest?: number; usageRequest?: number; assessments?: Assessment[]; onOpenAssessment?: (id: string) => void; onPurgeStatus: (value: ProjectPurgeStatus) => void;
};

export default function Settings({settings, projects, action, busy, dataRequest=0, usageRequest=0, assessments, onOpenAssessment, onPurgeStatus}: Props) {
  const [section, setSection] = useState<Section>('Geral');
  const [search, setSearch] = useState('');
  const normalized = (value: string) => value.normalize('NFD').replace(/[\u0300-\u036f]/g,'').toLocaleLowerCase('pt-BR');
  const matches = groups.map(group => ({...group, items: group.items.filter(item =>
    normalized(group.name + ' ' + item.name).includes(normalized(search.trim())))})).filter(group => group.items.length);
  const [edits, setEdits] = useState<Partial<Draft>>({});
  const [saved, setSaved] = useState<Section>();
  const [saveError, setSaveError] = useState('');
  const panel = useRef<HTMLDivElement>(null);
  const committed = {...settings, ...useDeviceAppearance()};
  const draft = {...committed, ...edits};
  const activeFields = fields[section] ?? [];
  const changedKeys = (keys: EditableKey[]) => keys.filter(key => key in edits && draft[key] !== committed[key]);
  const dirty = changedKeys(activeFields).length > 0;

  function navigate(next: Section) {
    setSection(next);
    setSaved(undefined);
    setSaveError('');
    panel.current?.scrollTo({top: 0});
  }

  useEffect(() => {
    if (dataRequest) {setSearch(''); navigate('Dados e privacidade');}
  }, [dataRequest]);
  useEffect(() => { if (usageRequest) {setSearch(''); navigate('Histórico de uso');} }, [usageRequest]);

  function change<K extends keyof Draft>(key: K, value: Draft[K]) {
    setEdits(current => ({...current, [key]: value}));
    setSaved(undefined);
    setSaveError('');
  }

  function discard(keys: EditableKey[]) {
    setEdits(current => {
      const next = {...current};
      for (const key of keys) delete next[key];
      return next;
    });
    setSaved(undefined);
    setSaveError('');
  }

  async function save(event: FormEvent) {
    event.preventDefault();
    if (busy || !dirty) return;
    const keys = changedKeys(activeFields);
    const payload = Object.fromEntries(keys.map(key => [key, draft[key]]));
    try {
      if (section === 'Aparência' && keys.some(key => key !== 'theme')) {
        await saveAppearance(appearanceFrom(draft), keys.includes('theme') ? () => action('settings.save', {theme: draft.theme}) : undefined);
      } else {
        await action('settings.save', payload);
      }
      setEdits(current => {
        const next = {...current};
        for (const key of keys) if (next[key] === payload[key]) delete next[key];
        return next;
      });
      setSaved(section);
    } catch (error) {
      setSaveError(errorMessage(error));
    }
  }

  const saveActions = <div className="settings-actions">
    <button className="button primary" type="submit" disabled={busy || !dirty}>Salvar alterações</button>
    <button className="button settings-discard" type="button" disabled={busy || !dirty} onClick={() => discard(activeFields)}>Descartar</button>
    {dirty && <span className="muted">Alterações não salvas</span>}
    {saved === section && !dirty && <span role="status" className="saved-message">Alterações salvas</span>}
    {saveError && <span role="alert">{saveError}</span>}
  </div>;

  return <div className="settings-layout">
    <h1 className="settings-page-title">Configurações</h1>
    <aside className="settings-menu">
      <label className="settings-search"><Search size={15} aria-hidden="true"/><input type="search" aria-label="Pesquisar configurações" placeholder="Pesquisar configurações" value={search} onChange={e=>setSearch(e.target.value)}/></label>
      <nav aria-label="Categorias de configuração">
        {matches.length === 0 && <p role="status" className="muted">Nenhuma categoria encontrada.</p>}
        {matches.map(group => <div className="settings-group" key={group.name}>
          <h2>{group.name}</h2>
          {group.items.map(({name, icon: Icon}) => <button type="button" key={name} aria-label={name} aria-current={section === name ? 'page' : undefined} onClick={() => navigate(name)}>
            <Icon size={18} strokeWidth={1.65} aria-hidden="true"/><span>{name}</span>
            {changedKeys(fields[name] ?? []).length > 0 && <span className="setting-modified" aria-label="Alterações não salvas">•</span>}
          </button>)}
        </div>)}
      </nav>
    </aside>
    <div className="settings-content" ref={panel} role="region" aria-label={`Conteúdo de ${section}`} tabIndex={0}>
      <section className="settings-sheet" aria-label={section}>
        <h2 className="settings-section-title">{section}</h2>
        {section === 'Aparência' && <form onSubmit={save}>
          <AppearanceSettings draft={draft} change={change} busy={busy}/>{saveActions}
        </form>}
        {section === 'Geral' && <GeneralSettings windowOptions={<WindowBehaviorSettings/>}/>}
        {section === 'IA e modelos' && <>
          {native
            ? <AiConnection saved={settings} draft={draft} change={change} action={action} busy={busy}/>
            : <form onSubmit={save}><AiConnection saved={settings} draft={draft} change={change} action={action} busy={busy}/>{saveActions}</form>}
          <AiLocalClosure action={action} busy={busy}/>
        </>}
        {section === 'Uso e orçamento' && <AiBudgetSettings action={action} busy={busy} onUsageHistory={()=>navigate('Histórico de uso')}/>}
        {section === 'Histórico de uso' && <UsageHistory assessments={assessments} onOpenAssessment={onOpenAssessment}/>}
        {section === 'Permissões' && <>
          <p className="muted">Cada operação depende dos alvos, métodos e limites do escopo autorizado da avaliação.</p>
          <form onSubmit={save}><div className="form-group">
            <KeyValue label="Modo de execução">Assistido</KeyValue>
            <Field label="Tempo por operação (segundos)"><input type="number" min={10} max={3600} required value={Number.isNaN(draft.timeoutSeconds) ? '' : draft.timeoutSeconds} onChange={e => change('timeoutSeconds', e.target.valueAsNumber)}/></Field>
            <Field label="Limite de saída (KiB)"><input type="number" min={16} max={4096} required value={Number.isNaN(draft.outputLimitKiB) ? '' : draft.outputLimitKiB} onChange={e => change('outputLimitKiB', e.target.valueAsNumber)}/></Field>
          </div>{saveActions}</form>
        </>}
        {section === 'Dados e privacidade' && <>
          <Notice>A cada envio à IA, você revisa e autoriza o conteúdo selecionado. As políticas de cada projeto determinam os destinos permitidos.</Notice>
          <ProjectDataSettings projects={projects} action={action} busy={busy}/>
          <ProjectPurge projects={projects} action={action} busy={busy} onStatus={onPurgeStatus}/>
        </>}
        {section === 'Armazenamento' && <>
          <form onSubmit={save}><Field label="Retenção desejada (dias)" hint="Prazo de referência para revisão. A exclusão exige seleção e confirmação.">
            <input type="number" min={1} max={3650} value={Number.isNaN(draft.retentionDays) ? '' : draft.retentionDays} required onChange={e => change('retentionDays', e.target.valueAsNumber)}/>
          </Field>{saveActions}</form>
          <Retention action={action} busy={busy}/>
        </>}
        {section === 'Backup e recuperação' && <Recovery projects={projects} action={action} busy={busy}/>}
        {section === 'Diagnóstico e suporte' && <SupportPackage/>}
        {section === 'Atualizações' && <>
          <div className="installed-version"><Boxes size={25} aria-hidden="true"/><div><strong>OpenKoda</strong><p>Versão {version}</p></div></div>
          <KeyValue label="Canal">Distribuição local</KeyValue>
          <KeyValue label="Instalação de atualizações">Manual, pelo instalador do OpenKoda</KeyValue>
        </>}
      </section>
      {section === 'Aparência' && <KeyboardTips/>}
    </div>
  </div>;
}
