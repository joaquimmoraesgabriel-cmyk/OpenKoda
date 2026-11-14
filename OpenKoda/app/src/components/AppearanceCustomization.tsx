import { useEffect, useId, useRef, useState } from 'react';
import { defaultAppearance, validFontFamily, type DeviceAppearance } from '../appearance';
import type { Settings } from '../types';

type Props = {draft: Settings & DeviceAppearance; change: <K extends keyof (Settings & DeviceAppearance)>(key: K, value: (Settings & DeviceAppearance)[K]) => void};
const samples = {ui: ['Segoe UI', 'Arial', 'Georgia', 'Tahoma'], code: ['Consolas', 'Cascadia Code', 'JetBrains Mono', 'Courier New']};

function FontRow({kind, draft, change}: Props & {kind: 'ui' | 'code'}) {
  const id = useId();
  const familyKey = kind === 'ui' ? 'uiFont' : 'codeFont';
  const sizeKey = kind === 'ui' ? 'uiFontSize' : 'codeFontSize';
  const title = kind === 'ui' ? 'interface' : 'código';
  const font = draft[familyKey];
  return <div className="font-row">
    <div className="pref-font-controls">
      <div><label htmlFor={id}>Fonte de {title}</label>
        <input id={id} list={id + '-fonts'} value={font} placeholder="Padrão do aplicativo" maxLength={70}
          pattern={String.raw`[\p{L}\p{N} _.\-]{0,70}`} title="Nome de uma fonte instalada" onChange={e => change(familyKey, e.target.value)}/>
        <datalist id={id + '-fonts'}>{samples[kind].map(name => <option key={name} value={name}/>)}</datalist>
      </div>
      <div><label htmlFor={id + '-size'}>Tamanho {kind === 'ui' ? 'do texto' : 'do código'}</label>
        <select id={id + '-size'} value={draft[sizeKey]} onChange={e => change(sizeKey, Number(e.target.value))}>
          {Array.from({length: kind === 'ui' ? 13 : 15}, (_, i) => i + (kind === 'ui' ? 12 : 10)).map(size =>
            <option value={size} key={size}>{size} px</option>)}
        </select>
      </div>
    </div>
    <div className="font-preview" aria-label={'Amostra da fonte de ' + title} style={{
      fontFamily: font && validFontFamily(font) ? '"' + font + '", ' + (kind === 'ui' ? 'sans-serif' : 'monospace') : kind === 'ui' ? 'system-ui' : 'monospace',
      fontSize: draft[sizeKey] + 'px',
    }}>{kind === 'ui' ? 'Projetos e avaliações' : 'const escopo = projeto.alvos;'}</div>
  </div>;
}

async function prepareWallpaper(file: File) {
  if (!['image/png','image/jpeg','image/webp'].includes(file.type) || file.size > 8 * 1024 * 1024) {
    throw new Error('Escolha uma imagem PNG, JPG ou WebP de até 8 MB.');
  }
  const bitmap = await createImageBitmap(file);
  try {
    if (bitmap.height / bitmap.width > 8 || bitmap.width / bitmap.height > 8) throw new Error('Escolha uma imagem com proporções mais próximas da janela.');
    const scale = Math.min(1, 1100 / Math.max(bitmap.width, bitmap.height));
    const canvas = document.createElement('canvas');
    canvas.width = Math.max(1, Math.round(bitmap.width * scale));
    canvas.height = Math.max(1, Math.round(bitmap.height * scale));
    const context = canvas.getContext('2d');
    if (!context) throw new Error('O dispositivo não conseguiu preparar a imagem.');
    context.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
    const image = canvas.toDataURL('image/webp', .72);
    if (!image.startsWith('data:image/webp;base64,') || image.length > 470_000) throw new Error('Escolha uma imagem menor para salvar neste dispositivo.');
    return image;
  } finally { bitmap.close(); }
}

export function AppearanceCustomization({draft, change}: Props) {
  const [imageError, setImageError] = useState('');
  const [reading, setReading] = useState(false);
  const fileInput = useRef<HTMLInputElement>(null);
  const fileRequest = useRef(0);
  useEffect(() => () => {fileRequest.current++;}, []);
  async function chooseImage(file?: File) {
    const request = ++fileRequest.current;
    setImageError('');
    if (!file) return;
    setReading(true);
    try {
      const image = await prepareWallpaper(file);
      if (request !== fileRequest.current) return;
      change('wallpaperImage', image);
      change('wallpaper', 'custom');
    } catch (error) {
      if (request === fileRequest.current) setImageError(error instanceof Error && error.name === 'Error' ? error.message : 'Não foi possível abrir essa imagem.');
    } finally {
      if (request === fileRequest.current) setReading(false);
    }
  }
  return <>
    <section className="pref-group">
      <h3>Tema de cores</h3>
      <div className="palette-options" role="radiogroup" aria-label="Tema de cores">
        {([['koda','OpenKoda'],['neutral','Neutro'],['slate','Ardósia'],['sand','Areia']] as const).map(([id,name]) => <label key={id}>
          <input type="radio" name="palette" checked={draft.palette === id} onChange={() => change('palette',id)}/>
          <i aria-hidden="true" data-palette-sample={id}/>{name}</label>)}
      </div>
    </section>
    <section className="pref-group">
      <h3>Papel de parede</h3>
      <div className="wallpapers" role="radiogroup" aria-label="Papel de parede">
        {([['none','Nenhum'],['mist','Névoa'],['dunes','Dunas'],['custom','Minha imagem']] as const).map(([id,label]) =>
          (id !== 'custom' || draft.wallpaperImage) && <label key={id}><input type="radio" name="wallpaper" checked={draft.wallpaper === id}
            onChange={() => {fileRequest.current++; setReading(false); change('wallpaper',id);}}/>
            <i data-sample={id} aria-hidden="true" style={id === 'custom' ? {backgroundImage: 'url("' + draft.wallpaperImage + '")'} : undefined}/>{label}</label>)}
      </div>
      <div className="inline-actions"><button type="button" className="button" disabled={reading} onClick={() => fileInput.current?.click()}>{reading ? 'Preparando imagem…' : 'Escolher imagem…'}</button>
        <input aria-label="Arquivo de papel de parede" hidden type="file" ref={fileInput} accept="image/png,image/jpeg,image/webp" onChange={e => {
          const file = e.target.files?.[0]; e.target.value = ''; void chooseImage(file);
        }}/>
        {draft.wallpaperImage && <button type="button" className="text-button" onClick={() => {
          fileRequest.current++; setReading(false); change('wallpaperImage',''); if (draft.wallpaper === 'custom') change('wallpaper','none');
        }}>Remover imagem</button>}</div>
      {imageError && <p role="alert">{imageError}</p>}
      {draft.wallpaper !== 'none' && <div className="appearance-row"><label htmlFor="backdrop-strength">Visibilidade do fundo</label>
        <input id="backdrop-strength" type="range" min="0" max="20" value={draft.wallpaperOpacity} onChange={e => change('wallpaperOpacity', Number(e.target.value))}/></div>}
      {draft.wallpaper !== 'none' && <div className="appearance-row"><label htmlFor="backdrop-motion">Animação do fundo</label>
        <select id="backdrop-motion" value={draft.wallpaperMotion} onChange={e => change('wallpaperMotion', e.target.value as DeviceAppearance['wallpaperMotion'])}>
          <option value="static">Estático</option><option value="dynamic">Movimento suave</option></select></div>}
    </section>
    <section className="pref-group">
      <div className="pref-section-title"><h3>Fontes</h3><button type="button" className="text-button" onClick={() => {
        for (const key of ['uiFont','uiFontSize','codeFont','codeFontSize'] as const) change(key, defaultAppearance[key]);
      }}>Restaurar fontes</button></div>
      <FontRow kind="ui" draft={draft} change={change}/><FontRow kind="code" draft={draft} change={change}/>
    </section>
    <section className="pref-group">
      <h3>Barra lateral e painéis</h3>
      <div className="appearance-row"><label htmlFor="sidebar-style">Navegação principal</label>
        <select id="sidebar-style" value={draft.sidebarStyle} onChange={e => change('sidebarStyle', e.target.value as DeviceAppearance['sidebarStyle'])}>
          <option value="auto">Adaptar à janela</option><option value="icons">Somente ícones</option></select></div>
      <div className="appearance-row"><label htmlFor="list-style">Avaliações e achados</label>
        <select id="list-style" value={draft.listStyle} onChange={e => change('listStyle', e.target.value as DeviceAppearance['listStyle'])}>
          <option value="preview">Lista com prévia</option><option value="line">Uma linha por item</option></select></div>
    </section>
  </>;
}
