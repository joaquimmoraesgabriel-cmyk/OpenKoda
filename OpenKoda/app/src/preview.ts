import { Assessment, Kind, Payload, Project, Settings, Snapshot, terminalStatuses } from './types';
import { validEvidenceView } from './data-policy';

export const previewStorageKey = 'openkoda.preview.v1';
type PreviewState = Snapshot & { creationReceipts?: Record<string, Assessment>; launches?: Record<string,{projectId:string;assessmentId:string;intent:string}> };
const uuid = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
export const defaultSettings: Settings = {provider:'compatible',endpoint:'',model:'',mode:'assisted',theme:'light',language:'pt-BR',retentionDays:90,timeoutSeconds:600,outputLimitKiB:256,hasApiKey:false};
export const emptySnapshot = (): Snapshot => ({schema:'openkoda.snapshot-view.v1',projection:{schema:'openkoda.projection.v1',redactorVersion:'openkoda.redactor.v1',mode:'no_credential_configured',redacted:false,omittedFields:[],reasonCodes:[]},projects:[],assessments:[],findings:[],settings:{...defaultSettings}});
const text = (p:Payload,key:string,max=2000) => { const v=p[key]; if(typeof v!=='string' || !v.trim() || v.length>max) throw new Error(`Informe ${key} válido.`); return v.trim(); };
const kind = (p:Payload):Kind => {if(!['code','web','network','files'].includes(String(p.kind))) throw new Error('Modalidade inválida.'); return p.kind as Kind;};
export function validateTarget(value:string, type:Kind) { if(value.length>2000 || /[\x00-\x1f]/.test(value)) throw new Error('Alvo inválido.'); if(type==='web'){let url;try{url=new URL(value);}catch{throw new Error('Informe uma URL HTTP ou HTTPS válida.');}if(!['https:','http:'].includes(url.protocol)||url.username||url.password)throw new Error('Use uma URL HTTP ou HTTPS sem credenciais.');} return value; }
function validSnapshot(s:PreviewState):boolean {
  const strings=(value:unknown,keys:string[])=>!!value&&typeof value==='object'&&keys.every(key=>typeof (value as Record<string,unknown>)[key]==='string');
  if(!s||!Array.isArray(s.projects)||!Array.isArray(s.assessments)||!Array.isArray(s.findings)||!strings(s.settings,['provider','endpoint','model','mode','theme','language']))return false;
  if(!['retentionDays','timeoutSeconds','outputLimitKiB'].every(key=>Number.isInteger(s.settings[key as keyof Settings])))return false;
  if(!s.projects.every(p=>strings(p,['id','name','kind','target','description','createdAt'])&&['code','web','network','files'].includes(p.kind)))return false;
  if(!s.assessments.every(a=>strings(a,['id','projectId','title','kind','target','objective','status','createdAt','updatedAt','summary'])&&s.projects.some(p=>p.id===a.projectId&&p.kind===a.kind&&(p.target===a.target||p.additionalTargets?.some(t=>t.target===a.target&&t.kind===a.kind)))&&Array.isArray(a.events)&&a.events.every(e=>strings(e,['id','at','message','kind']))&&Array.isArray(a.evidence)&&a.evidence.every(validEvidenceView)&&Array.isArray(a.coverage)&&a.coverage.every(c=>typeof c==='string')))return false;
  if (s.creationReceipts !== undefined) {
    if (!s.creationReceipts || typeof s.creationReceipts !== 'object' || Array.isArray(s.creationReceipts)) return false;
    const ids = new Set(s.assessments.map(a => a.id));
    if (!Object.entries(s.creationReceipts).every(([key,a]) => uuid.test(key) && a && ids.has(a.id)
      && strings(a,['id','projectId','kind','target','title','objective','createdAt','updatedAt','summary'])
      && a.status === 'draft' && Array.isArray(a.events) && a.events.length === 1
      && strings(a.events[0],['id','at','message','kind']) && Array.isArray(a.evidence) && !a.evidence.length
      && Array.isArray(a.coverage) && !a.coverage.length)) return false;
  }
  return s.findings.every(f=>strings(f,['id','assessmentId','title','severity','confidence','status','description','recommendation','createdAt'])&&Array.isArray(f.evidenceIds)&&f.evidenceIds.every(id=>typeof id==='string')&&s.assessments.some(a=>a.id===f.assessmentId));
}
export function createPreviewTransport(storage: Pick<Storage,'getItem'|'setItem'>) {
  const read = ():PreviewState => { const raw=storage.getItem(previewStorageKey); if(!raw)return emptySnapshot(); try{const s=JSON.parse(raw);if(!validSnapshot(s))throw Error();return {...s,schema:emptySnapshot().schema,projection:emptySnapshot().projection};}catch{throw new Error('Os dados da prévia estão inválidos. Eles foram preservados; use o aplicativo nativo ou recupere o armazenamento da prévia.');} };
  const write = (s:PreviewState) => storage.setItem(previewStorageKey,JSON.stringify(s));
  function handle<T>(action:string,payload:Payload):T {
    if(action==='environment.status')return {status:'unavailable',distro:'OpenKoda-Kali',message:'O ambiente Kali só pode ser consultado no aplicativo desktop.',tools:[]} as T;
    const s=read(); const now=new Date().toISOString();
    if(action==='snapshot'){
      const {creationReceipts, launches, ...view}=s;
      return structuredClone(view) as T;
    }
    let result:unknown;
    switch(action){
      case 'project.launch': {
        const key=text(payload,'idempotencyKey',36);
        if (!uuid.test(key)) throw new Error('Chave de idempotência inválida.');
        const intent=JSON.stringify(payload), receipt=s.launches?.[key];
        if (receipt) {
          if(receipt.intent!==intent) throw new Error('scope_idempotency_conflict');
          return {projectId:receipt.projectId,assessmentId:receipt.assessmentId,replayed:true} as T;
        }
        const k=kind(payload),root=validateTarget(text(payload,'root',4096),k);
        const objective=text(payload,'objective',4000),title=text(payload,'title',160);
        let project=s.projects.find(p=>!p.archived && p.kind===k && p.target===root && (!payload.projectId||payload.projectId===p.id));
        if(!project) {
          if(payload.projectId)throw new Error('Projeto indisponível.');
          project={id:crypto.randomUUID(),kind:k,target:root,name:text(payload,'name',120),description:objective,createdAt:now};
          s.projects.unshift(project);
        }
        let target=root;
        if(payload.relativeTarget) {
          const relative=text(payload,'relativeTarget',4096).replaceAll('\\','/');
          if(!['code','files'].includes(k)||relative.split('/').some(part=>!part||part==='.'||part==='..'||part.includes(':')))throw new Error('Informe um caminho dentro do projeto.');
          target=root.replace(/[\\/]+$/,'')+'/'+relative;
          project.additionalTargets??=[];
          if(!project.additionalTargets.some(t=>t.target===target))project.additionalTargets.push({id:crypto.randomUUID(),kind:k,target,label:relative,createdAt:now});
        }
        const id=crypto.randomUUID();
        s.assessments.unshift({id,projectId:project.id,title,kind:k,target,objective,status:'draft',createdAt:now,updatedAt:now,
          events:[{id:crypto.randomUUID(),at:now,message:'Conversa criada na prévia. Abra o aplicativo desktop para analisar o alvo.',kind:'info'}],evidence:[],coverage:[],summary:''});
        s.launches??={};s.launches[key]={projectId:project.id,assessmentId:id,intent};
        result={projectId:project.id,assessmentId:id,replayed:false};
        break;
      }
      case 'project.newConversation': {
        const key=text(payload,'idempotencyKey',36);
        if(!uuid.test(key))throw new Error('Chave de idempotência inválida.');
        const intent=JSON.stringify(payload), receipt=s.launches?.[key];
        if(receipt){
          if(receipt.intent!==intent)throw new Error('scope_idempotency_conflict');
          const previous=s.assessments.find(a=>a.id===receipt.assessmentId);
          if(!previous)throw new Error('Conversa indisponível.');
          return structuredClone(previous) as T;
        }
        const project=s.projects.find(p=>p.id===payload.projectId && !p.archived);
        if(!project)throw new Error('Escolha um projeto ativo.');
        const source=s.assessments.find(a=>a.id===payload.sourceAssessmentId && a.projectId===project.id);
        if(payload.sourceAssessmentId && !source)throw new Error('A conversa de origem não pertence a este projeto.');
        const a:Assessment={id:crypto.randomUUID(),projectId:project.id,kind:source?.kind??project.kind,target:source?.target??project.target,
          title:text(payload,'title',160),objective:source?.objective??'Revisar este projeto em conversa.',aiSelection:source?.aiSelection,
          status:'draft',createdAt:now,updatedAt:now,events:[],evidence:[],coverage:[],summary:''};
        s.assessments.unshift(a);
        s.launches??={};s.launches[key]={projectId:project.id,assessmentId:a.id,intent};
        result=a;break;
      }
      case 'project.create': {const k=kind(payload);const p:Project={id:crypto.randomUUID(),name:text(payload,'name',160),kind:k,target:validateTarget(text(payload,'target'),k),description:typeof payload.description==='string'?payload.description.slice(0,4000):'',createdAt:now};s.projects.unshift(p);result=p;break;}
      case 'project.archive':{const p=s.projects.find(p=>p.id===payload.id);if(!p)throw new Error('Projeto não encontrado.');p.archived=true;result={ok:true};break;}
      case 'assessment.create': {
        let key: string | undefined;
        if (Object.hasOwn(payload,'idempotencyKey')) {
          if (typeof payload.idempotencyKey !== 'string' || !uuid.test(payload.idempotencyKey)) throw new Error('Chave de idempotência inválida.');
          key = payload.idempotencyKey.toLowerCase();
        }
        const title=text(payload,'title',160), objective=text(payload,'objective',4000);
        const receipt=key && s.creationReceipts?.[key];
        if (receipt) {
          if (receipt.projectId !== payload.projectId || receipt.kind !== payload.kind || receipt.target !== payload.target
            || receipt.title !== title || receipt.objective !== objective || payload.scope !== undefined) {
            throw new Error('scope_idempotency_conflict: esta chave já foi usada para outra solicitação.');
          }
          return structuredClone(receipt) as T;
        }
        if(payload.scope!==undefined)throw new Error('Escopo versionado requer o aplicativo desktop. A prévia não concede autorização.');
        const p=s.projects.find(p=>p.id===payload.projectId&&!p.archived);
        if(!p)throw new Error('Escolha um projeto ativo.');
        if(payload.target!==p.target||payload.kind!==p.kind)throw new Error('A avaliação deve respeitar o alvo e a modalidade do projeto.');
        const a:Assessment={id:crypto.randomUUID(),projectId:p.id,title,kind:p.kind,target:p.target,objective,status:'draft',createdAt:now,updatedAt:now,events:[{id:crypto.randomUUID(),at:now,message:'Rascunho criado na prévia local. Nenhuma ferramenta executada.',kind:'info'}],evidence:[],coverage:[],summary:''};
        s.assessments.unshift(a);
        if(key){s.creationReceipts??={};s.creationReceipts[key]=structuredClone(a);}
        result=a;break;
      }
      case 'assessment.start':throw new Error('A execução de avaliações requer o aplicativo desktop e um executor preparado. O rascunho foi preservado.');
      case 'assessment.pause':throw new Error('Não existe execução ativa nesta prévia para pausar.');
      case 'assessment.cancel':{const a=s.assessments.find(a=>a.id===payload.id);if(!a)throw new Error('Avaliação não encontrada.');if(terminalStatuses.includes(a.status))throw new Error('Esta avaliação já foi encerrada.');a.status='canceled';a.updatedAt=now;a.events.push({id:crypto.randomUUID(),at:now,message:'Rascunho cancelado. Nenhum processo estava em execução.',kind:'info'});result=a;break;}
      case 'settings.save':{const allowed=['provider','endpoint','model','mode','theme','language','retentionDays','timeoutSeconds','outputLimitKiB'];if(Object.keys(payload).some(k=>!allowed.includes(k)))throw new Error('Campo de configuração não permitido.');if(payload.endpoint){let u;try{u=new URL(String(payload.endpoint));}catch{throw new Error('Endpoint inválido.');}if(u.protocol!=='https:'||u.username||u.password)throw new Error('O endpoint deve usar HTTPS e não conter credenciais.');}for(const k of ['retentionDays','timeoutSeconds','outputLimitKiB'])if(k in payload&&(!Number.isInteger(payload[k])||Number(payload[k])<1))throw new Error('Os limites devem ser números inteiros positivos.');s.settings={...s.settings,...payload,hasApiKey:false};result=s.settings;break;}
      case 'report.export':throw new Error('A exportação de relatórios é realizada pelo aplicativo desktop. A prévia não grava arquivos no computador.');
      default:throw new Error('Esta capacidade ainda não está disponível na prévia do navegador.');
    }
    write(s);return structuredClone(result) as T;
  }
  return async <T>(action:string,payload:Payload={}):Promise<T> => {
    // Other preview tabs use the same localStorage item. Serialize read/modify/write across them.
    if (action !== 'snapshot' && action !== 'environment.status' && typeof window !== 'undefined'
      && storage === window.localStorage && navigator.locks) {
      return navigator.locks.request(previewStorageKey, () => handle<T>(action,payload));
    }
    return handle<T>(action,payload);
  };
}
