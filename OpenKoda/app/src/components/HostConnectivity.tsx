import { useState } from 'react';
import { native, request } from '../api';

const labels = {
  internet: 'Acesso à Internet indicado',
  local: 'Apenas rede local',
  noTraffic: 'Rede conectada sem tráfego detectado',
  disconnected: 'Desconectado da rede',
  unavailable: 'Indicador indisponível',
};
type Status = keyof typeof labels;
type Observation = { source: 'windowsNlm'; status: Status; observedAt: string };

// Leaving and reopening the settings panel can share the same pending IPC call.
let pending: Promise<Observation> | undefined;
function observe(): Promise<Observation> {
  pending ??= request<Observation>('host.connectivity').then(value => {
    if (value?.source !== 'windowsNlm' || !Object.hasOwn(labels, value.status)
      || !Number.isFinite(Date.parse(value.observedAt))) throw new Error('Observação inválida');
    return value;
  }).finally(() => { pending = undefined; });
  return pending;
}

export default function HostConnectivity() {
  const [reading, setReading] = useState(false);
  const [observation, setObservation] = useState<Observation>();

  async function refresh() {
    setReading(true);
    try {
      setObservation(await observe());
    } catch {
      setObservation({ source: 'windowsNlm', status: 'unavailable', observedAt: new Date().toISOString() });
    } finally {
      setReading(false);
    }
  }
  return <section className="content-section" aria-label="Conectividade com a Internet">
    <h2>Internet</h2>
    {native ? <>
      <p>Indicador do Gerenciador de Lista de Redes do Windows.</p>
      <div role="status" aria-live="polite" aria-atomic="true">
        <p>{reading ? 'Consultando o Windows…' : observation ? labels[observation.status] : 'Ainda não consultado'}</p>
        {observation && !reading && <p>Última consulta em <time dateTime={observation.observedAt}>
          {new Date(observation.observedAt).toLocaleString('pt-BR')}
        </time>.</p>}
      </div>
      <button type="button" className="button" disabled={reading} onClick={refresh}>Consultar conectividade</button>
    </> : <p>O indicador do Windows está indisponível na prévia do navegador.</p>}
  </section>;
}
