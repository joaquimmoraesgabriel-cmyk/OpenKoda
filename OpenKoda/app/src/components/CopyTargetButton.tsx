import { Check, Copy } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';

export default function CopyTargetButton({ target }: { target: string }) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle');
  const writing = useRef(false);
  useEffect(() => {
    if (state !== 'copied') return;
    const timer = window.setTimeout(() => setState('idle'), 2200);
    return () => window.clearTimeout(timer);
  }, [state]);

  async function copy() {
    if (writing.current) return;
    writing.current = true;
    try {
      await navigator.clipboard.writeText(target);
      setState('copied');
    } catch {
      setState('failed');
    } finally {
      writing.current = false;
    }
  }

  const Icon = state === 'copied' ? Check : Copy;
  return <div className="target-copy-action">
    <button className="button" type="button" onClick={() => void copy()}>
      <Icon size={15} aria-hidden="true"/><span>Copiar alvo</span>
    </button>
    <span role="status">{state === 'copied' ? 'Alvo copiado.' : state === 'failed' ? 'Selecione o alvo acima e copie manualmente.' : ''}</span>
  </div>;
}
