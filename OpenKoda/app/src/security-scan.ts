import { request } from './api';
import type { AiSelection } from './ai-provider';
import type { Assessment } from './types';
import { prepareScanMetadata } from './project-chat';

export const defaultScanLimit = 5;
export const codeAnalysisProviderNotice = 'A análise de código usa OpenAI nesta versão. Os demais provedores continuam disponíveis nas conversas.';
export const analysisFileAccessNotice = 'Os comandos da análise podem ler outros arquivos acessíveis à sua conta Windows. Use apenas código em que você confia nesta versão local.';
// Product copy only. Original reports and engine receipts remain unchanged on disk.
export const analysisDisplayText=(text:string)=>text.replace(/Codex Security/gi,'OpenKoda');
const states = ['starting','running','cancelling','completed','failed','cancelled','interrupted'] as const;
export type ScanState = typeof states[number];
export const scanStateLabel:Record<ScanState,string> = {
  starting:'Iniciando',running:'Em análise',cancelling:'Cancelamento solicitado',completed:'Concluída',
  failed:'Falhou',cancelled:'Cancelada',interrupted:'Interrompida',
};
export const scanActive = (status:ScanState) => status === 'starting' || status === 'running' || status === 'cancelling';
export const supportsSecurityModel = (selection:AiSelection|null|undefined) =>
  selection?.provider === 'openai';
type ScanEvent = {at:string;stage:string;message:string};
type ScanFinding = {title:string;severity:string;description:string;location:string|null};
type Coverage = {scope:string[];excludedPaths:string[];additionalExclusions:number;findingsTotal:number;reportTruncated:boolean;summary?:string;limitations?:string[]};
export type ScanResult = {scanId:string|null;report:string;findings:ScanFinding[];coverage:Coverage;cost:{estimatedUsd:number|null}|null};
export type SecurityStatus = {available:boolean;hasApiKey:boolean;provider:string;model:string;reason?:string;requiresSensitiveConfirmation?:boolean};
export class SensitiveScanConfirmation extends Error {
  constructor(){super('A fonte desta avaliação está classificada como sensível. Revise o envio ao provedor selecionado.');}
}
export type SecurityScan = {
  id:string;assessmentId:string;status:ScanState;stage:string;message:string;events:ScanEvent[];
  createdAt:string;updatedAt:string;provider:string;model:string;maxCostUsd:number;result:ScanResult|null;error:string|null;
};

const object = (v:unknown):v is Record<string,unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const strings = (v:unknown) => Array.isArray(v) && v.every(item=>typeof item==='string');
const count = (v:unknown) => typeof v==='number' && Number.isSafeInteger(v) && v>=0;
type ScanReceipt = {id:string;assessmentId:string;status:ScanState;createdAt:string;updatedAt?:string;findingsTotal?:number};
export function projectScanReceipts(chats:Iterable<Assessment>):ScanReceipt[] {
  const records:ScanReceipt[]=[];
  for(const chat of chats)for(const event of chat.events){
    const item=(event as {securityScan?:unknown}).securityScan;
    if(!object(item)||typeof item.id!=='string'||!states.includes(item.status as ScanState))continue;
    records.push({id:item.id,assessmentId:chat.id,status:item.status as ScanState,
      createdAt:typeof item.createdAt==='string'?item.createdAt:event.at,
      updatedAt:typeof item.updatedAt==='string'?item.updatedAt:undefined,
      findingsTotal:count(item.findingsTotal)?item.findingsTotal as number:undefined});
  }
  return records;
}
function validResult(v:unknown):boolean {
  if(!object(v) || !(v.scanId===null||typeof v.scanId==='string') || typeof v.report!=='string' || !Array.isArray(v.findings))return false;
  const validFindings=v.findings.every(f=>object(f) && typeof f.title==='string' && typeof f.description==='string' &&
    typeof f.severity==='string' && (f.location===null || typeof f.location==='string'));
  const c=v.coverage;
  return validFindings && object(c) && strings(c.scope) && strings(c.excludedPaths) && count(c.additionalExclusions) &&
    count(c.findingsTotal) && typeof c.reportTruncated==='boolean' && (c.summary===undefined||typeof c.summary==='string') &&
    (c.limitations===undefined||strings(c.limitations)) && (v.cost===null || object(v.cost) &&
    (v.cost.estimatedUsd===null || typeof v.cost.estimatedUsd==='number' && Number.isFinite(v.cost.estimatedUsd)));
}
export function readScan(response:unknown, assessmentId:string):SecurityScan|null {
  if (object(response) && response.scan === null) return null;
  const s = object(response) ? response.scan : undefined;
  if (!object(s) || typeof s.id !== 'string' || s.assessmentId !== assessmentId ||
      !states.includes(s.status as ScanState) || typeof s.stage !== 'string' || typeof s.message !== 'string' ||
      typeof s.provider !== 'string' || typeof s.model !== 'string' || typeof s.maxCostUsd !== 'number' ||
      typeof s.createdAt !== 'string' || typeof s.updatedAt !== 'string' ||
      !Array.isArray(s.events) || !s.events.every(e => object(e) && typeof e.at === 'string' && typeof e.stage === 'string' && typeof e.message === 'string') ||
      !(s.error === null || typeof s.error === 'string') || !(s.result === null || validResult(s.result)) ||
      s.status==='completed' && s.result===null) {
    throw new Error('O aplicativo não recebeu um registro válido desta análise de segurança.');
  }
  return s as unknown as SecurityScan;
}

type StartScan = {projectId:string;assessmentId:string;selection:AiSelection;idempotencyKey:string;prompt:string;maxCostUsd:number;target?:string;acknowledgeSensitive?:boolean;signal?:AbortSignal};
export async function startSecurityScan({projectId,assessmentId,selection,idempotencyKey,prompt,maxCostUsd,target,acknowledgeSensitive,signal}:StartScan) {
  if(!supportsSecurityModel(selection))throw new Error(codeAnalysisProviderNotice);
  function requireActive(){if(signal?.aborted)throw new Error('Criação cancelada antes da análise. O projeto criado foi preservado.');}
  async function currentStatus(){
    requireActive();
    const status=await request<SecurityStatus>('security.status',{assessmentId});
    requireActive();
    if (status?.available!==true || status.hasApiKey!==true || status.provider!==selection.provider || status.model!==selection.model) {
      throw new Error(status?.reason || 'A análise está indisponível com o modelo selecionado. Confira o motor e a credencial nas configurações.');
    }
    return status;
  }
  let status=await currentStatus();
  if(await prepareScanMetadata(projectId,assessmentId,selection))status=await currentStatus();
  requireActive();
  if(status.requiresSensitiveConfirmation&&!acknowledgeSensitive)throw new SensitiveScanConfirmation();
  const response = await request('security.scan.start',{assessmentId,idempotencyKey,prompt:prompt.trim(),maxCostUsd,...(target?{target}:{}),
    ...(status.requiresSensitiveConfirmation?{acknowledgeSensitive:true}:{})});
  const scan = readScan(response,assessmentId);
  if (!scan) throw new Error('O motor não devolveu um registro para esta análise. Consulte a conversa antes de repetir.');
  return scan;
}
