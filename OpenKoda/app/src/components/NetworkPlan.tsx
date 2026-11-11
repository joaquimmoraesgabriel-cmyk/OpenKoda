import { ScopeRestriction, useScopeAccess } from './ScopeAccess';
import { useEffect, useState } from 'react';
import { native } from '../api';
import { parsePorts } from '../network';
import { Action, Assessment, errorMessage } from '../types';
import { Field, KeyValue, Notice } from './shared';

export default function NetworkPlan({assessment,action,busy,projectActive=true}:{assessment:Assessment;action:Action;busy:boolean;projectActive?:boolean}){
  const scope=useScopeAccess();
  const [ports,setPorts]=useState('');const [pending,setPending]=useState(false);const [error,setError]=useState('');
  const parsed=parsePorts(ports);const canPrepare=['draft','blocked','paused','partial','failed','canceled'].includes(assessment.status);
  useEffect(()=>{if(['partial','failed','canceled'].includes(assessment.status))setPorts('');},[assessment.status]);
  async function prepare(){
    if(parsed.error||!projectActive||!scope.allows('network.tcp-connect.v1'))return;
    setPending(true);setError('');
    try{await action('assessment.networkPlan',{id:assessment.id,ports:parsed.ports});}
    catch(e){setError(errorMessage(e));}finally{setPending(false);}
  }
  return <section className="content-section network-plan"><h2>Observação TCP delimitada</h2><ScopeRestriction method="network.tcp-connect.v1"/>
    {!native?<Notice>O plano TCP está disponível no aplicativo desktop. A prévia não inicia conexões de rede.</Notice>:<>
      <p className="muted">Informe as portas autorizadas para este IP ou CIDR. Preparar o plano não abre conexões. Revise os destinos exatos, aprove o plano e execute em uma ação separada.</p>
      <KeyValue label="Alvo autorizado"><span className="break-word">{assessment.target}</span></KeyValue>
      <Notice>Limites: até 16 IPs, 16 portas únicas e 64 combinações de IP e porta. A execução tenta conexões TCP sem enviar conteúdo da aplicação, resolver nomes DNS ou executar processos externos.</Notice>
      {error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}
      {!projectActive&&<Notice>O projeto está arquivado ou indisponível. Reative um projeto válido antes de preparar outra operação.</Notice>}
      {canPrepare?<><Field label="Portas TCP autorizadas" hint="Informe números separados por vírgula ou espaço, sem intervalos. Nenhuma porta é selecionada automaticamente."><input value={ports} onChange={e=>setPorts(e.target.value)} disabled={busy||pending||!projectActive||!scope.allows('network.tcp-connect.v1')} autoComplete="off" inputMode="text" maxLength={200}/></Field>
        {ports.trim()&&parsed.error&&<p role="alert" className="muted">{parsed.error}</p>}
        <button type="button" className="button primary" disabled={busy||pending||!projectActive||!scope.allows('network.tcp-connect.v1')||!!parsed.error} onClick={()=>void prepare()}>{pending?'Preparando plano…':'Preparar plano TCP'}</button>
      </>:<p className="muted">Consulte o plano e as execuções abaixo. Para alterar portas de um plano aprovado, revogue sua aprovação primeiro.</p>}
    </>}
  </section>;
}
