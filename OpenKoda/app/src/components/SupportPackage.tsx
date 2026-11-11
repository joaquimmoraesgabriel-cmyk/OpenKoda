import { useEffect, useRef, useState } from 'react';
import { native, request } from '../api';
import { errorMessage, shortDate } from '../types';
import { KeyValue } from './shared';

interface SupportFile { fileName: string; relativePath: string; sha256: string; bytes: number }
interface SupportReview extends SupportFile { reviewId: string; expiresAt: string; preview: string }
const uuid = /^[a-f0-9]{8}(-[a-f0-9]{4}){3}-[a-f0-9]{12}$/;

async function canReview(review: SupportReview) {
  if (!review || !uuid.test(review.reviewId) || review.fileName !== 'suporte-'+review.reviewId+'.json' ||
    review.relativePath !== 'exports/'+review.fileName || !/^[a-f0-9]{64}$/.test(review.sha256) || typeof review.preview !== 'string' ||
    !Number.isFinite(Date.parse(review.expiresAt)) || Date.parse(review.expiresAt) <= Date.now()) return false;
  const bytes = new TextEncoder().encode(review.preview);
  if (bytes.length !== review.bytes) return false;
  const hash = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), byte => byte.toString(16).padStart(2,'0')).join('');
  if(hash !== review.sha256) return false;
  try {
    const content = JSON.parse(review.preview);
    return content?.formatVersion===1 && content.telemetry==='disabled' && content.remoteTransmission===false;
  } catch { return false; }
}

export default function SupportPackage() {
  const [review,setReview] = useState<SupportReview>();
  const [approved,setApproved] = useState(false);
  const [receipt,setReceipt] = useState<SupportFile>();
  const [error,setError] = useState('');
  const [notice,setNotice] = useState('');
  const [pending,setPending] = useState(false);
  const active = useRef(false);
  const reviewElement = useRef<HTMLElement>(null);
  const generation = useRef(0);
  useEffect(() => () => { generation.current++; }, []);
  useEffect(() => {
    if (!review) return;
    reviewElement.current?.focus();
    const expiry=setTimeout(() => {
      setReview(undefined); setApproved(false); setNotice('A revisão expirou. Prepare uma nova prévia.');
    },Math.max(0,Date.parse(review.expiresAt)-Date.now()));
    return () => clearTimeout(expiry);
  },[review]);
  async function run(operation: (current: () => boolean) => Promise<void>) {
    if (active.current) return;
    const epoch=generation.current;
    const current=() => epoch===generation.current;
    active.current=true;setPending(true);setError('');setNotice('');
    try { await operation(current); }
    catch (e) { if(current())setError(errorMessage(e)); }
    finally { active.current=false;if(current())setPending(false); }
  }
  function prepare() {
    return run(async current => {
      setReview(undefined);setApproved(false);setReceipt(undefined);
      const next = await request<SupportReview>('support.review');
      if (!await canReview(next)) throw new Error('Não foi possível conferir a integridade, o destino ou a validade da prévia. Prepare outra revisão.');
      if (current()) setReview(next);
    });
  }
  function write() {
    if (!approved || !review) return;
    return run(async current => {
      setApproved(false);setReview(undefined);
      if (Date.parse(review.expiresAt)<=Date.now()) throw new Error('A revisão expirou. Prepare uma nova prévia.');
      const result=await request<SupportFile>('support.export',{reviewId:review.reviewId,confirmation:true});
      if (!result || result.fileName!==review.fileName || result.relativePath!==review.relativePath ||
          result.sha256!==review.sha256 || result.bytes!==review.bytes) throw new Error('O recibo não confirma os bytes e o destino revisados. Confira a pasta de exportações do perfil ativo.');
      if (current()) { setReceipt(result);setNotice('Pacote de suporte salvo na pasta de exportações do perfil ativo.'); }
    });
  }
  return <section className="content-section" aria-label="Pacote de suporte local">
    <h3>Pacote de suporte</h3>
    <KeyValue label="Telemetria">Desativada</KeyValue>
    <p>O OpenKoda não envia diagnósticos automaticamente. O pacote local contém versões, plataforma e contagens de arquivos de banco de dados da instalação.</p>
    {native ? <button type="button" className="button" disabled={pending} onClick={()=>void prepare()}>Preparar prévia do pacote de suporte</button>
      : <p className="muted">Abra o aplicativo desktop para preparar o pacote desta instalação.</p>}
    {review && <section ref={reviewElement} tabIndex={-1} className="review-sheet" aria-label="Revisão do pacote de suporte">
      <h3>Conteúdo do arquivo JSON</h3><pre>{review.preview}</pre>
      <KeyValue label="Destino no perfil ativo"><span className="mono break-word">{review.relativePath}</span></KeyValue>
      <KeyValue label="Tamanho">{review.bytes.toLocaleString('pt-BR')} bytes</KeyValue>
      <KeyValue label="SHA-256"><span className="mono break-word">{review.sha256}</span></KeyValue>
      <KeyValue label="Validade">{shortDate(review.expiresAt)}</KeyValue>
      <label className="check-field"><input type="checkbox" checked={approved} disabled={pending} onChange={event=>setApproved(event.target.checked)}/>
        <span>Revisei os dados e autorizo salvar este arquivo no destino indicado.</span></label>
      <div className="inline-actions">
        <button type="button" className="button primary" disabled={pending||!approved} onClick={()=>void write()}>Salvar pacote de suporte</button>
        <button type="button" className="button" disabled={pending} onClick={()=>{setReview(undefined);setApproved(false);setNotice('Revisão descartada.');}}>Descartar prévia</button>
      </div>
    </section>}
    {error && <p role="alert">{error}</p>}
    <p role="status">{notice}</p>
    {receipt && <div><KeyValue label="Arquivo salvo no perfil ativo"><span className="mono break-word">{receipt.relativePath}</span></KeyValue>
      <KeyValue label="SHA-256 do arquivo"><span className="mono break-word">{receipt.sha256}</span></KeyValue></div>}
  </section>;
}
