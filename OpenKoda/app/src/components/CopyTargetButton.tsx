import { useEffect, useRef, useState } from 'react';
import { CopyFeedback } from './motion/CopyFeedback';

export default function CopyTargetButton({ target }: { target: string }) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle');
  const writing = useRef(false);
  useEffect(() => {
    if (state !== 'copied') return;
    const timer = window.setTimeout(() => setState('idle'), 1800);
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

  return <div className="target-copy-action">
    <button className="button" type="button" aria-label="Copiar alvo" onClick={() => void copy()}>
      <CopyFeedback copied={state==='copied'} label="Copiar alvo" done="Alvo copiado"/>
    </button>
    <span role="status">{state === 'copied' ? 'Alvo copiado.' : state === 'failed' ? 'Selecione o alvo acima e copie manualmente.' : ''}</span>
  </div>;
}
