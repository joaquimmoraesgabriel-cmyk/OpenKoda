import { useEffect, useRef, useState } from 'react';
import { File, FolderOpen } from 'lucide-react';
import { native, request } from '../api';
import { errorMessage, Kind } from '../types';
import { Field } from './shared';

interface TargetFieldProps {
  kind: Kind;
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
}

export default function ProjectTargetField({ kind, label, value, onChange, disabled }: TargetFieldProps) {
  const [picking, setPicking] = useState(false);
  const [error, setError] = useState('');
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; };
  }, []);

  const local = kind === 'code' || kind === 'files';
  let hint = 'Informe o caminho completo da fonte.';
  if (local && native) hint = 'Escolha a fonte neste computador ou informe o caminho completo.';
  if (kind === 'network') hint = 'Informe o endereço IP ou a rede CIDR autorizada.';
  if (kind === 'web') hint = 'Informe a URL do site ou da API autorizada.';

  async function pick(selection: 'folder' | 'file') {
    setPicking(true);
    setError('');
    try {
      const path = await request<string | null>('project.pickTarget', { selection });
      if (mounted.current && path !== null) onChange(path);
    } catch (cause) {
      if (mounted.current) setError(errorMessage(cause));
    } finally {
      if (mounted.current) setPicking(false);
    }
  }

  return <Field label={label} hint={hint}>
    <input
      value={value} onChange={event => onChange(event.target.value)}
      required maxLength={4096} disabled={disabled || picking}
      placeholder={kind === 'web' ? 'https://api.exemplo.local' : kind === 'network' ? '192.0.2.10' : 'D:\\Projetos\\meu-projeto'}
    />
    {local && native && <div className="project-picker-actions">
      <button type="button" className="button" disabled={disabled || picking} onClick={() => void pick('folder')}>
        <FolderOpen size={16}/>Escolher pasta
      </button>
      {kind === 'files' && <button type="button" className="button" disabled={disabled || picking} onClick={() => void pick('file')}>
        <File size={16}/>Escolher arquivo
      </button>}
    </div>}
    {error && <span role="alert" className="project-picker-error">{error}</span>}
  </Field>;
}
