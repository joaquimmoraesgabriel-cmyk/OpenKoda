import BackupReview from './BackupReview';
import { useEffect, useState } from 'react';
import { native, request } from '../api';
import { Action, Project, errorMessage, shortDate } from '../types';
import { Notice } from './shared';
import RecoveryActivation from './RecoveryActivation';

interface Backup {id:string;createdAt:string;bytes:number;verification:string}
export default function Recovery({projects,action,busy}:{projects:Project[];action:Action;busy:boolean}) {
  const [backups,setBackups]=useState<Backup[]>([]);const [verified,setVerified]=useState<string[]>([]);
  const [result,setResult]=useState('');const [error,setError]=useState('');const [pending,setPending]=useState(false);
  const [recoveryId,setRecoveryId]=useState<string>();
  async function refresh(){if(native)setBackups((await request<{backups:Backup[]}>('backup.list')).backups);}
  useEffect(()=>{void refresh().catch(e=>setError(errorMessage(e)));},[]);
  async function perform(name:string,id?:string){setPending(true);setResult('');setError('');if(name==='backup.restore')setRecoveryId(undefined);try{const response=await action<{id?:string;path?:string;message?:string}>(name,id?{id}:{});if(name==='backup.verify'&&id)setVerified(ids=>[...ids,id]);if(name==='backup.restore')setRecoveryId(response.id);await refresh();setResult(name==='backup.restore'?`${response.message??'Restauração preparada em uma pasta separada; os dados atuais continuam ativos.'} ${response.path??''}`:name==='backup.verify'?'Backup verificado.':'Backup local criado.');}catch(e){setError(errorMessage(e));}finally{setPending(false);}}
  return <section className="content-section"><h2>Backup e recuperação</h2><p className="muted">Crie uma cópia consistente dos dados locais. As chaves de API não fazem parte do backup e precisam ser cadastradas novamente após a restauração. O orçamento e os débitos mínimos da instalação não são substituídos pelo backup: restaurar ou trocar um perfil não renova o saldo.</p>{native?<><BackupReview projects={projects} action={action} busy={busy||pending} onSaved={refresh}/><div className="backup-list">{backups.map(backup=><article key={backup.id} className="backup-row"><div><strong>{shortDate(backup.createdAt)}</strong><small>{backup.bytes.toLocaleString('pt-BR')} bytes · {verified.includes(backup.id)?'Verificado':'Verificação necessária'}</small></div><button className="button" type="button" disabled={busy||pending} onClick={()=>void perform('backup.verify',backup.id)}>Verificar backup</button><button className="button" type="button" disabled={busy||pending||!verified.includes(backup.id)} onClick={()=>void perform('backup.restore',backup.id)}>Preparar restauração</button></article>)}</div>{!backups.length&&<p className="muted spaced">Nenhum backup local encontrado.</p>}<Notice>A restauração é preparada em uma pasta separada e não substitui os dados atuais. Revise a ativação abaixo; credenciais precisam ser cadastradas novamente e o ambiente Kali exige revalidação.</Notice><RecoveryActivation recoveryId={recoveryId} action={action} busy={busy||pending}/></>:<Notice>Backup e recuperação estão disponíveis no aplicativo desktop.</Notice>}{result&&<p role="status" className="break-word">{result}</p>}{error&&<Notice tone="warning">{error}</Notice>}</section>;
}
