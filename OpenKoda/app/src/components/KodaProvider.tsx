import type { AccountStatus } from '../commercial';
import { useDesktopSession } from '../desktop-session';

export const kodaAccountNavigation = 'openkoda:account-navigation';
export type KodaAccountSection = 'Conta e segurança'|'Plano e assinatura';
export function openKodaAccount(section:KodaAccountSection){window.dispatchEvent(new CustomEvent(kodaAccountNavigation,{detail:{section}}));}

export function kodaAccess(session:AccountStatus & {loading?:boolean;error?:string}){
  const base={allowed:false,catalog:false,section:'Plano e assinatura' as KodaAccountSection};
  if(session.loading)return {...base,state:'checking',label:'Verificando conta',message:'Aguarde a verificação da conta para consultar o acesso à Koda.',cta:''};
  if(session.state==='offline'||session.error||session.user&&!session.account)return {...base,state:'offline',label:'Conta não verificada',message:'Conecte-se para verificar seu plano. O acesso à IA da Koda não é presumido sem conexão.',cta:'Verificar conta'};
  if(!session.user||session.state!=='authenticated')return {...base,state:'signedOut',label:'Entre na sua conta',message:'A Koda oferece modelos de IA pela assinatura, sem configurar uma chave de API. Entre para consultar seu plano.',cta:'Entrar na Koda',section:'Conta e segurança' as KodaAccountSection};
  const account=session.account!;
  const paid=account.subscription?.status==='active'&&!!account.plan;
  if(account.entitlements?.hostedInference===true&&account.controls?.inferenceEnabled!==false)return {...base,allowed:true,catalog:true,state:'ready',label:'Disponível pelo plano',message:'Modelos de IA pela Koda, conforme o catálogo disponível e a cota do seu plano.',cta:'Ver plano e cota'};
  if(paid)return {...base,catalog:true,state:'unavailable',label:'Serviço indisponível',message:'Seu plano está ativo, mas o serviço de IA da Koda não está disponível agora. Consulte sua conta antes de enviar.',cta:'Ver plano e cota'};
  if(account.simulation?.status==='active')return {...base,catalog:true,state:'simulation',label:'Simulação interna ativa',message:'Você pode consultar o catálogo. A simulação libera skills e raciocínio com sua chave própria; não concede créditos de IA da Koda.',cta:'Ver simulação e plano'};
  return {...base,catalog:true,state:'free',label:'Disponível com assinatura',message:'Assine para usar modelos de IA pela Koda, conforme o catálogo disponível, sem configurar sua chave de API.',cta:'Conhecer planos'};
}

export function KodaAccessCard({compact=false,onNavigate}:{compact?:boolean;onNavigate?:()=>void}){
  const session=useDesktopSession(),access=kodaAccess(session);
  return <section className={'koda-provider-access'+(compact?' compact':'')} aria-label="Acesso à Koda">
    <div><h3>Koda</h3><span className="provider-state" data-present={access.allowed}>{access.label}</span></div>
    <p>{access.message}</p>
    {access.cta&&<button type="button" className="button" onClick={()=>{
      if(access.state==='offline'){void session.refresh();return;}
      onNavigate?.();openKodaAccount(access.section);
    }}>{access.cta}</button>}
  </section>;
}
