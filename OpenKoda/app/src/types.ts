import { workReason } from './workflow-types';
import type { ProjectTarget, ScopeTargetRef } from './combined-scope';
import { dataReason } from './data-policy';
import type { AssessmentScope, ScopeAdmission, ScopeReceipt, ScopeRef, ScopeRevision } from './scope';
export type Kind = 'code' | 'web' | 'network' | 'files';
export type View = 'assessments' | 'findings' | 'projects' | 'kali' | 'settings';
export type AssessmentStatus = 'draft' | 'preparing' | 'planning' | 'awaiting_approval' | 'queued' | 'running' | 'pausing' | 'paused' | 'validating' | 'completed' | 'partial' | 'canceling' | 'canceled' | 'blocked' | 'failed' | 'unknown';
export interface Project { additionalTargets?:ProjectTarget[]; id: string; name: string; kind: Kind; target: string; description: string; createdAt: string; archived?: boolean }
export interface ActivityEvent { id: string; at: string; message: string; kind: string }
export type Evidence = import('./data-policy').EvidenceView;
export interface Assessment { scopeId?:string; id: string; projectId: string; title: string; kind: Kind; target: string; objective: string; status: AssessmentStatus; createdAt: string; updatedAt: string; events: ActivityEvent[]; evidence: Evidence[]; coverage: string[]; summary: string }
export interface Finding { id: string; assessmentId: string; title: string; severity: 'low' | 'medium' | 'high' | 'critical'; confidence: string; status: 'candidate' | 'validated' | 'dismissed' | 'fixed'; description: string; recommendation: string; evidenceIds: string[]; createdAt: string }
export interface Settings { provider: string; endpoint: string; model: string; mode: string; theme: string; language: string; retentionDays: number; timeoutSeconds: number; outputLimitKiB: number; hasApiKey: boolean }
export interface InputFile { path:string; size:number; sha256:string }
export interface FileIdentity {volume:string;index:string}
export interface SourceScope {version:number;rulesVersion:string;rootIdentity:FileIdentity;targetIsFile:boolean;bindings:{path:string;identity:FileIdentity}[];excludedDirectories:{path:string;reason:string}[];manifestSha256:string}
export interface ExecutionPlan { targetRef?:ScopeTargetRef; requestedPorts?:number[]; identityPreflightRefs?:ScopeTargetRef[]; scopeOmissions?:import('./combined-scope').ScopeOmission[]; scopeRef?:ScopeRef; id:string; assessmentId:string; adapter:string; target:string; timeoutSeconds:number; outputLimitKiB:number; inputFiles:InputFile[]; resolvedAddresses:string[]; sourceScope?:SourceScope|null; digest:string; createdAt:string; expiresAt:string; approvedAt:string|null; status:string }
export interface CandidateDraft {assessmentId:string;title:string;description:string;recommendation:string;evidenceIds:string[]}
export interface Execution { scopeAdmission?:ScopeAdmission|null; id:string; planId:string; assessmentId:string; status:string; startedAt:string; finishedAt:string|null; evidenceId:string|null; message:string }
export interface FindingValidation { id:string; findingId:string; assessmentId:string; method:string; expected:string; observed:string; evidence:{id:string;sha256:string}[]; author:string; createdAt:string }
export interface Snapshot { workPlanBindings?:import('./workflow-types').WorkPlanBinding[];schema:'openkoda.snapshot-view.v1';projection:import('./data-policy').ProjectionInfo;scopeRevisions?:ScopeRevision[]; assessmentScopes?:AssessmentScope[]; scopeReceipts?:ScopeReceipt[]; projects: Project[]; assessments: Assessment[]; findings: Finding[]; settings: Settings; plans?:ExecutionPlan[]; executions?:Execution[]; validations?:FindingValidation[] }
export interface Environment { status: 'unavailable' | 'ready' | 'degraded'; distro: string; message: string; tools: string[]; environmentId?:string; phase?:string; updatedAt?:string; details?:unknown; executorReady?:boolean; containment?:string; lastError?:string; imageProvisionAvailable?:boolean; managedRegistration?:boolean; repairAvailable?:boolean }
export type Payload = Record<string, unknown>;
export type Action = <T>(action: string, payload?: Payload) => Promise<T>;
export const kindLabels: Record<Kind, string> = { code: 'Código', web: 'Web / API', network: 'Rede', files: 'Arquivos' };
export const statusLabels: Record<AssessmentStatus, string> = { draft:'Rascunho', preparing:'Preparando', planning:'Planejando', awaiting_approval:'Aguardando aprovação', queued:'Na fila', running:'Em execução', pausing:'Pausando', paused:'Pausada', validating:'Validando resultados', completed:'Concluída', partial:'Parcial', canceling:'Cancelando', canceled:'Cancelada', blocked:'Bloqueada', failed:'Falhou', unknown:'Efeito desconhecido' };
export const findingLabels: Record<Finding['status'], string> = { candidate: 'Candidato', validated: 'Validado', dismissed: 'Descartado', fixed: 'Corrigido' };
export const severityLabels: Record<Finding['severity'], string> = { low: 'Baixa', medium:'Média', high:'Alta', critical:'Crítica' };
export const terminalStatuses: AssessmentStatus[] = ['completed','partial','canceled','failed','unknown'];
export function shortDate(value: string) { const d=new Date(value); return Number.isNaN(d.valueOf()) ? 'Data indisponível' : d.toLocaleString('pt-BR',{day:'2-digit',month:'short',hour:'2-digit',minute:'2-digit'}); }
export function errorMessage(error: unknown): string { const message=error instanceof Error ? error.message : typeof error === 'string' ? error : 'Não foi possível concluir a ação. Tente novamente.';return /^(data_|credential_reference_)[a-z_]+$/.test(message)?dataReason(message):/^(work_|capture_)[a-z_]+$/.test(message)?workReason(message):message; }
