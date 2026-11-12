import { ArrowRight, FolderCode, Github, Globe2, Network } from 'lucide-react';
import { useState } from 'react';
import { KMark } from './ProjectRail';

export type StartKind = 'code' | 'github' | 'web' | 'network';

const targets = [
  {kind:'code', label:'Pasta local', hint:'Código e arquivos', icon:FolderCode},
  {kind:'github', label:'GitHub', hint:'Baixar um repositório público', icon:Github},
  {kind:'web', label:'Web / API', hint:'Site ou endpoint', icon:Globe2},
  {kind:'network', label:'Rede', hint:'Endereço IP ou CIDR', icon:Network},
] as const;

export default function ProjectWelcome({onStart,sourceOnly=false}:{onStart:(kind:StartKind)=>void;sourceOnly?:boolean}) {
  const [kind,setKind]=useState<StartKind>('code');
  return <section className="welcome-stage" aria-labelledby="welcome-title">
    <div className="welcome-content">
      <div className="welcome-brand"><KMark size={28}/><span>OpenKoda</span></div>
      <h1 id="welcome-title">Seu primeiro projeto</h1>
      <p>{sourceOnly?'Escolha uma pasta ou um repositório do GitHub para analisar.':'Escolha o que você quer analisar.'}</p>
      <fieldset className="welcome-targets"><legend className="sr-only">Alvo do projeto</legend>
        {(sourceOnly?targets.slice(0,2):targets).map(({kind:id,label,hint,icon:Icon})=><label key={id} className="welcome-option">
          <input type="radio" name="welcome-target" value={id} checked={id===kind} onChange={()=>setKind(id)}/>
          <span className="welcome-type"><Icon size={20} strokeWidth={1.65}/></span>
          <span><strong>{label}</strong><small>{hint}</small></span>
          <span className="welcome-radio" aria-hidden="true"/>
        </label>)}
      </fieldset>
      <button type="button" className="button primary welcome-start" onClick={()=>onStart(kind)}>Criar projeto<ArrowRight size={16}/></button>
    </div>
  </section>;
}
