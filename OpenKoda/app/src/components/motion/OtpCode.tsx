import { useId, useLayoutEffect, useRef } from 'react';
import { Check } from 'lucide-react';
import { springAnimate } from './spring';
import './motion-controls.css';

export const normalizeOtp=(value:string)=>value.replace(/\D/g,'').slice(0,6);
function Digit({value}:{value:string}) {
  const node=useRef<HTMLSpanElement>(null);
  useLayoutEffect(()=>{if(!node.current||!value)return;const a=springAnimate(node.current,v=>({transform:`translateY(${(1-v)*-14}px)`,opacity:Math.min(1,Math.max(0,v))}),[520,16]);return()=>a?.cancel();},[value]);
  return <span ref={node}>{value}</span>;
}
// Verification status must come from the authentication operation. Six digits
// merely finish entry; they do not grant access or produce a success state.
export function OtpCode({value,onChange,status='idle',disabled=false}:{value:string;onChange:(value:string)=>void;status?:'idle'|'verifying'|'verified'|'error';disabled?:boolean}) {
  const id=useId(),code=normalizeOtp(value);
  return <div className="motion-otp" data-status={status}><label htmlFor={id}>Código de verificação</label><div className="motion-otp-entry"><div className="motion-otp-boxes" aria-hidden="true">{Array.from({length:6},(_,i)=><span className="motion-otp-box" data-active={i===Math.min(code.length,5)} key={i}><Digit value={code[i]??''}/></span>)}</div><input id={id} type="text" inputMode="numeric" autoComplete="one-time-code" value={code} maxLength={6} disabled={disabled||status==='verifying'||status==='verified'} aria-invalid={status==='error'} aria-describedby={id+'-status'} onChange={event=>onChange(normalizeOtp(event.target.value))}/></div><span id={id+'-status'} className="motion-otp-status" role="status">{status==='verified'?<><Check size={16}/>Código confirmado</>:status==='verifying'?'Conferindo código…':status==='error'?'Não foi possível verificar. Confira o código e tente de novo.':'Digite os seis números recebidos por e-mail.'}</span></div>;
}
