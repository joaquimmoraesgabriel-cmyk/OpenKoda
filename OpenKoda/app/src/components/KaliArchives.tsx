import { useRef, useState } from 'react';
import { request } from '../api';
import { Action, errorMessage, shortDate } from '../types';
import { KeyValue } from './shared';

type Registration = { name: string; guid: string; basePath: string };
type Backup = { id: string; format?: 'tar' | 'vhdx'; sourceId: string; source: Registration; maxBytes: number; spool: { terminalCaptures: number; sessionRecords: number } };
type Restore = { id: string; backup: Backup; bytes: number; sha256: string; firstWslDistribution: boolean };
type Receipt = {
  operation: { kind: 'export'; backup: Backup } | { kind: 'import'; restore: Restore };
  status: string; startedAt: string; bytes?: number; sha256?: string; error?: string;
  activationState?: 'available' | 'active' | 'previous';
};
type ArchivePage = { records: Receipt[]; nextBeforeId?: string | null };
type HistoryPage = { records: Receipt[]; beforeId: string | null; nextBeforeId: string | null; trail: (string | null)[] };
const archiveId = /^[a-f0-9]{8}(?:-[a-f0-9]{4}){3}-[a-f0-9]{12}$/;
type Plan = {
  id: string; digest: string | null; expiresAt: string;
  payload: { backup?: Backup; restore?: Restore; target: string; effects: string; distribution?: string };
};
type ActivationPlan = {
  id: string; digest: string | null; expiresAt: string;
  payload: { previousEnvironmentId: string; previousDistribution: string; targetEnvironmentId: string;
    targetDistribution: string; registration: Registration & { version: number }; effects: string[] };
};
type ActivationReceipt = { status: string; environmentId: string; distro: string; previousEnvironmentId: string; previousDistribution: string };
const labels: Record<string, string> = {
  exported: 'Cópia verificada', inactive: 'Cópia restaurada e inativa',
  incomplete_copy: 'Exportação incompleta', stop_unknown: 'Parada da origem a conferir',
  copy_unknown: 'Estado da cópia restaurada a conferir', stopping_source: 'Encerrando a origem',
  exporting_source: 'Exportando arquivos do Kali', copying: 'Exportando a cópia',
  importing: 'Importando a cópia', personalizing: 'Conferindo a nova identidade',
  stopping_copy: 'Encerrando a cópia restaurada',
};

export default function KaliArchives({ action, busy, onExecutorChange }: {
  action: Action; busy: boolean; onExecutorChange: (executor?: { distro: string; environmentId: string }) => void;
}) {
  const [history, setHistory] = useState<HistoryPage>();
  const records = history?.records;
  const [plan, setPlan] = useState<Plan>();
  const [activation, setActivation] = useState<ActivationPlan>();
  const [approved, setApproved] = useState(false);
  const [pending, setPending] = useState(false);
  const running = useRef(false);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const disabled = pending || busy;
  function clearReview() { setPlan(undefined); setActivation(undefined); setApproved(false); }
  async function loadPage(beforeId: string | null = null, trail: (string | null)[] = []) {
    clearReview();
    const page = await request<ArchivePage>('environment.archive.list', beforeId ? { beforeId } : {});
    const next = page.nextBeforeId ?? null;
    if (!Array.isArray(page.records) || (next !== null &&
      (!archiveId.test(next) || !page.records.length || next === beforeId || trail.includes(next)))) {
      throw new Error('O histórico devolveu uma página inválida. Consulte as cópias novamente.');
    }
    setHistory({ records: page.records, beforeId, nextBeforeId: next, trail });
  }
  async function run(operation: () => Promise<void>) {
    if (running.current) return;
    running.current = true;
    setPending(true); setError(''); setMessage('');
    try { await operation(); } catch (e) { setError(errorMessage(e)); }
    finally { running.current = false; setPending(false); }
  }
  async function prepare(id?: string) {
    clearReview();
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
  async function prepareActivation(id: string) {
    clearReview();
    const reviewed = await action<ActivationPlan>('environment.archive.activatePlan', { id });
    const change = reviewed.payload;
    if (!archiveId.test(reviewed.id) || !reviewed.digest || !/^[a-f0-9]{64}$/.test(reviewed.digest) ||
        !Number.isFinite(Date.parse(reviewed.expiresAt)) || !change ||
        !archiveId.test(change.previousEnvironmentId) || !archiveId.test(change.targetEnvironmentId) ||
        change.targetEnvironmentId !== id || change.previousEnvironmentId === change.targetEnvironmentId ||
        typeof change.previousDistribution !== 'string' || !/^[a-zA-Z0-9_-]+$/.test(change.previousDistribution) ||
        change.targetDistribution !== 'OpenKoda-Recovery-' + id || change.registration?.name !== change.targetDistribution ||
        change.registration.version !== 2 || typeof change.registration.basePath !== 'string' || !change.registration.basePath.trim() ||
        typeof change.registration.guid !== 'string' || !archiveId.test(change.registration.guid.replace(/^\{(.+)\}$/, '$1').toLowerCase()) ||
        !Array.isArray(change.effects) || !change.effects.length || change.effects.some(effect => typeof effect !== 'string' || !effect.trim())) {
      throw new Error('O vínculo atual ou a identidade da cópia não estão disponíveis para revisão. Consulte o histórico e prepare outra revisão.');
    }
    setActivation(reviewed);
  }
  async function activate() {
    if (!activation?.digest || !approved) return;
    const { id, digest, expiresAt, payload } = activation;
    clearReview();
    if (Date.parse(expiresAt) <= Date.now()) throw new Error('A revisão expirou. Prepare outra revisão do vínculo.');
    try {
      const receipt = await action<ActivationReceipt>('environment.archive.activate', { planId: id, digest });
      if (receipt.status !== 'activated' || receipt.environmentId !== payload.targetEnvironmentId ||
          receipt.distro !== payload.targetDistribution || receipt.previousEnvironmentId !== payload.previousEnvironmentId ||
          receipt.previousDistribution !== payload.previousDistribution) {
        throw new Error('O recibo não confirma a troca revisada. Consulte o histórico para conferir o vínculo atual.');
      }
      onExecutorChange({ distro: receipt.distro, environmentId: receipt.environmentId });
      setMessage('Executor vinculado a ' + receipt.distro + '.');
    } catch (e) {
      onExecutorChange();
      await loadPage().catch(() => {});
      throw e;
    }
    await loadPage();
  }
  async function apply() {
    if (!plan?.digest || !approved) return;
    const reviewed = plan;
    setPlan(undefined); setApproved(false);
    try {
      const receipt = await action<Receipt>(reviewed.payload.restore ? 'environment.archive.import' : 'environment.archive.export',
        { planId: reviewed.id, digest: reviewed.digest });
      setMessage(labels[receipt.status]);
    } finally { await loadPage(); }
  }
  const backup = plan?.payload.backup ?? plan?.payload.restore?.backup;
  return <section className="content-section" aria-label="Cópias do Kali">
    <h3>Cópias do Kali</h3>
    <p>Exporte os arquivos da distribuição gerida para uma cópia TAR local ou restaure uma cópia verificada em uma distribuição separada.</p>
    <div className="inline-actions">
      <button type="button" className="button" disabled={disabled} onClick={() => void run(() => prepare())}>Revisar exportação do Kali</button>
      <button type="button" className="button" disabled={disabled} onClick={() => void run(() => loadPage())}>Consultar cópias do Kali</button>
    </div>
    {records && (records.length ? <ul>{records.map((record, index) => {
      const id = record?.operation?.kind === 'export' ? record.operation.backup?.id :
        record?.operation?.kind === 'import' ? record.operation.restore?.id : undefined;
      if (!id || !archiveId.test(id)) {
        return <li key={index} className="execution-entry">Identidade da tentativa indisponível.</li>;
      }
      const imported = record.status === 'inactive' && record.operation.kind === 'import';
      const stateLabel = imported && record.activationState === 'active' ? 'Cópia vinculada ao executor' :
        imported && record.activationState === 'previous' ? 'Vínculo anterior do executor' :
        Object.hasOwn(labels, record.status) ? labels[record.status] : 'Estado desconhecido';
      return <li key={id} className="execution-entry"><strong>{stateLabel}</strong>
        <p>{shortDate(record.startedAt)} · <span className="mono break-word">{id}</span></p>
        {record.sha256 && <KeyValue label="SHA-256 da cópia"><span className="mono break-word">{record.sha256}</span></KeyValue>}
        {record.bytes != null && <p>{record.bytes.toLocaleString('pt-BR')} bytes</p>}
        {imported && <details><summary>Acessar os arquivos recuperados</summary>
          <p>Abra o PowerShell com seu usuário do Windows e execute o comando abaixo para entrar na cópia como usuário comum. Ao terminar, use <code>exit</code>.</p>
          <code className="mono break-word">wsl.exe --distribution OpenKoda-Recovery-{id} --user openkoda --cd ~</code>
          <p>As avaliações e evidências do aplicativo usam o backup de dados em Configurações.</p>
        </details>}
        {imported && record.activationState === 'available' && <button type="button" className="button" disabled={disabled}
          onClick={() => void run(() => prepareActivation(id))}>Revisar uso desta cópia como executor</button>}
        {record.error && <p>{record.error}</p>}
        {record.status === 'exported' && <button type="button" className="button" disabled={disabled} onClick={() => void run(() => prepare(id))}>Revisar restauração desta cópia</button>}
        {record.status === 'stop_unknown' && <button type="button" className="button" disabled={disabled} onClick={() => void run(async () => {
          await action('environment.archive.inspectStop', { id }); await loadPage(history?.beforeId, history?.trail);
        })}>Conferir se a origem parou</button>}
      </li>;
    })}</ul> : <p>Nenhuma tentativa de cópia registrada.</p>)}
    {history && (history.trail.length > 0 || history.nextBeforeId) && <nav className="inline-actions" aria-label="Páginas de cópias do Kali">
      <button type="button" className="button" disabled={disabled || !history.trail.length}
        onClick={() => void run(() => loadPage(history.trail.at(-1) ?? null, history.trail.slice(0, -1)))}>Tentativas mais recentes</button>
      <span aria-live="polite">Página {history.trail.length + 1}</span>
      <button type="button" className="button" disabled={disabled || !history.nextBeforeId}
        onClick={() => void run(() => loadPage(history.nextBeforeId, [...history.trail, history.beforeId]))}>Tentativas mais antigas</button>
    </nav>}
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
        <button type="button" className="button" disabled={disabled} onClick={clearReview}>Fechar revisão da cópia</button>
      </div>
    </div>}
    {activation && <section className="maintenance-review" aria-label="Revisão do vínculo do executor">
      <h4>Usar a cópia restaurada como executor</h4>
      <KeyValue label="Executor atual">{activation.payload.previousDistribution} · {activation.payload.previousEnvironmentId}</KeyValue>
      <KeyValue label="Executor proposto">{activation.payload.targetDistribution} · {activation.payload.targetEnvironmentId}</KeyValue>
      <KeyValue label="Local registrado"><span className="mono break-word">{activation.payload.registration.basePath}</span></KeyValue>
      <KeyValue label="Registro WSL"><span className="mono break-word">{activation.payload.registration.guid}</span></KeyValue>
      <KeyValue label="Validade da revisão">{shortDate(activation.expiresAt)}</KeyValue>
      <ul>{activation.payload.effects.map((effect, index) => <li key={index}>{effect}</li>)}</ul>
      <label className="check-field"><input type="checkbox" checked={approved} disabled={disabled} onChange={e=>setApproved(e.target.checked)}/>
        <span>Autorizo verificar esta cópia e vinculá-la como executor das próximas operações do Kali.</span></label>
      <div className="inline-actions">
        <button type="button" className="button primary" disabled={disabled || !approved || Date.parse(activation.expiresAt) <= Date.now()}
          onClick={() => void run(activate)}>Confirmar troca de executor</button>
        <button type="button" className="button" disabled={disabled} onClick={clearReview}>Fechar revisão do vínculo</button>
      </div>
    </section>}
    {error && <p role="alert">{error}</p>}{message && <p role="status">{message}</p>}
  </section>;
}
