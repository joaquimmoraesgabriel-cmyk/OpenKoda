import { invoke, isTauri } from '@tauri-apps/api/core';
import { Payload } from './types';
import { createPreviewTransport } from './preview';
import { validEvidenceView } from './data-policy';

export const native = isTauri();
let preview: ReturnType<typeof createPreviewTransport> | undefined;
export async function request<T>(action:string,payload:Payload={}):Promise<T> {
  if(native){
    const result=await invoke<T>('request',{request:{action,payload}});
    if(action==='snapshot'){
      const view=result as unknown as import('./types').Snapshot;
      if(view?.schema!=='openkoda.snapshot-view.v1'||view.projection?.schema!=='openkoda.projection.v1'||!Array.isArray(view.assessments)||view.assessments.some(a=>!Array.isArray(a.evidence)||!a.evidence.every(validEvidenceView)))throw new Error('A resposta do núcleo não corresponde à projeção de dados desta versão. Atualize o aplicativo antes de continuar.');
      if(view.workPlanBindings!==undefined&&(!Array.isArray(view.workPlanBindings)||view.workPlanBindings.some(binding=>!binding||['executionPlanId','jobId','planId','revisionId'].some(key=>typeof binding[key as keyof typeof binding]!=='string'||!binding[key as keyof typeof binding]))))throw new Error('Os vínculos de operações retornados pelo núcleo não são reconhecidos. Nenhuma aprovação será oferecida.');
    }
    return result;
  }
  preview ??= createPreviewTransport(window.localStorage);
  return preview<T>(action,payload);
}
