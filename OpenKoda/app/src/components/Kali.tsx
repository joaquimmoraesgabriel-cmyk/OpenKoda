import KaliArchives from './KaliArchives';
import { useEffect, useRef, useState } from 'react';
import { Box, Download, RefreshCw, Server, Wrench } from 'lucide-react';
import { native, request } from '../api';
import { Action, Environment, errorMessage, shortDate } from '../types';
import { Empty, KeyValue, Notice, SearchBox, Tabs } from './shared';

interface CatalogTool {package:string;name:string;group:string;description:string;adapter:string;adapterImplemented?:boolean;executionReady:boolean;availability:string;installed:boolean|null;version:string|null}
interface Catalog {version:string;tools:CatalogTool[];inventoryVerified:boolean;distro:string}
interface MaintenancePlan {id:string;kind:string;environmentId:string;createdAt:string;expiresAt:string;payload:Record<string,unknown>;digest:string;consumed:boolean}

export default function Kali({environment,action,busy,onExecutorChange}:{environment:Environment;action:Action;busy:boolean;onExecutorChange:(executor?:{distro:string;environmentId:string})=>void}) {
  const [tab,setTab]=useState('Ferramentas');const [search,setSearch]=useState('');
  const [catalog,setCatalog]=useState<Catalog>();const [selected,setSelected]=useState<string[]>([]);
  const [plan,setPlan]=useState<MaintenancePlan>();const [approved,setApproved]=useState(false);
  const [attemptedPlans,setAttemptedPlans]=useState<string[]>([]);
  const [pending,setPending]=useState(false);const [pendingAction,setPendingAction]=useState('');const [error,setError]=useState('');const [message,setMessage]=useState('');
  const catalogGeneration=useRef(0);
  async function refreshCatalog(){
    if(!native)return;
    const generation=catalogGeneration.current;
    const next=await request<Catalog>('packages.catalog');
    if(generation===catalogGeneration.current)setCatalog(next);
  }
  useEffect(()=>{void refreshCatalog().catch(e=>setError(errorMessage(e)));},[]);
  async function perform(name:string,payload:Record<string,unknown>={}) {
    setPending(true);setPendingAction(name);setError('');setMessage('');
    const preparing=['environment.plan','environment.image.plan','environment.repair.plan','packages.plan'].includes(name);
    if(preparing){setPlan(undefined);setApproved(false);}
    const applying=['environment.provision','environment.image.provision','environment.repair.apply','packages.apply'].includes(name);
    if(applying&&plan){setAttemptedPlans(ids=>[...ids,plan.id]);setApproved(false);}
    try {
      const result=await action<unknown>(name,payload);
      if(preparing){setPlan(result as MaintenancePlan);setApproved(false);setTab('Manutenção');}
      else {await refreshCatalog();setMessage(name==='environment.provision'||name==='environment.image.provision'?'Preparação encerrada. Consulte o estado verificado do ambiente.':name==='environment.repair.apply'?'Tentativa de recuperação registrada. Consulte o estado verificado do ambiente.':name==='packages.apply'?'Manutenção encerrada. Consulte o inventário verificado.':'Diagnóstico atualizado.');if(applying){setPlan(undefined);setApproved(false);setSelected([]);}}
    }catch(e){setError(errorMessage(e));}finally{setPending(false);setPendingAction('');}
  }
  const disabled=busy||pending;
  const tools=(catalog?.tools??[]).filter(tool=>(tool.name+' '+tool.package+' '+tool.group).toLowerCase().includes(search.toLowerCase()));
  const expired=plan?Date.parse(plan.expiresAt)<=Date.now():false;
  const applyAction=plan?({provision:'environment.provision','provision-image':'environment.image.provision','environment-repair':'environment.repair.apply',packages:'packages.apply'} as Record<string,string>)[plan.kind]:undefined;
  const attempted=!!plan&&attemptedPlans.includes(plan.id);
  const imageFallback=native&&environment.status!=='ready'&&environment.imageProvisionAvailable===true;
  const imagePreparation=imageFallback&&<div className="spaced"><p className="muted">Se a preparação padrão falhar, esta alternativa baixa a imagem oficial do Kali e verifica seu SHA-256. A instalação exige uma revisão separada do plano.</p><button className="button" disabled={disabled} onClick={()=>void perform('environment.image.plan')}><Download size={17}/>Preparar com imagem oficial verificada</button></div>;
  const repairPreparation=native&&environment.repairAvailable===true&&<div className="spaced"><p className="muted">A recuperação revisada pode encerrar somente a distribuição gerida {environment.distro} para reconciliar seu estado. Ela não encerra as demais distribuições nem instala ou repara pacotes automaticamente.</p><button className="button" disabled={disabled} onClick={()=>void perform('environment.repair.plan')}>Revisar recuperação do Kali</button></div>;
  return <div className="single-page"><div className="detail-heading"><div><h1>Kali e ferramentas</h1><p>Prepare o ambiente e acompanhe as ferramentas disponíveis.</p></div><button className="button" disabled={disabled} onClick={()=>void perform(native?'environment.diagnose':'environment.status')}><RefreshCw size={17}/>{pending?'Verificando…':'Atualizar diagnóstico'}</button></div>
    <section className={`environment-card env-${environment.status}`}><Server size={27}/><div><h2>{environment.status==='ready'?'Kali conectado':environment.status==='degraded'?'Ambiente requer atenção':'Kali ainda não conectado'}</h2><p>{environment.message}</p></div><span className="badge">{environment.distro}</span></section>
    {error&&<div role="alert"><Notice tone="warning">{error}</Notice></div>}{message&&<p role="status" className="saved-message">{message}</p>}{pendingAction==='environment.image.plan'&&<p role="status">Baixando e verificando a imagem oficial. Isso pode levar alguns minutos; a distribuição ainda não será instalada.</p>}
    <Tabs items={['Ferramentas','Ambiente','Manutenção']} selected={tab} onChange={setTab}/>
    {tab==='Ferramentas'&&<section className="content-section"><div className="section-heading"><div><h2>Catálogo de ferramentas</h2><p className="muted">Instalação, disponibilidade do adaptador e execução são verificadas separadamente.</p></div><SearchBox label="Pesquisar ferramentas" value={search} onChange={setSearch}/></div>{tools.length?<><div className="tool-list">{tools.map(tool=><label className="tool-row" key={tool.package}><input type="checkbox" checked={selected.includes(tool.package)} onChange={e=>setSelected(items=>e.target.checked?[...items,tool.package]:items.filter(item=>item!==tool.package))} disabled={disabled||!native}/><Box size={20}/><div className="tool-copy"><strong>{tool.name}</strong><p>{tool.description}</p>{tool.availability&&<p>{tool.availability}</p>}<small>{tool.group} · {tool.package}{tool.version?' · '+tool.version:''}</small></div><div className="tool-badges"><span className="badge">{tool.installed===null?'Inventário pendente':tool.installed?'Instalado':'Não instalado'}</span><span className="badge">{tool.adapterImplemented?'Adaptador disponível':tool.executionReady?'Execução disponível':'Adaptador não liberado'}</span></div></label>)}</div><div className="form-actions"><span className="muted">{selected.length} pacote(s) selecionado(s)</span><button className="button primary" disabled={disabled||!selected.length} onClick={()=>void perform('packages.plan',{packages:selected})}>Simular instalação</button></div></>:<Empty title={search?'Nenhuma ferramenta encontrada':'Nenhuma ferramenta verificada'} description={search?'Experimente outro termo de busca.':native?'Consulte o diagnóstico para carregar o catálogo e verificar o inventário.':'O catálogo é consultado no aplicativo desktop, com o ambiente Kali conectado.'} icon={<Wrench size={30}/>}/>}</section>}
    {tab==='Ambiente'&&<section className="content-section"><h2>Identidade e diagnóstico</h2><KeyValue label="Distribuição gerenciada">{environment.distro}</KeyValue><KeyValue label="Estado">{environment.status==='ready'?'Disponível':environment.status==='degraded'?'Degradado':'Indisponível'}</KeyValue>{environment.phase&&<KeyValue label="Etapa do ambiente">{environment.phase}</KeyValue>}{environment.environmentId&&<KeyValue label="Identidade"><span className="mono break-word">{environment.environmentId}</span></KeyValue>}<KeyValue label="Executor Kali">{environment.executorReady?'Disponível':environment.status==='ready'&&environment.executorReady===undefined?'Consultar diagnóstico':'Ainda não liberado'}</KeyValue>{environment.containment&&<KeyValue label="Limite de isolamento">{environment.containment}</KeyValue>}{environment.updatedAt&&<KeyValue label="Última verificação">{shortDate(environment.updatedAt)}</KeyValue>}{environment.lastError&&<Notice tone="warning">{environment.lastError}</Notice>}<Notice>Uma distribuição pessoal existente não é adotada, alterada ou removida automaticamente. A preparação usa um ambiente dedicado do OpenKoda.</Notice>{!!environment.details&&<details className="plan-details"><summary>Detalhes do diagnóstico</summary><pre>{typeof environment.details==='string'?environment.details:JSON.stringify(environment.details,null,2)}</pre></details>}{native&&<div className="inline-actions"><button className="button primary" disabled={disabled} onClick={()=>void perform('environment.plan')}><Download size={17}/>Revisar preparação do Kali</button><button className="button" disabled={disabled} onClick={()=>void perform('environment.resume')}>Reconciliar preparação anterior</button></div>}{imagePreparation}{repairPreparation}</section>}
    {tab==='Manutenção'&&<section className="content-section"><h2>Pacotes e recuperação</h2><Notice>A manutenção é separada das avaliações. Revise o plano e seus efeitos antes de instalar ou alterar o ambiente.</Notice>{native&&<div className="inline-actions"><button className="button" disabled={disabled} onClick={()=>void perform('packages.refresh')}><RefreshCw size={17}/>Atualizar metadados do catálogo</button><button className="button" disabled={disabled} onClick={()=>void perform('environment.plan')}>Revisar preparação do Kali</button></div>}{!native&&<p className="muted">A preparação e a manutenção do ambiente estão disponíveis no aplicativo desktop.</p>}{imagePreparation}{repairPreparation}
      {plan&&<div className="maintenance-review"><h3>Revisão da operação</h3><KeyValue label="Operação">{plan.kind}</KeyValue><KeyValue label="Ambiente"><span className="mono break-word">{plan.environmentId}</span></KeyValue><KeyValue label="Validade">{shortDate(plan.expiresAt)}{expired?' · Expirado':''}</KeyValue><pre aria-label="Detalhes do plano de manutenção">{JSON.stringify(plan.payload,null,2)}</pre><details className="plan-details"><summary>Integridade do plano</summary><p className="mono break-word">SHA-256: {plan.digest}</p></details>{attempted&&<Notice tone="warning">Este plano já foi enviado. Consulte o diagnóstico e revise um novo plano antes de outra tentativa.</Notice>}<label className="check-field"><input type="checkbox" checked={approved} onChange={e=>setApproved(e.target.checked)} disabled={disabled||expired||plan.consumed||attempted}/><span>Revisei o ambiente, a origem e os efeitos deste plano e autorizo sua execução.</span></label><div className="inline-actions"><button className="button primary" disabled={disabled||!approved||expired||plan.consumed||attempted||!applyAction} onClick={()=>applyAction&&void perform(applyAction,{planId:plan.id,digest:plan.digest})}>{pending?'Executando manutenção…':'Executar operação revisada'}</button><button className="button" disabled={disabled} onClick={()=>{setPlan(undefined);setApproved(false);}}>Descartar plano</button></div></div>}
      {native&&<KaliArchives action={action} busy={disabled} onExecutorChange={executor=>{
        catalogGeneration.current++;setCatalog(undefined);setSelected([]);setPlan(undefined);setApproved(false);setAttemptedPlans([]);setMessage('');setError('');
        onExecutorChange(executor);
      }}/>}
      <p className="muted spaced">Se houver falha ou interrupção, consulte o diagnóstico antes de repetir uma operação. O estado e os registros são preservados.</p>
    </section>}
  </div>;
}
