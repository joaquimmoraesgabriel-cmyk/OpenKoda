import { useId, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { Check, CircleDot, ListChecks, Paperclip, Plus, ShieldCheck, ShieldQuestion } from 'lucide-react';
import { approvalModes, type ApprovalMode } from '../project-chat';

type MenuProps = {
  label: string;
  title?: string;
  trigger: ReactNode;
  disabled: boolean;
  children: ReactNode;
  kind: 'actions' | 'approval';
  triggerLabel?: string;
};

function ComposerMenu({ label, title, trigger, disabled, children, kind, triggerLabel = label }: MenuProps) {
  const id = useId();
  const anchor = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  useLayoutEffect(() => {
    const button = anchor.current, panel = menu.current;
    if (!open || !button || !panel) return;
    function position() {
      if (!button || !panel) return;
      const box = button.getBoundingClientRect();
      const margin = 8, gap = 6;
      const above = box.top - margin - gap, below = innerHeight - box.bottom - margin - gap;
      panel.style.maxHeight = Math.max(0, above, below) + 'px';
      panel.style.left = Math.max(margin, Math.min(box.left, innerWidth - margin - panel.offsetWidth)) + 'px';
      panel.style.top = (above >= below ? box.top - gap - panel.offsetHeight : box.bottom + gap) + 'px';
    }
    position();
    const first = panel.querySelector<HTMLButtonElement>('button[aria-checked="true"]:enabled') ?? panel.querySelector<HTMLButtonElement>('button:enabled');
    first?.focus({ preventScroll: true });
    const observer = new ResizeObserver(position);
    observer.observe(button); observer.observe(panel);
    window.addEventListener('resize', position);
    window.addEventListener('scroll', position, true);
    return () => {
      observer.disconnect(); window.removeEventListener('resize', position); window.removeEventListener('scroll', position, true);
    };
  }, [open]);
  useLayoutEffect(() => {
    if (disabled && menu.current?.matches(':popover-open')) menu.current.hidePopover();
  }, [disabled]);
  function close() {
    menu.current?.hidePopover();
    anchor.current?.focus({ preventScroll: true });
  }
  return <>
    <button type="button" className={'composer-menu-trigger composer-' + kind + '-trigger'} popoverTarget={id}
      ref={anchor} disabled={disabled} aria-label={triggerLabel} title={title ?? label} aria-expanded={open} aria-haspopup="menu"
      onKeyDown={e => { if (e.key === 'ArrowDown') { e.preventDefault(); menu.current?.showPopover(); } }}>
      {trigger}
    </button>
    <div ref={menu} id={id} role="menu" popover="auto" aria-label={label} className={'composer-menu composer-' + kind + '-menu'}
      onToggle={e => setOpen(e.currentTarget.matches(':popover-open'))}
      onClick={e => { if (e.target instanceof Element && e.target.closest('button:enabled')) close(); }}
      onBlur={e => { if (e.relatedTarget instanceof Element && !e.currentTarget.contains(e.relatedTarget) && e.relatedTarget !== anchor.current) e.currentTarget.hidePopover(); }}
      onKeyDown={e => {
        if (e.key === 'Tab') {
          // The popover items are outside the normal tab order. Resume from
          // the trigger so hiding the currently focused item cannot drop focus.
          close();
          if(e.shiftKey)e.preventDefault();
          return;
        }
        if (e.key === 'Escape') { e.preventDefault(); close(); return; }
        if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) return;
        e.preventDefault();
        const options = Array.from(e.currentTarget.querySelectorAll<HTMLButtonElement>('button:enabled'));
        const index = options.indexOf(e.target as HTMLButtonElement);
        if (e.key === 'Home') options[0]?.focus();
        else if (e.key === 'End') options.at(-1)?.focus();
        else options[(index + (e.key === 'ArrowUp' ? -1 : 1) + options.length) % options.length]?.focus();
      }}>
      {children}
    </div>
  </>;
}

type Props = {
  mode: ApprovalMode;
  onMode: (mode: ApprovalMode) => void;
  proposing: boolean;
  hasAttachment: boolean;
  disabled: boolean;
  canPropose: boolean;
  onAttach: () => void;
  onPropose: () => void;
  onAnalyze?: () => void;
  onCompact?: () => void;
  onSkills?: () => void;
};
const choices = [
  { mode: 'always', trigger: 'Sempre solicitar', icon: ShieldQuestion, description: 'Confirme cada mensagem que escrever e cada operação que escolher executar.' },
  { mode: 'risk', trigger: 'Aprovação por risco', icon: ShieldCheck, description: 'Confirme o envio de anexos e as operações que vão além da leitura local.' },
  { mode: 'scope', trigger: 'Dentro do escopo', icon: CircleDot, description: 'Envie mensagens diretamente. O clique em Executar autoriza a operação prevista no escopo.' },
] as const;

export function ChatComposerMenus({ mode, onMode, proposing, hasAttachment, disabled, canPropose, onAttach, onPropose, onAnalyze, onCompact, onSkills }: Props) {
  const id = useId();
  const current = choices.find(choice => choice.mode === mode)!;
  const Icon = current.icon;
  return <>
    <ComposerMenu kind="actions" label="Adicionar à mensagem" trigger={<Plus size={19}/>} disabled={disabled}>
      <button type="button" role="menuitem" tabIndex={-1} disabled={proposing} onClick={onAttach}><Paperclip size={16}/>Anexar foto ou arquivo</button>
      {onCompact&&<button type="button" role="menuitem" tabIndex={-1} onClick={onCompact}>Compactar contexto</button>}
      {onSkills&&<button type="button" role="menuitem" tabIndex={-1} onClick={onSkills}>Skills da assinatura</button>}
      {onAnalyze&&<button type="button" role="menuitem" tabIndex={-1} disabled={hasAttachment} onClick={onAnalyze}><ListChecks size={16}/>Analisar código do projeto</button>}
      {canPropose && <button type="button" role="menuitemcheckbox" tabIndex={-1} aria-checked={proposing} disabled={hasAttachment} onClick={onPropose}>
        <ListChecks size={16}/>Pedir verificação{proposing && <Check size={15} className="composer-selected"/>}
      </button>}
    </ComposerMenu>
    <ComposerMenu kind="approval" label="Modo de aprovação" title={'Modo de aprovação · ' + approvalModes[mode]} disabled={disabled}
      triggerLabel={'Modo de aprovação, ' + current.trigger}
      trigger={<><Icon size={16}/><span>{current.trigger}</span></>}>
      <p className="composer-menu-title">Aprovação nesta conversa</p>
      {choices.map(choice => <button type="button" role="menuitemradio" tabIndex={-1} key={choice.mode} aria-checked={mode === choice.mode}
        aria-label={approvalModes[choice.mode]} aria-describedby={id + choice.mode} onClick={() => onMode(choice.mode)}>
        <choice.icon size={16}/><span><strong>{approvalModes[choice.mode]}</strong><small id={id + choice.mode}>{choice.description}</small></span>
        {mode === choice.mode && <Check size={15} className="composer-selected"/>}
      </button>)}
      <p className="composer-menu-note">Envios com dados sensíveis ou reserva de orçamento exigem confirmação em todos os modos.</p>
    </ComposerMenu>
  </>;
}
