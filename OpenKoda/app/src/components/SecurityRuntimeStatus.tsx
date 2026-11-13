import { useEffect, useState } from 'react';
import { native, request } from '../api';
import { errorMessage } from '../types';
import { KeyValue } from './shared';

type Status = { reason?: string; runtime: { available: boolean; version?: string } };

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
    <p>A revisão estática envia trechos do código selecionado ao provedor do projeto.</p>
    {!native ? <p>Consulte os componentes no aplicativo desktop.</p>
      : error ? <p role="alert">{error}</p>
      : !status ? <p role="status">Verificando os componentes…</p>
      : <>
        <KeyValue label="Estado">{status.runtime.available ? 'Pronto para executar' : 'Requer configuração'}</KeyValue>
        <KeyValue label="Motor de análise">{status.runtime.version ?? 'Integrado'}</KeyValue>
        <KeyValue label="Executor">Revisão estática por trechos</KeyValue>
        {status.reason && <p>{status.reason}</p>}
      </>}
    {native && <button type="button" className="button" onClick={() => refresh(value => value + 1)}>Verificar novamente</button>}
  </section>;
}
