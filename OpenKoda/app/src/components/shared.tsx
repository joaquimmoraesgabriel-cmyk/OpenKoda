import { Children, ReactElement, ReactNode, cloneElement, isValidElement, useId } from 'react';
import { Search, ShieldCheck, Info, AlertTriangle } from 'lucide-react';
import { AssessmentStatus, statusLabels } from '../types';

export function SearchBox({value,onChange,label}:{value:string;onChange:(v:string)=>void;label:string}) {return <div className="search-box"><Search size={18}/><input type="search" aria-label={label} placeholder={label} value={value} onChange={e=>onChange(e.target.value)}/></div>;}
export function Empty({title,description,action,icon}:{title:string;description:string;action?:ReactNode;icon?:ReactNode}) {return <div className="empty-state"><div className="empty-icon">{icon??<ShieldCheck size={30} strokeWidth={1.5}/>}</div><h2>{title}</h2><p>{description}</p>{action}</div>;}
export function Notice({children,tone='info'}:{children:ReactNode;tone?:'info'|'warning'}) {return <div className={`notice ${tone}`}>{tone==='warning'?<AlertTriangle size={19}/>:<Info size={19}/>}<div>{children}</div></div>;}
export function Status({value}:{value:AssessmentStatus}) {return <span className={`status status-${value}`}><span className="status-dot"/>{statusLabels[value]??value}</span>;}
export function Tabs({items,selected,onChange,label='Seções'}:{items:string[];selected:string;onChange:(value:string)=>void;label?:string}) {return <nav className="tabs" aria-label={label}>{items.map(item=><button key={item} type="button" className={selected===item?'active':''} aria-current={selected===item?'page':undefined} onClick={()=>onChange(item)}>{item}</button>)}</nav>;}
export function Disclosure({label,icon,children}:{label:string;icon?:ReactNode;children:ReactNode}) {return <details className="disclosure"><summary>{icon}<span>{label}</span></summary><div className="disclosure-body">{children}</div></details>;}
export function Field({label,children,hint}:{label:string;children:ReactNode;hint?:string}) {
 const id=useId();
 return <div className="field"><label className="field-label" htmlFor={id}>{label}</label><div>{Children.map(children,child=>isValidElement(child)&&['input','select','textarea'].includes(String(child.type))?cloneElement(child as ReactElement<{id:string;'aria-describedby'?:string}>,{id,'aria-describedby':hint?`${id}-hint`:undefined}):child)}{hint&&<small id={`${id}-hint`}>{hint}</small>}</div></div>;
}
export function KeyValue({label,children}:{label:string;children:ReactNode}){return <div className="key-value"><span>{label}</span><div>{children}</div></div>;}
