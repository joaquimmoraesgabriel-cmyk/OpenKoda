import { useState } from 'react';
import { Action, Project } from '../types';
import { Field } from './shared';
import DataPolicy from './DataPolicy';
import CredentialReferences from './CredentialReferences';

export default function ProjectDataSettings({projects,action,busy}:{projects:Project[];action:Action;busy:boolean}){
 const [selected,setSelected]=useState('');const project=projects.find(item=>item.id===selected);
 return <section className="content-section"><h2>Dados por projeto</h2><p className="muted">Revise políticas e referências, incluindo projetos arquivados. A revisão de recuperação não reativa operações sobre os alvos.</p><Field label="Projeto para política de dados"><select value={selected} onChange={e=>setSelected(e.target.value)}><option value="">Selecione um projeto</option>{projects.map(item=><option key={item.id} value={item.id}>{item.name}{item.archived?' · arquivado':''}</option>)}</select></Field>{project&&<><DataPolicy key={project.id} projectId={project.id} action={action} busy={busy}/><CredentialReferences key={'refs-'+project.id} projectId={project.id} action={action} busy={busy}/></>}</section>;
}
