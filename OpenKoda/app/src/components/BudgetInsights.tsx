import { useState } from 'react';
import { ArrowRight, ChartNoAxesColumn, Wallet } from 'lucide-react';
import type { BudgetStatus, BudgetCharge } from '../ai-budget';
import { providers } from '../ai-provider';
import { shortDate } from '../types';

const count = new Intl.NumberFormat('pt-BR');
const short = new Intl.DateTimeFormat('pt-BR', {day:'2-digit', month:'2-digit'});
const states: Record<string, string> = {
  reserved: 'Reservado', dispatched:'Enviado', unknown:'Custo desconhecido', priced:'Estimado',
  unpriced:'Sem tarifa', assumed:'Débito assumido', not_dispatched:'Sem envio',
};

export function readableAmount(value?: string | null) {
  if (value == null || !/^\d+(?:\.\d{1,6})?$/.test(value)) return 'Não informado';
  const [whole, fraction = ''] = value.split('.');
  const decimals = fraction.replace(/0+$/, '').padEnd(2, '0');
  return 'USD ' + count.format(BigInt(whole)) + ',' + decimals;
}

export function BudgetOverview({status, onPolicy, policyOpen}: {status:BudgetStatus; onPolicy:()=>void; policyOpen:boolean}) {
  const balance = status.installation;
  const limit = Number(balance?.limitUsd);
  const remaining = balance?.remainingUsd;
  const percent = remaining != null && limit > 0 ? Math.max(0, Math.min(100, (1 - Number(remaining) / limit) * 100)) : null;
  const committedLabel = percent === null ? undefined : percent.toLocaleString('pt-BR', {maximumFractionDigits:1}) + '% comprometido';
  return <div className="usage-overview">
    <section className="billing-group" aria-labelledby="usage-limit-heading">
      <div className="limit-heading"><span className="usage-glyph"><Wallet size={20} aria-hidden="true"/></span>
        <div className="insight-title"><div><h3 id="usage-limit-heading">Orçamento da instalação</h3>
          <p>{status.degraded ? 'Registro indisponível' : status.enabled ? 'Controle monetário ativado' : 'Controle monetário desativado'}</p></div></div>
      </div>
      <div className="billing-balance-card">
        <div className="usage-available">
          <p className="balance-caption">Saldo estimado disponível</p>
          <div className="balance-amount">{balance && balance.limitUsd == null ? 'Sem limite definido' : readableAmount(remaining)}</div>
          {percent !== null && <><progress className="balance-meter" max={100} value={percent} aria-label="Percentual comprometido do limite" aria-valuetext={committedLabel}/>
            <div className="balance-fractions"><span>{committedLabel}</span><span>Limite de {readableAmount(balance?.limitUsd)}</span></div></>}
        </div>
        <div className="budget-card-footer"><span>Custos pendentes <strong>{balance?.unresolvedCount ?? 'Não informado'}</strong></span>
        <button type="button" className="button" aria-expanded={policyOpen} aria-controls={policyOpen ? 'usage-limits' : undefined} onClick={onPolicy}>{policyOpen ? 'Fechar limites e preços' : 'Gerenciar limites e preços'}<ArrowRight size={14} aria-hidden="true"/></button></div>
      </div>
    </section>
    <section className="billing-group">
      <h3>Composição do orçamento</h3>
      <dl className="usage-balance-lines billing-breakdown" aria-label="Composição do orçamento">
        <div><dt><i className="series-input"/><span>Estimativa com tokens informados<small>Calculada pelas tarifas do modelo e do provedor.</small></span></dt><dd>{readableAmount(balance?.pricedUsageUsd)}</dd></div>
        <div><dt><i className="series-output"/><span>Reservas retidas<small>Valores separados para solicitações em andamento ou com resultado desconhecido.</small></span></dt><dd>{readableAmount(balance?.heldUsd)}</dd></div>
        <div><dt><i className="series-assumed"/><span>Débitos assumidos<small>Estimativas registradas ao revisar custos desconhecidos.</small></span></dt><dd>{readableAmount(balance?.assumedChargeUsd)}</dd></div>
      </dl>
    </section>

  </div>;
}

type DayUsage = {date: Date; input: number; output: number; inputs: number; outputs: number; partial: number};
function tokenCount(n: number | null | undefined): n is number {
  return typeof n === 'number' && Number.isSafeInteger(n) && n >= 0;
}

export function usageDays(charges: BudgetCharge[], length: number, today = new Date()): DayUsage[] {
  const days = Array.from({length}, (_, i) => {
    const date = new Date(today);
    date.setHours(0, 0, 0, 0);
    date.setDate(date.getDate() + i + 1 - length);
    return {date, input:0, output:0, inputs:0, outputs:0, partial:0};
  });
  const bucket = new Map(days.map(day => [day.date.toDateString(), day]));
  for (const entry of charges) {
    const date = new Date(entry.createdAt);
    const day = bucket.get(date.toDateString());
    if (!day || date > today || entry.state === 'not_dispatched') continue;
    if (tokenCount(entry.inputTokens)) { day.input += entry.inputTokens; day.inputs++; }
    if (tokenCount(entry.outputTokens)) { day.output += entry.outputTokens; day.outputs++; }

    if (!tokenCount(entry.inputTokens) || !tokenCount(entry.outputTokens)) day.partial++;
  }
  return days;
}

export function BudgetAnalytics({status}: {status: BudgetStatus}) {
  const [length, setLength] = useState(7);
  const [modelFilter, setModelFilter] = useState('');
  const modelOptions = new Map<string, string>();
  for (const charge of status.recentCharges) {
    const id = chargeModelKey(charge);
    if (modelOptions.has(id)) continue;
    const provider = Object.values(providers).find(value => value.endpoint === charge.endpoint);
    const name = provider?.name ?? (charge.endpoint || 'Provedor não registrado');
    modelOptions.set(id, name + ' · ' + charge.model);
  }
  const selected = modelOptions.has(modelFilter) ? modelFilter : '';
  const records = selected ? status.recentCharges.filter(entry => chargeModelKey(entry) === selected) : status.recentCharges;
  const days = usageDays(records, length);
  const peak = Math.max(1, ...days.map(day => day.input + day.output));
  const total = days.reduce((sum, d) => ({
    input:sum.input+d.input, output:sum.output+d.output, inputs:sum.inputs+d.inputs, outputs:sum.outputs+d.outputs, partial:sum.partial+d.partial,
  }), {input:0, output:0, inputs:0, outputs:0, partial:0});
  const truncated = status.recentChargesTotal !== null && status.recentChargesTotal > status.recentCharges.length;
  if (status.degraded) return <p className="insight-empty">O registro de orçamento está indisponível. Atualize para consultar o histórico.</p>;
  return <div className="usage-analytics">
    {truncated && <p className="analysis-scope">As análises consideram os {status.recentCharges.length} registros mais recentes de {count.format(status.recentChargesTotal!)}.</p>}
    {modelOptions.size > 0 && <label className="usage-model-filter">
      Modelo no histórico
      <select value={selected} onChange={event => setModelFilter(event.target.value)}>
        <option value="">Todos os modelos</option>
        {[...modelOptions].sort((a,b) => a[1].localeCompare(b[1],'pt-BR')).map(([id,name]) => <option key={id} value={id}>{name}</option>)}
      </select>
    </label>}
    <section aria-labelledby="tokens-heading">
      <div className="insight-title"><div><h3 id="tokens-heading">Atividade de tokens</h3><p>Entrada e saída informadas pelos provedores</p></div>
        <div className="insight-switch" role="group" aria-label="Período dos tokens">
          {[7, 30].map(value => <button type="button" key={value} aria-pressed={length === value} onClick={() => setLength(value)}>{value} dias</button>)}
        </div>
      </div>
      <div className="usage-chart">
        <div className="chart-totals"><span><i className="series-input"/>Entrada<strong>{total.inputs ? count.format(total.input) : '—'}</strong></span>
          <span><i className="series-output"/>Saída<strong>{total.outputs ? count.format(total.output) : '—'}</strong></span></div>
        {total.inputs + total.outputs > 0 ? <ol className="token-bars" aria-label="Tokens informados por dia">
          {days.map(day => <li key={day.date.getTime()} aria-label={short.format(day.date) + (day.inputs || day.outputs ? ' · Entrada: ' + (day.inputs ? count.format(day.input) : 'não informada') + ' · Saída: ' + (day.outputs ? count.format(day.output) : 'não informada') : ' · Sem dados de tokens')}>
            <div aria-hidden="true" className="token-stack" title={short.format(day.date) + ' · ' + count.format(day.input + day.output) + ' tokens informados'}>
              <i className="series-input" style={{height:(day.input / peak * 100) + '%'}}/><i className="series-output" style={{height:(day.output / peak * 100) + '%'}}/>
            </div>
          </li>)}
        </ol> : <div className="tokens-empty"><ChartNoAxesColumn size={24} aria-hidden="true"/><span>Nenhum dado de tokens informado no período.</span></div>}
        <div className="heatmap-range"><span>{short.format(days[0].date)}</span><span>{short.format(days[days.length-1].date)}</span></div>
      </div>
      {total.partial > 0 && <p className="analysis-scope">{total.partial === 1 ? '1 registro com contagem de tokens incompleta.' : count.format(total.partial) + ' registros com contagens de tokens incompletas.'}</p>}
    </section>
    <section aria-labelledby="recent-charge-heading"><div className="insight-title"><div><h3 id="recent-charge-heading">Registros recentes de custo</h3><p>{records.length ? 'Abra um modelo para conferir o detalhe do registro.' : 'Ainda não há registros de custo nesta instalação.'}</p></div></div>
      {records.length > 0 && <div className="usage-table-scroll"><table className="usage-table"><caption className="visually-hidden">Histórico de custos da instalação</caption>
        <thead><tr><th scope="col">Modelo e data</th><th scope="col">Estado</th><th scope="col">Estimativa</th></tr></thead><tbody>
          {records.map(entry => <tr key={entry.chargeId}><td><details><summary>{entry.model}<small>{shortDate(entry.createdAt)}</small></summary>
            <dl className="charge-values"><dt>Registro</dt><dd>{entry.chargeId}</dd>{entry.endpoint && <><dt>Endpoint</dt><dd>{entry.endpoint}</dd></>}
              <dt>Reserva original</dt><dd>{readableAmount(entry.reservedUsd)}</dd><dt>Débito assumido</dt><dd>{readableAmount(entry.assumedChargeUsd)}</dd>
              <dt>Tokens de entrada</dt><dd>{entry.inputTokens ?? 'Não informado'}</dd><dt>Tokens de saída</dt><dd>{entry.outputTokens ?? 'Não informado'}</dd></dl>
          </details></td><td>{states[entry.state] ?? entry.state}</td><td>{readableAmount(entry.pricedUsageUsd)}</td></tr>)}
        </tbody></table></div>}
    </section>
  </div>;
}

function chargeModelKey(charge: BudgetCharge) {
  return JSON.stringify([charge.endpoint ?? '', charge.model]);
}
