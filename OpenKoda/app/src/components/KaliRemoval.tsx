import { useState } from 'react';
import { Action, errorMessage, shortDate } from '../types';
import { KeyValue, Notice } from './shared';

type Registration = { guid: string; name: string; basePath: string; version: number };
type Plan = { id: string; digest: string | null; expiresAt: string; payload: {
  environmentId: string; registration: Registration; effects: string[];
  spool: { rawCaptures: number; terminalCaptures: number; sessionRecords: number };
} };
type Receipt = { record: null | { environmentId: string; registration: Registration; observation: string; pathStatus: string | null }; confirmedNow: boolean };
const uuid = /^[a-f\d]{8}(?:-[a-f\d]{4}){3}-[a-f\d]{12}$/i;
function knownRegistration(id: string, r?: Registration) {
  return uuid.test(id) && !!r && uuid.test(r.guid.replace(/^\{(.+)\}$/, '$1')) &&
    r.version === 2 && !!r.basePath && (r.name === 'OpenKoda-Kali' || r.name === 'OpenKoda-Recovery-' + id);
}

export default function KaliRemoval({ action, busy }: { action: Action; busy: boolean }) {
  const [plan, setPlan] = useState<Plan>();
  const [receipt, setReceipt] = useState<Receipt>();
  const [name, setName] = useState('');
  const [consent, setConsent] = useState(false);
  const [keep, setKeep] = useState(false);
  const [error, setError] = useState('');
  const [pending, setPending] = useState(false);
  const disabled = busy || pending;
  async function run(task: () => Promise<void>) {
    setPending(true);
    setError('');
    setReceipt(current => current && { ...current, confirmedNow: false });
    try { await task(); } catch (e) { setError(errorMessage(e)); }
    finally { setPending(false); }
  }
  async function review() {
    setPlan(undefined);
    setReceipt(undefined);
    setName('');
    setConsent(false);
    setKeep(false);
    const value = await action<Plan>('environment.removal.plan');
    if (!uuid.test(value.id) || !value.digest || !/^[a-f\d]{64}$/.test(value.digest) ||
        !knownRegistration(value.payload.environmentId, value.payload.registration) || !Array.isArray(value.payload.effects) ||
        ![value.payload.spool?.rawCaptures,value.payload.spool?.terminalCaptures,value.payload.spool?.sessionRecords].every(n => Number.isSafeInteger(n) && n >= 0)) {
      throw new Error('A identidade completa da distribuição precisa estar visível para autorizar a retirada.');
    }
    setPlan(value);
  }
  async function detach() {
    if (!plan || !consent || name !== plan.payload.registration.name) return;
    setPlan(undefined);
    setReceipt(await action<Receipt>('environment.removal.detach', {
      planId: plan.id, digest: plan.digest, confirmName: name, acceptLinuxDataLoss: consent,
    }));
  }
  const row = receipt?.record;
  const registration = row?.registration;
  const canGuide = row && receipt?.confirmedNow && row.observation === 'still_registered' && knownRegistration(row.environmentId, registration);
  return <section className="content-section" aria-label="Remoção do Kali">
    <h3>Retirar a distribuição Kali</h3>
    <p>Você pode <a href="#kali-export">exportar os arquivos do Kali</a> antes da retirada. O OpenKoda desativa o executor. A exclusão dos arquivos Linux exige uma ação sua no Windows.</p>
    <div className="inline-actions">
      <button className="button" disabled={disabled} onClick={() => void run(review)}>Revisar retirada do Kali</button>
      <button className="button" disabled={disabled} onClick={() => void run(async () => { setReceipt(await action<Receipt>('environment.removal.status')); })}>Consultar retirada registrada</button>
    </div>
    {error && <Notice tone="warning">{error}</Notice>}
    {plan && <div className="plan-details">
      <KeyValue label="Distribuição">{plan.payload.registration.name}</KeyValue>
      <KeyValue label="Identidade do ambiente">{plan.payload.environmentId}</KeyValue>
      <KeyValue label="Registro WSL">{plan.payload.registration.guid}</KeyValue>
      <KeyValue label="Local registrado">{plan.payload.registration.basePath}</KeyValue>
      <KeyValue label="Validade da revisão">{shortDate(plan.expiresAt)}</KeyValue>
      <KeyValue label="Capturas com dados brutos no Kali">{plan.payload.spool.rawCaptures}</KeyValue>
      <KeyValue label="Capturas terminadas">{plan.payload.spool.terminalCaptures}</KeyValue>
      <KeyValue label="Registros de sessão no Kali">{plan.payload.spool.sessionRecords}</KeyValue>
      <ul>{plan.payload.effects.map(effect => <li key={effect}>{effect}</li>)}</ul>
      <label>Digite o nome da distribuição a retirar<input value={name} disabled={disabled} onChange={e => setName(e.target.value)}/></label>
      <label className="check-field"><input type="checkbox" checked={consent} disabled={disabled} onChange={e => setConsent(e.target.checked)}/>Entendo que a exclusão no Windows pode apagar os dados Linux desta distribuição.</label>
      <div className="inline-actions"><button className="button" disabled={disabled || !consent || name !== plan.payload.registration.name} onClick={() => void run(detach)}>Desativar este executor</button>
        <button className="button" disabled={disabled} onClick={() => setPlan(undefined)}>Descartar revisão</button></div>
    </div>}
    {receipt && !row && <p role="status">Nenhuma retirada registrada.</p>}
    {row && <div role="status">
      <p>Distribuição registrada na retirada: {row.registration.name}. Identidade do ambiente: {row.environmentId}.</p>
      {row.observation === 'registration_absent' ? <p>O registro WSL da distribuição anterior está ausente.
        {row.pathStatus === 'remaining' ? ' O local original ainda existe. Examine os resíduos no Windows.' :
          row.pathStatus === 'absent' ? ' O local original também estava ausente na conferência.' : ' A presença de arquivos no local original precisa ser conferida.'}</p> :
        row.observation === 'owner_released' ? <p>Vínculo encerrado. O estado dos arquivos e do registro no Windows permanece sem confirmação.</p> :
        <><p>{row.observation === 'registration_conflict' ? 'O registro atual diverge da retirada. Preserve a distribuição e confira o GUID e o caminho no Windows.' :
          'O executor está desativado. Confira o registro antes de agir no Windows e novamente depois da exclusão.'}</p>
          <button className="button" disabled={disabled} onClick={() => void run(async () => { setReceipt(await action<Receipt>('environment.removal.inspect')); })}>Conferir registro no Windows</button>
          <details><summary>Preservar a distribuição</summary>
            <label className="check-field"><input type="checkbox" disabled={disabled} checked={keep} onChange={e=>setKeep(e.target.checked)}/>Quero concluir a retirada local e preservar o que existe no Windows.</label>
            <button className="button" disabled={disabled || !keep} onClick={()=>void run(async()=>{
              setKeep(false);setReceipt(await action<Receipt>('environment.removal.keep',{environmentId:row.environmentId,keepDistribution:true}));
            })}>Encerrar vínculo local</button>
          </details></>}
      {canGuide && registration && <div>
        <p>Em seu PowerShell, confira se DistributionName é {registration.name} e se BasePath é {registration.basePath}.</p>
        <pre>{`Get-ItemProperty -LiteralPath 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Lxss\\{${registration.guid.replace(/[{}]/g,'')}}' | Select-Object DistributionName, BasePath`}</pre>
        <p>Se os dois valores ainda coincidirem e você quiser apagar os dados Linux, execute o comando abaixo e confira o registro novamente aqui.</p>
        <pre>{`wsl.exe --unregister ${registration.name}`}</pre>
      </div>}
    </div>}
  </section>;
}
