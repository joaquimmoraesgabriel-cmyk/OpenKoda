import { useEffect, useState } from 'react';
import { native, request } from '../api';
import { errorMessage } from '../types';
import { KeyValue } from './shared';

type Status = { reason?: string; runtime: { available: boolean; sdkVersion: string; nodeVersion: string; pythonVersion: string; gitVersion: string; codexVersion: string; missing: string[] } };

export default function SecurityRuntimeStatus() {
  const [status, setStatus] = useState<Status>();
  const [error, setError] = useState('');
  const [revision, refresh] = useState(0);
  useEffect(() => {
    if (!native) return;
    let active = true;
    setStatus(undefined); setError('');
    void request<Status>('security.status').then(value => {
      if (active) {
        if (!value?.runtime || typeof value.runtime.available !== 'boolean') throw new Error('Não foi possível consultar o motor de análise.');
        setStatus(value);
      }
    }).catch(cause => { if (active) setError(errorMessage(cause)); });
    return () => { active = false; };
  }, [revision]);
  return <section className="content-section" aria-label="Motor de análise">
    <h3>Motor de análise</h3>
    <p>O Codex Security executa as análises de código. Os resultados e o histórico ficam associados ao projeto.</p>
    {!native ? <p>Consulte os componentes no aplicativo desktop.</p>
      : error ? <p role="alert">{error}</p>
      : !status ? <p role="status">Verificando os componentes…</p>
      : <>
        <KeyValue label="Estado">{status.runtime.available ? 'Pronto para executar' : 'Requer configuração'}</KeyValue>
        <KeyValue label="Codex Security">{status.runtime.sdkVersion ?? 'Indisponível'}</KeyValue>
        <KeyValue label="Codex">{status.runtime.codexVersion ?? 'Indisponível'}</KeyValue>
        <KeyValue label="Node.js">{status.runtime.nodeVersion ?? 'Indisponível'}</KeyValue>
        <KeyValue label="Python">{status.runtime.pythonVersion ?? 'Indisponível'}</KeyValue>
        <KeyValue label="Git">{status.runtime.gitVersion ?? 'Instale o Git para Windows'}</KeyValue>
        {status.runtime.missing?.length>0&&<p>Não foi possível verificar: {status.runtime.missing.join(', ')}. Use Verificar novamente para repetir o diagnóstico.</p>}
        {status.reason && <p>{status.reason}</p>}
      </>}
    {native && <button type="button" className="button" onClick={() => refresh(value => value + 1)}>Verificar novamente</button>}
  </section>;
}
