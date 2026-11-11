import { useRef, useState } from 'react';
import { Play, ShieldCheck } from 'lucide-react';
import { operationLabels, type OperationAdapter } from '../ai-operation';
import { errorMessage, type Action, type ExecutionPlan } from '../types';
import type { ApprovalMode } from '../project-chat';

export function ChatPlanApproval({ plan, mode, disabled, action }: {
  plan: ExecutionPlan;
  mode: ApprovalMode;
  disabled: boolean;
  action: Action;
}) {
  const [review, setReview] = useState(false);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState('');
  const lock = useRef(false);
  const localRead = ['local.inventory.v1', 'source.rules.v1'].includes(plan.adapter);
  const needsReview = mode === 'always' || (mode === 'risk' && !localRead);
  const label = operationLabels[plan.adapter as OperationAdapter] ?? plan.adapter;

  async function execute() {
    if (lock.current || disabled) return;
    if (needsReview && !review) {
      setReview(true);
      return;
    }
    lock.current = true;
    setWorking(true);
    setError('');
    try {
      const decision = { id: plan.id, digest: plan.digest };
      if (plan.status === 'proposed') await action('plan.approve', decision);
      await action('plan.run', decision);
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      lock.current = false;
      setWorking(false);
      setReview(false);
    }
  }
  return <section className="chat-approval" aria-label="Operação pendente">
    <ShieldCheck size={18}/><h3>{label}</h3><p>{plan.target}</p>
    <details open={review || undefined}>
      <summary>Detalhes da operação</summary>
      <p>Prazo de {plan.timeoutSeconds} segundos · saída de até {plan.outputLimitKiB} KiB</p>
      {plan.resolvedAddresses.length > 0 && <p>Destinos · {plan.resolvedAddresses.join(', ')}</p>}
      {!!plan.requestedPorts?.length && <p>Portas · {plan.requestedPorts.join(', ')}</p>}
      {plan.inputFiles.length > 0 && <ul>{plan.inputFiles.map(input => <li key={input.path}>{input.path}</li>)}</ul>}
    </details>
    {error && <p role="alert">{error}</p>}
    <div>
      {review && <button className="button" disabled={working} onClick={() => setReview(false)}>Voltar</button>}
      <button className="button primary" disabled={disabled || working} onClick={() => void execute()}>
        <Play size={15}/>{working ? 'Executando…' : review ? 'Autorizar operação' : 'Executar operação'}
      </button>
    </div>
  </section>;
}

