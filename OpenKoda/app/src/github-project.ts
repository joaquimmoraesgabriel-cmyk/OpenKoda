import { request } from './api';

export type GitHubImport={id:string;status:'downloading'|'ready'|'failed'|'cancelled'|'interrupted';path?:string;name:string;url:string;revision?:string;replayed:boolean;message:string};
export function githubProjectName(value:string) {
  try{const url=new URL(value);return url.hostname==='github.com'?url.pathname.split('/').filter(Boolean)[1]?.replace(/\.git$/i,'')??'':'';}catch{return '';}
}
export function readGitHubImport(value:unknown):GitHubImport {
  const result=value as GitHubImport|null;
  if(!result||typeof result.id!=='string'||!['downloading','ready','failed','cancelled','interrupted'].includes(result.status)||
    typeof result.name!=='string'||typeof result.url!=='string'||typeof result.message!=='string'||typeof result.replayed!=='boolean'||
    result.status==='ready'&&(!result.path||typeof result.path!=='string'||!result.revision||typeof result.revision!=='string'))throw new Error('Não foi possível confirmar o download do repositório.');
  return result;
}
export const importGitHubRepository=async(url:string,idempotencyKey:string)=>readGitHubImport(await request('github.repository.import',{url:url.trim(),idempotencyKey}));
async function existingImport(action:string,id:string){
  const receipt=readGitHubImport(await request(action,{id}));
  if(receipt.id!==id)throw new Error('O recibo não corresponde ao download deste projeto.');
  return receipt;
}
export const getGitHubImport=(id:string)=>existingImport('github.repository.get',id);
export const cancelGitHubImport=(id:string)=>existingImport('github.repository.cancel',id);
