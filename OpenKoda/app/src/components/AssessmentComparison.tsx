import { useEffect, useState } from 'react';
import { native, request } from '../api';
import { type Assessment, type Snapshot, kindLabels } from '../types';
import { Field, Notice } from './shared';
import { projectScanReceipts, scanStateLabel } from '../security-scan';

type Run = {assessmentId:string; environmentId:string};
type ComparisonRow = {label:string; values:string[] | null};
const unique = (values:string[]) => Array.from(new Set(values)).sort();

function describe(snapshot:Snapshot, assessment:Assessment, runs:Run[], sourceOnly:boolean):ComparisonRow[] {
  const binding = snapshot.assessmentScopes?.find(item => item.assessmentId === assessment.id && item.id === assessment.scopeId);
  const revisionId = binding?.frozen?.revisionId ?? binding?.currentRevisionId;
  const revision = snapshot.scopeRevisions?.find(item => item.id === revisionId && item.assessmentId === assessment.id
    && (!binding?.frozen || item.digest === binding.frozen.digest));
  const body = revision?.body;
  const targetSet = body?.targetSet;
  const entries = targetSet?.entries;
  const targets = entries?.map(item => `${item.entryId === targetSet?.primaryEntryId ? 'Principal · ' : ''}${kindLabels[item.kind]} · ${item.target}`)
    ?? [`${kindLabels[assessment.kind]} · ${body?.target ?? assessment.target}`];
  const targetRow = {label:'Alvos registrados',values:snapshot.projection.mode === 'metadata_only' ? null : unique(targets)};
  if (sourceOnly) {
    const scans = projectScanReceipts([assessment]).sort((a,b)=>b.createdAt.localeCompare(a.createdAt));
    const latest = scans[0];
    return [
      targetRow,
      {label:'Objetivo da avaliação',values:snapshot.projection.mode === 'metadata_only' ? null : [assessment.objective]},
      {label:'Varreduras do Codex Security',values:snapshot.projection.mode === 'metadata_only' ? null : [String(scans.length)]},
      {label:'Situação da última varredura',values:latest ? [scanStateLabel[latest.status]] : null},
      {label:'Achados informados na última varredura',values:latest?.status === 'completed' && latest.findingsTotal !== undefined ? [String(latest.findingsTotal)] : null},
    ];
  }
  const methods = body ? entries ? [
    ...entries.flatMap(item => item.methods.map(method => `${item.target} · ${method}`)),
    ...(body.targetSet?.commonMethods ?? []).map(method => `Comum · ${method}`),
  ] : body.methods : null;
  const plans = snapshot.plans?.filter(plan => plan.assessmentId === assessment.id) ?? [];
  const manifests = plans.flatMap(plan => plan.sourceScope?.manifestSha256 ? [`${plan.target} · ${plan.sourceScope.manifestSha256}`] : []);
  const environments = runs.filter(run => run.assessmentId === assessment.id && run.environmentId).map(run => run.environmentId);
  const executedPlans = new Set(snapshot.executions?.filter(item => item.assessmentId === assessment.id).map(item => item.planId));
  const adapters = plans.filter(plan => executedPlans.has(plan.id)).map(plan => plan.adapter);
  return [
    targetRow,
    {label:'Métodos da revisão de escopo',values:methods && unique(methods)},
    {label:'Adaptadores com registro de execução',values:adapters.length ? unique(adapters) : null},
    {label:'Fontes nos planos locais (alvo e SHA-256 do manifesto)',values:manifests.length ? unique(manifests) : null},
    {label:'Identidades de ambiente no histórico do Kali',values:environments.length ? unique(environments) : null},
  ];
}

function ComparisonDetails({projectId, before, after, sourceOnly}:{projectId:string; before:string; after:string; sourceOnly:boolean}) {
  const [result,setResult] = useState<{titles:string[]; columns:ComparisonRow[][]; historyUnavailable:boolean}>();
  const [error,setError] = useState('');
  useEffect(() => {
    let active = true;
    async function read() {
      const snapshot = await request<Snapshot>('snapshot');
      const first = snapshot.assessments.find(item => item.projectId === projectId && item.id === before);
      const second = snapshot.assessments.find(item => item.projectId === projectId && item.id === after);
      if (!first || !second || before === after) throw new Error('Selecione duas avaliações diferentes deste projeto.');
      const history = native && !sourceOnly ? await request<Run[]>('kali.mcp.assessmentEnvironments',{
        projectId,assessmentIds:[first.id,second.id],
      }).catch(() => null) : null;
      const records = Array.isArray(history) && history.every(record =>
        (record?.assessmentId === first.id || record?.assessmentId === second.id) && typeof record.environmentId === 'string'
      ) ? history : null;
      if (active) setResult({titles:[first.title,second.title],columns:[describe(snapshot,first,records ?? [],sourceOnly),describe(snapshot,second,records ?? [],sourceOnly)],historyUnavailable:native && !sourceOnly && !records});
    }
    void read().catch(() => { if (active) setError('Não foi possível consultar as avaliações deste projeto. Atualize a comparação.'); });
    return () => { active = false; };
  },[projectId,before,after,sourceOnly]);
  if (error) return <Notice tone="warning">{error}</Notice>;
  if (!result) return <p role="status">Consultando os registros das avaliações…</p>;
  return <div aria-label="Resultado da comparação" role="region">
    {result.historyUnavailable && <Notice tone="warning">O histórico de ambientes do Kali está indisponível.</Notice>}
    {result.columns[0].map((row,index) => {
      const later = result.columns[1][index].values;
      const status = !row.values || !later ? 'Dados insuficientes' : JSON.stringify(row.values) === JSON.stringify(later) ? 'Mesmos valores exibidos' : 'Registros diferentes';
      return <section className="content-section" key={row.label} aria-label={row.label}>
        <h4>{row.label}</h4><p>{status}</p>
        {result.columns.map((column,side) => <div key={side}>
          <strong>{side === 0 ? 'A' : 'B'} · {result.titles[side]}</strong>
          {column[index].values?.length ? <ul className="plain-list break-word">{column[index].values.map(value => <li key={value}>{value}</li>)}</ul>
            : <p>{column[index].values ? 'Nenhum selecionado na revisão.' : 'Sem registro disponível.'}</p>}
        </div>)}
      </section>;
    })}
    <p>A correção de um achado exige reteste e confirmação. Um resultado ausente pode refletir mudanças de escopo ou cobertura.</p>
  </div>;
}

export default function AssessmentComparison({projectId,assessments,sourceOnly=false}:{projectId:string; assessments:Assessment[]; sourceOnly?:boolean}) {
  const [before,setBefore] = useState('');
  const [after,setAfter] = useState('');
  const [generation,setGeneration] = useState(0);
  const options = assessments.filter(item => item.projectId === projectId);
  const valid = before !== after && options.some(item => item.id === before) && options.some(item => item.id === after);
  if (options.length < 2) return null;
  return <details className="disclosure spaced"><summary>Comparar avaliações</summary><div className="disclosure-body">
    <div className="form-group">{(['A','B'] as const).map(side => <Field key={side} label={`Avaliação ${side}`}>
      <select value={side === 'A' ? before : after} onChange={event => {
        setGeneration(0);
        if (side === 'A') setBefore(event.target.value); else setAfter(event.target.value);
      }}><option value="">Selecione uma avaliação</option>
        {options.map(item => <option key={item.id} value={item.id} disabled={item.id === (side === 'A' ? after : before)}>{item.title} · {item.id}</option>)}
      </select>
    </Field>)}</div>
    <button className="button" type="button" disabled={!valid} onClick={() => setGeneration(value => value+1)}>{generation ? 'Atualizar comparação' : 'Comparar registros'}</button>
    {valid && generation > 0 && <ComparisonDetails key={generation} projectId={projectId} before={before} after={after} sourceOnly={sourceOnly}/>}
  </div></details>;
}
