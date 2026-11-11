import { Check, CornerDownLeft } from 'lucide-react';
import type { Settings } from '../types';
import { interfaceScales, type DeviceAppearance } from '../appearance';

const themes = [['system', 'Sistema'], ['light', 'Claro'], ['dark', 'Escuro']] as const;

function ThemeWindow({dark = false}: {dark?: boolean}) {
  return <span className={`theme-window${dark ? ' theme-window-dark' : ''}`}>
    <span className="theme-window-nav"><i/><i/><i/></span>
    <span className="theme-window-body"><i/><i/><i/></span>
  </span>;
}

export function AppearanceSettings({draft, change, busy}: {
  draft: Settings & DeviceAppearance;
  change: <K extends keyof (Settings & DeviceAppearance)>(key: K, value: (Settings & DeviceAppearance)[K]) => void;
  busy: boolean;
}) {
  return <fieldset className="appearance-controls" disabled={busy}>
    <fieldset className="theme-options">
      <legend className="visually-hidden">Tema</legend>
      {themes.map(([id,label]) => <label key={id} className="theme-option">
        <input type="radio" name="interface-theme" value={id} checked={draft.theme === id} onChange={() => change('theme', id)}/>
        <span aria-hidden="true" className={`theme-sample theme-sample-${id}`}>
          {id !== 'dark' && <ThemeWindow/>}{id !== 'light' && <ThemeWindow dark/>}
          <span className="theme-check"><Check size={13} strokeWidth={3}/></span>
        </span>
        <span>{label}</span>
      </label>)}
    </fieldset>
    <div className="appearance-row">
      <div><label htmlFor="appearance-scale">Tamanho da interface</label><small>Escala de textos, ícones e controles.</small></div>
      <select id="appearance-scale" value={draft.interfaceScale} onChange={e => change('interfaceScale', Number(e.target.value))}>
        {interfaceScales.map(scale => <option key={scale} value={scale}>{scale}%</option>)}
      </select>
    </div>
    <div className="appearance-row">
      <div><strong id="density-title">Densidade</strong><small>Espaçamento das listas e dos formulários.</small></div>
      <div className="density-options" role="radiogroup" aria-labelledby="density-title">
        {([['comfortable','Confortável'], ['compact','Compacta']] as const).map(([id,label]) =>
          <label key={id}><input type="radio" name="interface-density" checked={draft.density === id} onChange={() => change('density', id)}/><span>{label}</span></label>
        )}
      </div>
    </div>
    <div className="appearance-row">
      <div><label htmlFor="appearance-motion">Movimento</label><small>Animações e transições da interface.</small></div>
      <select id="appearance-motion" value={draft.motion} onChange={e => change('motion', e.target.value as DeviceAppearance['motion'])}>
        <option value="system">Seguir o sistema</option><option value="reduced">Reduzir movimento</option>
      </select>
    </div>
  </fieldset>;
}

export function KeyboardTips() {
  return <section className="settings-sheet settings-keys" aria-label="Atalhos de teclado">
    <h2>Atalhos de teclado</h2>
    <div><span>Próximo controle</span><kbd>Tab</kbd></div>
    <div><span>Controle anterior</span><kbd>Shift + Tab</kbd></div>
    <div><span>Acionar o botão em foco</span><kbd><CornerDownLeft size={12} aria-hidden="true"/>Enter</kbd></div>
  </section>;
}
