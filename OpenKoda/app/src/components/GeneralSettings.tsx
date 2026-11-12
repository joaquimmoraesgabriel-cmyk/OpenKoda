import { useState, type ReactNode } from 'react';
import { useIdentity, saveIdentity } from '../profile-identity';
import { composerShortcut, saveComposerShortcut, type ComposerShortcut } from '../composer-preference';
import HostConnectivity from './HostConnectivity';
import { KeyValue } from './shared';

export default function GeneralSettings({windowOptions}: {windowOptions?: ReactNode}) {
  const identity = useIdentity();
  const [editedName,setEditedName] = useState<string>();
  const [shortcut,setShortcut] = useState(composerShortcut);
  const [nameMessage,setNameMessage] = useState('');
  const [composerMessage,setComposerMessage] = useState('');
  const name = editedName ?? identity.name;
  return <>
    <section className="general-section"><h3>Usuário</h3>
      <form onSubmit={event => {event.preventDefault(); try {
        saveIdentity({name}); setEditedName(undefined); setNameMessage('Nome salvo no perfil.');
      } catch { setNameMessage('O dispositivo não permitiu salvar o nome.'); }}}>
        <div className="general-row"><label htmlFor="general-name">Nome de exibição</label>
          <input id="general-name" maxLength={60} placeholder="Meu perfil" value={name} onChange={e=>{setEditedName(e.target.value);setNameMessage('');}}/></div>
        <div className="inline-actions"><button className="button" type="submit" disabled={name.trim()===identity.name}>Salvar nome</button>
          <span role="status">{nameMessage}</span></div>
      </form>
    </section>
    <section className="general-section" aria-label="Idioma e região"><h3>Idioma e região</h3>
      <KeyValue label="Idioma">Português (Brasil)</KeyValue>
      <KeyValue label="Fuso horário">{Intl.DateTimeFormat().resolvedOptions().timeZone}</KeyValue>
    </section>
    {windowOptions}
    <section className="general-section"><h3>Compositor de mensagens</h3>
      <div className="general-row"><label htmlFor="composer-shortcut">Atalho de envio</label>
        <select id="composer-shortcut" value={shortcut} onChange={e=>{
          const next=e.target.value as ComposerShortcut;
          try { saveComposerShortcut(next); setShortcut(next); setComposerMessage('Atalho salvo.'); }
          catch { setComposerMessage('O dispositivo não permitiu salvar o atalho.'); }
        }}><option value="modifier-enter">Ctrl + Enter</option><option value="enter">Enter</option></select></div>
      <p className="muted">Use o atalho para enviar a mensagem. Quando o envio exigir confirmação, o OpenKoda abrirá a revisão. Shift + Enter insere uma linha.</p>
      <span role="status">{composerMessage}</span>
    </section>
    <HostConnectivity/>
  </>;
}
