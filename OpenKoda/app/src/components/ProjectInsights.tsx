import { useEffect, useState, type ReactNode } from 'react';
import { ArrowDown, ArrowUp, ArrowUpDown, ChartNoAxesColumn, ChevronLeft, ChevronRight, FolderOpen, MessageSquare, Play, Shield } from 'lucide-react';
import { native, request } from '../api';
import { groupUsage, modelKey, providerName, type UsageRow } from '../usage-history';
import { projectScanReceipts, scanStateLabel } from '../security-scan';
import { errorMessage, findingLabels, severityLabels, shortDate, statusLabels, type Finding, type Project, type Snapshot } from '../types';
import CopyTargetButton from './CopyTargetButton';
import { SearchBox, Status } from './shared';

type View = 'conversations' | 'executions' | 'findings' | 'usage';
type Cell = { text: string; order?: string | number | null; content?: ReactNode };
type Row = { id: string; cells: Cell[]; status?: Finding['status']; severity?: Finding['severity'] };
type Sort = { column: number; descending: boolean };
const pageSize = 10;
const number = new Intl.NumberFormat('pt-BR');
const collator = new Intl.Collator('pt-BR', { numeric: true, sensitivity: 'base' });
const severityOrder = { low: 1, medium: 2, high: 3, critical: 4 };
const searchText = (text: string) => text.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase('pt-BR');

function compareCells(left: Cell, right: Cell, descending: boolean) {
  const a = left.order === undefined ? left.text : left.order;
  const b = right.order === undefined ? right.text : right.order;
  if (a === null || b === null) return a === b ? 0 : a === null ? 1 : -1;
  const difference = typeof a === 'number' && typeof b === 'number' ? a - b : collator.compare(String(a), String(b));
  return descending ? -difference : difference;
}

function RecordTable({ title, columns, rows, initialSort, findings, empty }: {
  title: string; columns: string[]; rows: Row[]; initialSort: Sort; findings?: boolean; empty: string;
}) {
  const [query, setQuery] = useState('');
  const [status, setStatus] = useState('');
  const [severity, setSeverity] = useState('');
  const [sort, setSort] = useState(initialSort);
  const [page, setPage] = useState(0);
  const needle = searchText(query.trim());
  const filtered = rows.filter(row => (!status || row.status === status) && (!severity || row.severity === severity)
    && (!needle || row.cells.some(cell => searchText(cell.text).includes(needle))));
  filtered.sort((a, b) => compareCells(a.cells[sort.column], b.cells[sort.column], sort.descending));
  const lastPage = Math.max(0, Math.ceil(filtered.length / pageSize) - 1);
  const currentPage = Math.min(lastPage, page);
  const first = currentPage * pageSize;
  const visible = filtered.slice(first, first + pageSize);

  function orderBy(column: number) {
    setSort({ column, descending: column === sort.column && !sort.descending });
    setPage(0);
  }

  return <section className="project-records" aria-label={title}>
    <header><h2>{title}</h2><span className="record-count" aria-live="polite">{number.format(filtered.length)} {filtered.length === 1 ? 'registro' : 'registros'}</span></header>
    {rows.length > 0 && <div className="project-record-filters">
      <SearchBox label={'Buscar em ' + title.toLocaleLowerCase('pt-BR')} value={query} onChange={value => { setQuery(value); setPage(0); }}/>
      {findings && <>
        <select aria-label="Estado do achado" value={status} onChange={event => { setStatus(event.target.value); setPage(0); }}>
          <option value="">Todos os estados</option>{Object.entries(findingLabels).map(([key, label]) => <option key={key} value={key}>{label}</option>)}
        </select>
        <select aria-label="Severidade do achado" value={severity} onChange={event => { setSeverity(event.target.value); setPage(0); }}>
          <option value="">Todas as severidades</option>{Object.entries(severityLabels).map(([key, label]) => <option key={key} value={key}>{label}</option>)}
        </select>
      </>}
    </div>}
    {visible.length ? <div className="project-table-scroll"><table aria-label={title} className="project-record-table">
      <thead><tr>{columns.map((label, index) => {
        const active = sort.column === index;
        const Icon = active ? sort.descending ? ArrowDown : ArrowUp : ArrowUpDown;
        return <th key={label} scope="col" aria-sort={active ? sort.descending ? 'descending' : 'ascending' : undefined}>
          <button type="button" aria-label={'Ordenar por ' + label} onClick={() => orderBy(index)}>{label}<Icon size={12} aria-hidden="true"/></button>
        </th>;
      })}</tr></thead>
      <tbody>{visible.map(row => <tr key={row.id}>{row.cells.map((cell, index) => index === 0
        ? <th key={index} scope="row">{cell.content ?? cell.text}</th>
        : <td key={index}>{cell.content ?? cell.text}</td>)}</tr>)}</tbody>
    </table></div> : <p className="record-empty" role="status">{rows.length ? 'Nenhum registro corresponde aos filtros.' : empty}</p>}
    {lastPage > 0 && <div className="record-pagination"><small>{first + 1}–{first + visible.length} de {filtered.length}</small>
      <button type="button" className="icon-button" aria-label="Página anterior" disabled={currentPage === 0} onClick={() => setPage(currentPage - 1)}><ChevronLeft size={16}/></button>
      <button type="button" className="icon-button" aria-label="Próxima página" disabled={currentPage === lastPage} onClick={() => setPage(currentPage + 1)}><ChevronRight size={16}/></button>
    </div>}
  </section>;
}

export default function ProjectInsights({ snapshot, project, selected, onConversation, onFinding }: {
  snapshot: Snapshot; project: Project; selected?: string; onConversation: (id: string) => void; onFinding: (id: string) => void;
}) {
  const [view, setView] = useState<View>('conversations');
  const [incompleteUsage, setIncompleteUsage] = useState(false);
  const [usage, setUsage] = useState<UsageRow[]>();
  const [error, setError] = useState('');
  const [refresh, setRefresh] = useState(0);
  const chats = new Map(snapshot.assessments.filter(a => a.projectId === project.id).map(a => [a.id, a]));
  const executions = (snapshot.executions ?? []).filter(run => chats.has(run.assessmentId));
  const scans = projectScanReceipts(chats.values());
  const withResults = scans.filter(scan => scan.findingsTotal !== undefined);
  const plans = new Map((snapshot.plans ?? []).filter(plan => chats.has(plan.assessmentId)).map(plan => [plan.id, plan]));
  const findings = snapshot.findings.filter(finding => chats.has(finding.assessmentId));
  const projectUsage = (usage ?? []).filter(row => row.assessmentId !== null && chats.has(row.assessmentId));

  useEffect(() => {
    if (!native) return;
    let current = true;
    setError('');
    void request<{ rows: UsageRow[]; scanHistoryUnavailable?: boolean }>('ai.budget.history').then(result => {
      if (!Array.isArray(result?.rows)) throw new Error('Histórico de uso indisponível.');
      if (current) { setUsage(result.rows); setIncompleteUsage(result.scanHistoryUnavailable === true); }
    }).catch(cause => {
      if (current) { setUsage(undefined); setError(errorMessage(cause)); }
    });
    return () => { current = false; };
  }, [snapshot, refresh]);

  function conversationCell(id: string): Cell {
    const title = chats.get(id)!.title;
    return { text: title, content: <button type="button" className="project-record-link" aria-current={selected === id ? 'true' : undefined} onClick={() => onConversation(id)}>{title}</button> };
  }

  const categories = [
    { key: 'conversations', title: 'Conversas', icon: MessageSquare, count: chats.size },
    { key: 'executions', title: 'Execuções', icon: Play, count: executions.length+scans.length },
    { key: 'findings', title: 'Achados', icon: Shield, count: findings.length },
    { key: 'usage', title: 'Uso de IA', icon: ChartNoAxesColumn, count: usage ? projectUsage.reduce((sum, row) => sum + row.requests, 0) : null },
  ] as const;

  let table: ReactNode;
  if (view === 'conversations') {
    const latest = new Map(scans.map(scan=>[scan.assessmentId,scan]));
    const rows: Row[] = [...chats.values()].map(chat => {
      const scan=latest.get(chat.id);
      const state=scan?scanStateLabel[scan.status]:statusLabels[chat.status];
      return {id:chat.id,cells:[conversationCell(chat.id),
        {text:state,content:scan?<span title="Estado da análise do OpenKoda">{state}</span>:<Status value={chat.status}/>},
        {text:shortDate(chat.updatedAt),order:Date.parse(chat.updatedAt)}]};
    });
    table = <RecordTable key={view} title="Conversas" columns={['Conversa', 'Estado', 'Atualizada em']} rows={rows} initialSort={{ column: 2, descending: true }} empty="Nenhuma conversa neste projeto."/>;
  } else if (view === 'executions') {
    const rows: Row[] = executions.map(run => ({ id: run.id, cells: [
      { text: plans.get(run.planId)?.adapter ?? 'Operação indisponível' }, conversationCell(run.assessmentId),
      { text: statusLabels[run.status as keyof typeof statusLabels] ?? run.status },
      { text: shortDate(run.startedAt), order: Date.parse(run.startedAt) },
    ] }));
    rows.push(...scans.map(scan=>({id:'security-'+scan.id,cells:[{text:'Análise do OpenKoda'},conversationCell(scan.assessmentId),
      {text:scanStateLabel[scan.status]},{text:shortDate(scan.createdAt),order:Date.parse(scan.createdAt)}]})));
    table = <RecordTable key={view} title="Execuções" columns={['Operação', 'Conversa', 'Estado', 'Iniciada em']} rows={rows} initialSort={{ column: 3, descending: true }} empty="Nenhuma execução registrada neste projeto."/>;
  } else if (view === 'findings') {
    const rows: Row[] = findings.map(finding => ({ id: finding.id, status: finding.status, severity: finding.severity, cells: [
      { text: finding.title, content: <button type="button" className="project-record-link" onClick={() => onFinding(finding.id)}>{finding.title}</button> },
      { text: severityLabels[finding.severity], order: severityOrder[finding.severity], content: <span className={'record-severity level-' + finding.severity}>{severityLabels[finding.severity]}</span> },
      { text: findingLabels[finding.status] }, conversationCell(finding.assessmentId),
    ] }));
    table = <><RecordTable key={view} title={withResults.length?'Achados registrados':'Achados'} columns={['Achado', 'Severidade', 'Estado', 'Conversa']} rows={rows} initialSort={{ column: 1, descending: true }} findings empty="Nenhum achado registrado neste projeto."/>
      {withResults.length>0&&<RecordTable title="Resultados da análise" columns={['Conversa','Achados relatados','Análise']}
        rows={withResults.map(scan=>({id:scan.id,cells:[conversationCell(scan.assessmentId),{text:number.format(scan.findingsTotal!),order:scan.findingsTotal},{text:scanStateLabel[scan.status]}]}))}
        initialSort={{column:1,descending:true}} empty="Nenhum resultado do motor neste projeto."/>}</>;
  } else if (error) {
    table = <div className="record-empty"><p role="alert">{error}</p><button className="button" type="button" onClick={() => setRefresh(value => value + 1)}>Tentar novamente</button></div>;
  } else if (!native || !usage) {
    table = <p className="record-empty" role="status">{native ? 'Consultando o histórico de uso…' : 'O histórico de uso está disponível no aplicativo desktop.'}</p>;
  } else {
    const records = new Map(projectUsage.map(row => [modelKey(row), row]));
    const rows: Row[] = [...groupUsage(projectUsage, modelKey)].map(([id, total]) => {
      const row = records.get(id)!;
      const tokens = total.measured ? number.format(total.volume) + (total.partial ? ' (parcial)' : '') : 'Não informado';
      return { id, cells: [{ text: row.model }, { text: providerName(row.provider) },
        { text: number.format(total.requests), order: total.requests }, { text: tokens, order: total.measured ? total.volume : null }] };
    });
    table = <RecordTable key={view} title="Uso de IA" columns={['Modelo', 'Provedor', 'Chamadas', 'Tokens medidos']} rows={rows} initialSort={{ column: 2, descending: true }} empty="Nenhuma chamada registrada neste projeto."/>;
  }

  return <div className="project-insights">
    {project.description && <p className="project-description">{project.description}</p>}
    <section className="project-primary-target" aria-label="Alvo cadastrado"><FolderOpen size={17} aria-hidden="true"/><div><small>Alvo principal</small><strong>{project.target}</strong></div><CopyTargetButton target={project.target}/></section>
    <div className="project-metrics" role="group" aria-label="Registros do projeto">
      {categories.map(({ key, title, icon: Icon, count }) => <button key={key} type="button" aria-pressed={key === view} onClick={() => setView(key)}>
        <span><Icon size={15} aria-hidden="true"/>{title}</span><strong>{count === null ? '—' : number.format(count)}</strong>{key === 'usage' && count !== null && <small>{count === 1 ? 'chamada' : 'chamadas'}</small>}
      </button>)}
    </div>
    {view === 'usage' && incompleteUsage && <p role="status">O consumo das análises de código está temporariamente indisponível.</p>}
    {table}
  </div>;
}
