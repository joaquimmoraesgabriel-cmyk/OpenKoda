import { useState } from 'react';
import { request } from '../api';
import { Action, errorMessage, shortDate } from '../types';
import { KeyValue } from './shared';

type Registration = { name: string; guid: string; basePath: string };
type Backup = { id: string; format?: 'tar' | 'vhdx'; sourceId: string; source: Registration; maxBytes: number; spool: { terminalCaptures: number; sessionRecords: number } };
type Restore = { id: string; backup: Backup; bytes: number; sha256: string; firstWslDistribution: boolean };
type Receipt = {
  operation: { kind: 'export'; backup: Backup } | { kind: 'import'; restore: Restore };
  status: string; startedAt: string; bytes?: number; sha256?: string; error?: string;
};
type Plan = {
  id: string; digest: string | null; expiresAt: string;
  payload: { backup?: Backup; restore?: Restore; target: string; effects: string; distribution?: string };
};
const labels: Record<string, string> = {
  exported: 'Cópia verificada', inactive: 'Cópia restaurada e inativa',
  incomplete_copy: 'Exportação incompleta', stop_unknown: 'Parada da origem a conferir',
  copy_unknown: 'Estado da cópia restaurada a conferir', stopping_source: 'Encerrando a origem',
  exporting_source: 'Exportando arquivos do Kali', copying: 'Exportando a cópia',
  importing: 'Importando a cópia', personalizing: 'Conferindo a nova identidade',
  stopping_copy: 'Encerrando a cópia restaurada',
};

export default function KaliArchives({ action, busy }: { action: Action; busy: boolean }) {
  const [records, setRecords] = useState<Receipt[]>();
  const [plan, setPlan] = useState<Plan>();
  const [approved, setApproved] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const disabled = pending || busy;
  async function reload() {
    setRecords((await request<{ records: Receipt[] }>('environment.archive.list')).records);
  }
  async function run(operation: () => Promise<void>) {
    setPending(true); setError(''); setMessage('');
    try { await operation(); } catch (e) { setError(errorMessage(e)); }
    finally { setPending(false); }
  }
  async function prepare(id?: string) {
    setPlan(undefined); setApproved(false);
    const reviewed = id ? await action<Plan>('environment.archive.importPlan', { id }) :
      await action<Plan>('environment.archive.exportPlan');
    if (!reviewed.digest || !/^[a-f0-9]{64}$/.test(reviewed.digest)) {
      throw new Error('A origem, o destino ou outros detalhes da operação estão ocultos. Confira a credencial em Configurações e prepare uma nova revisão.');
    }
    const backup = reviewed.payload.restore?.backup ?? reviewed.payload.backup;
    if (!backup || (backup.format !== undefined && backup.format !== 'tar' && backup.format !== 'vhdx')) {
      throw new Error('Formato da cópia indisponível para revisão.');
    }
    setPlan(reviewed);
  }
  async function apply() {
    if (!plan?.digest || !approved) return;
    const reviewed = plan;
    setPlan(undefined); setApproved(false);
    try {
      const receipt = await action<Receipt>(reviewed.payload.restore ? 'environment.archive.import' : 'environment.archive.export',
        { planId: reviewed.id, digest: reviewed.digest });
      setMessage(labels[receipt.status]);
    } finally { await reload(); }
  }
  const backup = plan?.payload.backup ?? plan?.payload.restore?.backup;
  return <section className="content-section" aria-label="Cópias do Kali">
    <h3>Cópias do Kali</h3>
    <p>Exporte os arquivos da distribuição gerida para uma cópia TAR local ou restaure uma cópia verificada em uma distribuição separada.</p>
    <div className="inline-actions">
      <button type="button" className="button" disabled={disabled} onClick={() => void run(() => prepare())}>Revisar exportação do Kali</button>
      <button type="button" className="button" disabled={disabled} onClick={() => void run(reload)}>Consultar cópias do Kali</button>
    </div>
    {records && (records.length ? <ul>{records.map((record, index) => {
      const id = record?.operation?.kind === 'export' ? record.operation.backup?.id :
        record?.operation?.kind === 'import' ? record.operation.restore?.id : undefined;
      if (!id || !/^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/.test(id)) {
        return <li key={index} className="execution-entry">Identidade da tentativa indisponível.</li>;
      }
      return <li key={id} className="execution-entry"><strong>{Object.hasOwn(labels, record.status) ? labels[record.status] : 'Estado desconhecido'}</strong>
        <p>{shortDate(record.startedAt)} · <span className="mono break-word">{id}</span></p>
        {record.sha256 && <KeyValue label="SHA-256 da cópia"><span className="mono break-word">{record.sha256}</span></KeyValue>}
        {record.bytes != null && <p>{record.bytes.toLocaleString('pt-BR')} bytes</p>}
        {record.status === 'inactive' && record.operation.kind === 'import' && <details><summary>Acessar os arquivos recuperados</summary>
          <p>Abra o PowerShell com seu usuário do Windows e execute o comando abaixo para entrar na cópia como usuário comum. Ao terminar, use <code>exit</code>.</p>
          <code className="mono break-word">wsl.exe --distribution OpenKoda-Recovery-{id} --user openkoda --cd ~</code>
          <p>As avaliações e evidências do aplicativo usam o backup de dados em Configurações. O executor continua vinculado a OpenKoda-Kali.</p>
        </details>}
        {record.error && <p>{record.error}</p>}
        {record.status === 'exported' && <button type="button" className="button" disabled={disabled} onClick={() => void run(() => prepare(id))}>Revisar restauração desta cópia</button>}
        {record.status === 'stop_unknown' && <button type="button" className="button" disabled={disabled} onClick={() => void run(async () => {
          await action('environment.archive.inspectStop', { id }); await reload();
        })}>Conferir se a origem parou</button>}
      </li>;
    })}</ul> : <p>Nenhuma tentativa de cópia registrada.</p>)}
    {plan && backup && <div className="maintenance-review">
      <h4>{plan.payload.restore ? 'Restaurar em ambiente inativo' : 'Exportar cópia do Kali'}</h4>
      <p>{plan.payload.effects}</p>
      <KeyValue label="Formato da cópia">{backup.format === 'tar' ? 'TAR · arquivos da distribuição WSL' : 'VHDX · disco virtual'}</KeyValue>
      {backup.format === 'tar'
        ? <p>O TAR inclui os arquivos exportados da distribuição, inclusive configurações, registros e capturas presentes. A cópia local preserva esse conteúdo sem redação.</p>
        : <p>O VHDX inclui o disco virtual inteiro. O espaço livre pode conter dados já excluídos.</p>}
      {!plan.payload.restore && <>
        <KeyValue label="Spool gerido revisado">{backup.spool.terminalCaptures} captura(s) com bytes removidos · {backup.spool.sessionRecords} registro(s) de sessão</KeyValue></>}
      <KeyValue label="Origem">{backup.source.name} · {backup.sourceId}</KeyValue>
      <KeyValue label="Destino"><span className="mono break-word">{plan.payload.target}</span></KeyValue>
      <KeyValue label={plan.payload.restore ? 'Tamanho verificado' : 'Tamanho máximo revisado'}>{(plan.payload.restore?.bytes ?? backup.maxBytes).toLocaleString('pt-BR')} bytes</KeyValue>
      {plan.payload.restore?.firstWslDistribution && <p>Nenhuma distribuição está registrada neste Windows. A cópia importada pode se tornar a distribuição padrão ao executar wsl.exe sem escolher um nome.</p>}
      {plan.payload.restore && <><KeyValue label="Nova distribuição">{plan.payload.distribution}</KeyValue>
        <KeyValue label="SHA-256 da cópia"><span className="mono break-word">{plan.payload.restore.sha256}</span></KeyValue></>}
      <KeyValue label="Validade">{shortDate(plan.expiresAt)}</KeyValue>
      <label className="check-field"><input type="checkbox" checked={approved} disabled={disabled} onChange={e=>setApproved(e.target.checked)}/>
        <span>{backup.format === 'tar'
          ? 'Revisei a origem e o destino e autorizo a operação com todos os arquivos da cópia TAR, inclusive registros e capturas presentes.'
          : 'Revisei a origem e o destino e autorizo a operação com o conteúdo integral do disco VHDX.'}</span></label>
      <div className="inline-actions">
        <button type="button" className="button primary" disabled={!approved || disabled || Date.parse(plan.expiresAt)<=Date.now()} onClick={() => void run(apply)}>
          {plan.payload.restore ? 'Importar cópia revisada' : 'Exportar cópia revisada'}</button>
        <button type="button" className="button" disabled={disabled} onClick={() => {setPlan(undefined); setApproved(false);}}>Fechar revisão da cópia</button>
      </div>
    </div>}
    {error && <p role="alert">{error}</p>}{message && <p role="status">{message}</p>}
  </section>;
}
