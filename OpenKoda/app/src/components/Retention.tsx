import { useEffect, useRef, useState } from 'react';
import { native, request } from '../api';
import { Action, errorMessage, shortDate } from '../types';
import { Field, KeyValue, Notice } from './shared';

type Category='backup'|'report'|'markdown'|'capture'|'kali_capture'|'kali_capture_metadata';
interface ArtifactFile {name:string;bytes:number;sha256:string;identity:unknown}
interface MetadataItem {
  reference:{captureId:string;assessmentId:string;runId:string;environmentId:string;ownerProfileFingerprint:string;manifestDigest:string;preservedBytes:number;availability:string};
  purgeControlRunId:string;purgeReceiptId:string;purgeProofSha256?:string|null;
  state:'uninspected'|'remaining'|'retired'|'unknown'|'foreign_profile';
}
interface MetadataPreview {
  item:MetadataItem;inspectionDigest:string;stateDigest:string;previousReceiptId?:string|null;previousReceiptDigest?:string|null;
  observation:{metadataBytes:number;slotPresent:boolean;entries:{name:string;bytes:number;sha256:string;identity:{device:string;inode:string}}[];pendingProducerWrite:{identity:{device:string;inode:string};bytes:number;sha256:string;attemptId:string;complete:boolean}|null};
}
interface Artifact {category:Category;id:string;label:string;bytes:number;fileCount:number;files:ArtifactFile[];identity:unknown;capture?:{assessmentId:string;manifestDigest:string;purgedManifestDigest:string;generation:number;preservedSha256:string;chunkCount:number};kaliCapture?:{reference:{captureId:string;assessmentId:string;runId:string;environmentId:string;ownerProfileFingerprint:string;manifestDigest:string;preservedBytes:number;availability:string};inspectionDigest:string;previousReceiptId?:string;previousReceiptDigest?:string};kaliMetadataItem?:MetadataItem;kaliMetadata?:MetadataPreview}
interface RetentionPlan {
  id:string;digest:string;status:'prepared'|'applying'|'applied'|'rejected'|'failed'|'partial'|'unknown'|'revoked';
  createdAt:string;expiresAt:string;items:Artifact[];totalBytes:number;totalFiles:number;confirmationPhrase:string;
  limits:string[];progress:{category:Category;id:string;name:string;status:'deleting'|'deleted'|'present'|'absent'|'replaced'|'unknown';sha256:string}[];error:string|null;
}
interface Inventory {items:Artifact[];journals:RetentionPlan[];hasUnresolved:boolean;blocked:boolean;limits:string[];projectPurgeSupported:false;automaticPurge:false;retentionDays:number;rejected?:{category:Category;id:string;reason:string}[];ignoredEntries?:number}
const categoryLabels:Record<Category,string>={backup:'Backup',report:'Pacote de relatório',markdown:'Relatório Markdown',capture:'Captura local de operação',kali_capture:'Captura no Kali',kali_capture_metadata:'Metadados de captura no Kali'};
const statusLabels:Record<RetentionPlan['status'],string>={prepared:'Preparado para revisão',applying:'Exclusão em andamento',applied:'Exclusão concluída',rejected:'Recusado',failed:'Falhou',partial:'Resultado parcial',unknown:'Resultado desconhecido',revoked:'Revogado'};
const confirmationPhrase='EXCLUIR PERMANENTEMENTE';
function itemKey(item:Artifact){return item.category+':'+item.id;}
function MetadataIdentity({preview}:{preview:MetadataPreview}){
  const {item,observation}=preview;
  return <>
    <KeyValue label="Ambiente"><span className="mono break-word">{item.reference.environmentId}</span></KeyValue>
    <KeyValue label="Produtor"><span className="mono break-word">{item.reference.runId}</span></KeyValue>
    <KeyValue label="Captura"><span className="mono break-word">{item.reference.captureId}</span></KeyValue>
    <KeyValue label="SHA-256 do manifesto Linux"><span className="mono break-word">{item.reference.manifestDigest}</span></KeyValue>
    <KeyValue label="Recibo da remoção dos bytes"><span className="mono break-word">{item.purgeReceiptId}</span></KeyValue>
    <KeyValue label="SHA-256 da prova de remoção"><span className="mono break-word">{item.purgeProofSha256}</span></KeyValue>
    <KeyValue label="SHA-256 da inspeção"><span className="mono break-word">{preview.inspectionDigest}</span></KeyValue>
    <KeyValue label="Capacidade">{observation.slotPresent?'Pode liberar 1 slot':'Nenhum slot adicional identificado nesta inspeção'}</KeyValue>
    <p>Consolida a prova no histórico do produtor e remove somente os metadados terminais desta captura. Não remove novamente os bytes nem apaga cópias externas. A inspeção não repete a consolidação.</p>
    {preview.previousReceiptId&&<><Notice tone="warning">Metadados restantes de uma tentativa anterior. Este plano exige nova confirmação para concluir somente o restante revisado.</Notice><KeyValue label="Tentativa anterior de consolidação"><span className="mono break-word">{preview.previousReceiptId}</span></KeyValue><KeyValue label="SHA-256 do compromisso anterior"><span className="mono break-word">{preview.previousReceiptDigest}</span></KeyValue></>}
    <ul aria-label="Metadados selecionados">{observation.entries.map(entry=><li key={entry.name}><strong>{entry.name}</strong> · {entry.bytes.toLocaleString('pt-BR')} bytes<div className="mono break-word">SHA-256: {entry.sha256}</div></li>)}</ul>
    {observation.pendingProducerWrite&&<section aria-label="Escrita interrompida do registro do produtor">
      <h4>Escrita interrompida do registro do produtor</h4>
      <p>O núcleo reconheceu este arquivo temporário de uma tentativa anterior. Este novo plano também revisa sua remoção antes de consolidar a prova. Nenhum arquivo desconhecido será removido.</p>
      <KeyValue label="Arquivo temporário"><span className="mono break-word">{item.reference.runId}.json.next</span></KeyValue>
      <KeyValue label="Tentativa da escrita interrompida"><span className="mono break-word">{observation.pendingProducerWrite.attemptId}</span></KeyValue>
      <KeyValue label="Bytes do temporário">{observation.pendingProducerWrite.bytes.toLocaleString('pt-BR')} bytes</KeyValue>
      <KeyValue label="SHA-256 do temporário"><span className="mono break-word">{observation.pendingProducerWrite.sha256}</span></KeyValue>
      <KeyValue label="Gravação observada">{observation.pendingProducerWrite.complete?'Completa, ainda sem consolidação':'Trecho incompleto reconhecido pelo núcleo'}</KeyValue>
    </section>}
    <p className="muted">O histórico do produtor continua armazenado. Liberar um slot não significa recuperar todo o armazenamento do Kali.</p>
  </>;
}
function MetadataResult({item,plan}:{item:Artifact;plan:RetentionPlan}){
  const progress=plan.progress.filter(entry=>entry.category==='kali_capture_metadata'&&entry.id===item.id).at(-1);
  if(!progress)return null;
  const message=progress.status==='deleted'||progress.status==='absent'?(item.kaliMetadata?.observation.slotPresent?'Metadados consolidados; slot liberado.':'Metadados consolidados; nenhum slot adicional foi liberado.') :progress.status==='present'?'Metadados restantes — prepare outro plano.':'Resultado da consolidação ainda desconhecido; consulte o histórico.';
  return <p role="status">{message}</p>;
}

export default function Retention({action,busy}:{action:Action;busy:boolean}) {
  const [inventory,setInventory]=useState<Inventory>();const [selected,setSelected]=useState<string[]>([]);
  const [review,setReview]=useState<RetentionPlan>();const [approved,setApproved]=useState(false);const [confirmation,setConfirmation]=useState('');
  const [pending,setPending]=useState(native);const [error,setError]=useState('');const [attempted,setAttempted]=useState<string[]>([]);
  const [inventoryError,setInventoryError]=useState('');
  const prepareButton=useRef<HTMLButtonElement>(null);const refreshButton=useRef<HTMLButtonElement>(null);
  function clearApproval(){setApproved(false);setConfirmation('');}
  function closeReview(){
    setReview(undefined);clearApproval();
    if(prepareButton.current&&!prepareButton.current.disabled)prepareButton.current.focus();
    else refreshButton.current?.focus();
  }
  async function refresh(){
    const next=await request<Inventory>('retention.inventory');setInventory(next);setInventoryError('');
    setSelected(current=>current.filter(key=>next.items.some(item=>itemKey(item)===key)));
    setReview(current=>current?next.journals.find(plan=>plan.id===current.id)??current:current);
  }
  useEffect(()=>{
    if(!native)return;
    let disposed=false;
    const delays=[200,500,1000];
    let retry=0;
    async function initialRead(){
      try{await refresh();}
      catch(e){
        if(disposed)return;
        const reason=errorMessage(e);
        if(reason==='Retenção indisponível durante outra operação.'&&retry<delays.length){
          timer=window.setTimeout(initialRead,delays[retry++]);return;
        }
        setInventoryError(reason);
      }
      if(!disposed)setPending(false);
    }
    let timer=window.setTimeout(initialRead,0);
    return ()=>{disposed=true;window.clearTimeout(timer);};
  },[]);
  async function prepare(){
    setPending(true);setError('');setReview(undefined);clearApproval();
    try{
      const items=(inventory?.items??[]).filter(item=>selected.includes(itemKey(item))).map(({category,id})=>({category,id}));
      const plan=await action<RetentionPlan>('retention.plan',{items});setReview(plan);await refresh();
    }catch(e){setError(errorMessage(e));await refresh().catch(()=>{});}finally{setPending(false);}
  }
  async function apply(){
    if(!review)return;
    const decision={id:review.id,digest:review.digest,confirmation};
    setPending(true);setError('');setAttempted(ids=>[...ids,review.id]);clearApproval();
    try{setReview(await action<RetentionPlan>('retention.apply',decision));setSelected([]);await refresh();}
    catch(e){setError(errorMessage(e));await refresh().catch(()=>{});}finally{setPending(false);}
  }
  async function reconcile(plan:RetentionPlan){
    setPending(true);setError('');clearApproval();
    try{setReview(await action<RetentionPlan>('retention.reconcile',{id:plan.id}));await refresh();}
    catch(e){setError(errorMessage(e));await refresh().catch(()=>{});}finally{setPending(false);}
  }
  async function consult(){setPending(true);try{await refresh();}catch(e){setInventoryError(errorMessage(e));}finally{setPending(false);}}
  function toggle(item:Artifact,checked:boolean){setSelected(keys=>checked?[...keys,itemKey(item)]:keys.filter(key=>key!==itemKey(item)));setReview(undefined);clearApproval();}
  const disabled=busy||pending;const blocked=!!inventory&&(inventory.blocked||inventory.hasUnresolved);
  const expired=review?Date.parse(review.expiresAt)<=Date.now():false;
  const canApply=!!review&&review.status==='prepared'&&!attempted.includes(review.id)&&!blocked&&!expired&&approved&&confirmation===confirmationPhrase&&review.confirmationPhrase===confirmationPhrase;
  return <section className="content-section retention"><h2>Revisão de retenção e exclusão</h2>
    <p className="muted">Selecione as cópias de backup, os relatórios, as capturas ou seus metadados terminais que deseja excluir permanentemente. O prazo salvo é apenas uma referência; nenhuma exclusão é agendada automaticamente.</p>
    {!native?<Notice>A revisão e a exclusão de arquivos geridos estão disponíveis no aplicativo desktop.</Notice>:<>
      {error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}
      {inventoryError&&<div role="alert"><Notice tone="warning">{inventoryError}</Notice></div>}
      <Notice>Ao excluir uma captura, seu conteúdo deixa de estar disponível. O manifesto, a origem e os hashes permanecem no histórico. A cópia local e a cópia no Kali são itens separados; selecione cada uma que deseja remover. Projetos, avaliações e demais evidências são preservados.</Notice>
      {!!inventory?.items.some(item=>item.category==='kali_capture_metadata')&&<Notice>Metadados de captura são uma seleção separada, disponível após remover os bytes no Kali. Sua consolidação exige outra revisão e preserva a prova no histórico do produtor.</Notice>}
      <button ref={refreshButton} type="button" className="button" disabled={disabled} onClick={()=>void consult()}>Atualizar arquivos e histórico</button>
      {inventory?.hasUnresolved&&<Notice tone="warning">Há uma exclusão em andamento ou com resultado desconhecido. Novos planos estão bloqueados. Consulte o histórico e reconcilie o resultado quando essa opção estiver disponível.</Notice>}
      {inventory?.blocked&&!inventory.hasUnresolved&&<Notice tone="warning">Encerre ou reconcilie avaliações ativas ou com estado desconhecido antes de preparar a exclusão de arquivos geridos.</Notice>}
      {inventory&&<><KeyValue label="Prazo de referência">{inventory.retentionDays} dias · seleção manual</KeyValue>
        {inventory.items.length?<fieldset className="context-selection"><legend>Arquivos geridos disponíveis</legend>{inventory.items.map(item=><label className="check-field" key={itemKey(item)}><input type="checkbox" checked={selected.includes(itemKey(item))} disabled={disabled||blocked||selected.length>=20&&!selected.includes(itemKey(item))} onChange={e=>toggle(item,e.target.checked)}/><span><strong className="break-word">{item.label}</strong><br/><span className="muted">{categoryLabels[item.category]} · {item.category==='kali_capture_metadata'?'Bytes da captura já removidos; metadados aguardam consolidação · Tamanho a conferir na preparação':`${item.kaliCapture?'cópia no ambiente Linux':item.capture?`${item.capture.chunkCount} trecho(s) de captura`:`${item.fileCount} arquivo(s)`} · ${item.bytes.toLocaleString('pt-BR')} bytes`}</span></span></label>)}</fieldset>:<p className="muted spaced">Nenhum backup, relatório ou captura disponível para exclusão.</p>}
        <div className="inline-actions"><span className="muted">{selected.length} de 20 itens selecionados</span><button ref={prepareButton} type="button" className="button" disabled={disabled||blocked||!selected.length} onClick={()=>void prepare()}>Revisar exclusão selecionada</button></div>
        {!!inventory.rejected?.length&&<details className="plan-details"><summary>Itens recusados pelo inventário</summary><ul>{inventory.rejected.map(item=><li key={item.category+item.id}><span className="mono break-word">{categoryLabels[item.category]} · {item.id}</span><p>{item.reason}</p></li>)}</ul></details>}
        {!!inventory.ignoredEntries&&<p className="muted spaced">{inventory.ignoredEntries} entrada(s) fora dos nomes geridos foram preservadas e não podem ser selecionadas.</p>}
        {!!inventory.limits.length&&<details className="plan-details"><summary>Limites da exclusão</summary><ul>{inventory.limits.map((limit,index)=><li key={index}>{limit}</li>)}</ul></details>}
      </>}
      {review&&<div className="maintenance-review" aria-label="Plano de exclusão"><h3>Revisão da exclusão permanente</h3>
        <KeyValue label="Estado do plano">{statusLabels[review.status]}</KeyValue><KeyValue label="Total revisado">{review.items.length} item(ns) · {review.totalFiles} arquivo(s) · {review.totalBytes.toLocaleString('pt-BR')} bytes</KeyValue><KeyValue label="Validade">{shortDate(review.expiresAt)}{expired?' · Expirado':''}</KeyValue><KeyValue label="SHA-256 do plano"><span className="mono break-word">{review.digest}</span></KeyValue>
        {review.items.map(item=><div className="execution-entry" key={itemKey(item)}><strong>{item.label}</strong><p>{categoryLabels[item.category]} · {item.bytes.toLocaleString('pt-BR')} bytes</p><details className="plan-details"><summary>{item.kaliMetadata?'Identidade dos metadados no Kali':item.kaliCapture?'Identidade da captura no Kali':item.capture?'Identidade da captura':`Arquivos de ${item.label}`}</summary>{item.kaliMetadata?<MetadataIdentity preview={item.kaliMetadata}/>:item.kaliCapture?<><KeyValue label="Ambiente"><span className="mono break-word">{item.kaliCapture.reference.environmentId}</span></KeyValue><KeyValue label="SHA-256 do manifesto Linux"><span className="mono break-word">{item.kaliCapture.reference.manifestDigest}</span></KeyValue><KeyValue label="SHA-256 da inspeção"><span className="mono break-word">{item.kaliCapture.inspectionDigest}</span></KeyValue><p>A cópia selecionada no Kali será removida. A cópia local exige seleção própria.</p>{item.kaliCapture.previousReceiptId&&<><Notice tone="warning">Uma inspeção confirmou que restaram bytes da exclusão anterior. Este plano exige uma nova confirmação e remove somente o restante da mesma captura.</Notice><KeyValue label="Recibo da exclusão parcial"><span className="mono break-word">{item.kaliCapture.previousReceiptId}</span></KeyValue><KeyValue label="SHA-256 do recibo anterior"><span className="mono break-word">{item.kaliCapture.previousReceiptDigest}</span></KeyValue></>}</>:item.capture?<><KeyValue label="SHA-256 dos bytes preservados"><span className="mono break-word">{item.capture.preservedSha256}</span></KeyValue><KeyValue label="Trechos preservados">{item.capture.chunkCount}</KeyValue><p>Os bytes e as janelas de leitura serão removidos. A origem e os hashes continuarão no histórico.</p></>:<pre>{JSON.stringify(item.files,null,2)}</pre>}</details>{item.kaliMetadata&&<MetadataResult item={item} plan={review}/>}</div>)}
        <Notice tone="warning">A exclusão é permanente. {review.limits.join(' ')}</Notice>
        {review.error&&<p role="alert">{review.error}</p>}
        {review.status==='prepared'&&!attempted.includes(review.id)&&<>
          <label className="check-field"><input type="checkbox" checked={approved} onChange={e=>setApproved(e.target.checked)} disabled={disabled||blocked||expired}/><span>Revisei os arquivos e aceito a exclusão permanente dos itens deste plano.</span></label>
          <Field label="Frase de confirmação" hint={`Digite exatamente: ${confirmationPhrase}`}><input value={confirmation} onChange={e=>setConfirmation(e.target.value)} disabled={disabled||blocked||expired} autoComplete="off" spellCheck={false}/></Field>
          <button type="button" className="button danger" disabled={disabled||!canApply} onClick={()=>void apply()}>Excluir itens revisados permanentemente</button>
        </>}
        {attempted.includes(review.id)&&review.status==='prepared'&&<Notice tone="warning">A solicitação já foi enviada. Consulte o histórico antes de preparar outro plano; esta tentativa não será repetida.</Notice>}
        {(review.status==='partial'||review.status==='unknown')&&<Notice tone="warning">{review.status==='partial'?'Somente parte do resultado foi confirmada.':'O efeito da tentativa ainda não foi confirmado.'} Nenhuma exclusão será repetida automaticamente.</Notice>}
        {review.status==='unknown'&&<button type="button" className="button" disabled={disabled} onClick={()=>void reconcile(review)}>Inspecionar resultado sem excluir</button>}
        {!!review.progress.length&&<details className="plan-details"><summary>Resultado por arquivo</summary><pre>{JSON.stringify(review.progress,null,2)}</pre></details>}
        <div className="inline-actions"><button type="button" className="button" disabled={disabled} onClick={closeReview}>Fechar revisão da exclusão</button></div>
      </div>}
      {!!inventory?.journals.length&&<details className="plan-details spaced"><summary>Histórico de exclusões</summary><div className="backup-list">{inventory.journals.map(plan=><article className="backup-row" key={plan.id}><div><strong>{statusLabels[plan.status]}</strong><small>{shortDate(plan.createdAt)} · {plan.totalFiles} arquivo(s)</small></div><button type="button" className="button" disabled={disabled} onClick={()=>{setReview(plan);clearApproval();}}>Consultar plano</button></article>)}</div></details>}
    </>}
  </section>;
}
