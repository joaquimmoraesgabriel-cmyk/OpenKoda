import { CreditCard, LockKeyhole } from 'lucide-react';
import { useState } from 'react';
import './motion-controls.css';

// Visual preparation only. Actual card entry belongs to the payment provider's
// hosted/tokenized fields when that integration is configured, never this app.
export function CardFieldPreview() {
  const [expanded,setExpanded]=useState(false);
  return <section className="motion-card-field" aria-label="Prévia do pagamento por cartão"><button type="button" className="motion-card-trigger" aria-expanded={expanded} onClick={()=>setExpanded(value=>!value)}><CreditCard size={18}/><span>Cartão de crédito<small>Integração de pagamento pendente</small></span><span>{expanded?'Fechar':'Ver campos'}</span></button>{expanded&&<div className="motion-card-details"><p>Estes campos ilustram o futuro formulário seguro. Não insira dados de cartão nesta prévia.</p><fieldset disabled><label>Número do cartão<input placeholder="•••• •••• •••• ••••" readOnly/></label><div><label>Validade<input placeholder="MM/AA" readOnly/></label><label>Código de segurança<input placeholder="•••" readOnly/></label></div></fieldset><small><LockKeyhole size={14}/>Nenhum pagamento está habilitado.</small></div>}</section>;
}
