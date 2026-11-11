import { useEffect, useState } from 'react';
import { native, request } from '../api';
import { errorMessage } from '../types';

export default function WindowBehaviorSettings() {
  const [minimize, setMinimize] = useState<boolean>();
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState('');
  useEffect(() => {
    if (!native) return;
    let active = true;
    void request<{minimize:boolean}>('window.behavior.get').then(result => {
      if (active) setMinimize(result.minimize);
    }).catch(e => { if (active) setMessage(errorMessage(e)); });
    return () => { active = false; };
  }, []);
  async function save(value: boolean) {
    setSaving(true);
    try {
      const result = await request<{minimize:boolean}>('window.behavior.set', {minimize:value});
      setMinimize(result.minimize);
      setMessage('Preferência salva.');
    } catch (e) { setMessage(errorMessage(e)); }
    finally { setSaving(false); }
  }
  return <section className="general-section"><h3>Comportamento da janela</h3>
    <div className="general-row"><label htmlFor="window-close">Ao fechar a janela</label>
      <select id="window-close" disabled={!native || minimize === undefined || saving}
        value={minimize ? 'minimize' : 'exit'} onChange={e=>void save(e.target.value === 'minimize')}>
        <option value="exit">Encerrar o aplicativo</option><option value="minimize">Minimizar na barra de tarefas</option>
      </select></div>
    <p className="muted">{!native ? 'Disponível no aplicativo para Windows.' : minimize
      ? 'Para encerrar, escolha Encerrar o aplicativo e feche a janela.' : 'Se houver uma operação em andamento, você poderá revisá-la antes de sair.'}</p>
    {message && <p role="status">{message}</p>}
  </section>;
}
