//! Local host for the pinned Codex Security SDK. The WebView supplies neither
//! an executable, an output directory, a repository root, nor credentials.
use crate::{ai::AiService, ai_provider::ProviderId, core::{self, Store}, model::{self, Assessment, Evidence, Snapshot}, scope, scope_targets};
use serde::{Deserialize, Serialize};
use serde_json::{json,Value};
use std::{collections::HashSet, fs, io::{BufRead,BufReader,Read,Write}, path::{Path,PathBuf}, process::{Child,Command,Stdio}, sync::{Arc,Mutex,atomic::{AtomicBool,Ordering},mpsc},thread,time::{Duration,Instant}};
use zeroize::Zeroizing;

const LIMIT:usize=2*1024*1024;
const UNEXPECTED:&str="O SDK não entregou um resultado terminal válido. Consulte o histórico antes de iniciar outra varredura.";
const INTERRUPTED:&str="Execução interrompida. O resultado remoto e o custo final podem ser desconhecidos.";

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Progress {at:String,stage:String,message:String}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Receipt {
    id:String,assessment_id:String,idempotency_key:String,request_digest:String,status:String,stage:String,message:String,
    created_at:String,updated_at:String,provider:String,model:String,max_cost_usd:f64,events:Vec<Progress>,
    result_evidence_id:Option<String>,findings_total:Option<u64>,error:Option<String>,
}
impl Receipt {
    fn active(&self)->bool{matches!(self.status.as_str(),"starting"|"running"|"cancelling")}
    fn update(&mut self,status:&str,stage:&str,message:&str){
        self.status=status.into();self.stage=stage.into();self.message=message.into();self.updated_at=model::now();
        if self.events.len()>=64{self.events.remove(1);}
        self.events.push(Progress{at:self.updated_at.clone(),stage:stage.into(),message:message.into()});
    }
}
#[derive(Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(crate) struct Start{assessment_id:String,idempotency_key:String,prompt:Option<String>,target:Option<Targets>,max_cost_usd:f64,#[serde(default,skip_serializing_if="std::ops::Not::not")]acknowledge_sensitive:bool}
#[derive(Serialize,Deserialize)]
#[serde(untagged)]
enum Targets{One(String),Many(Vec<String>)}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Lookup {assessment_id:String}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Status {assessment_id:Option<String>}

#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Finding{#[serde(default,skip_serializing_if="Option::is_none")]id:Option<String>,title:String,severity:String,description:String,location:Option<String>}
#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Coverage{
    scope:Vec<String>,excluded_paths:Vec<String>,additional_exclusions:u64,findings_total:u64,report_truncated:bool,
    #[serde(default,skip_serializing_if="String::is_empty")]summary:String,
    #[serde(default,skip_serializing_if="Vec::is_empty")]limitations:Vec<String>,
}
#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct Cost{estimated_usd:Option<f64>}
#[derive(Serialize,Deserialize,Debug)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ResultData {scan_id:Option<String>,report:String,findings:Vec<Finding>,coverage:Coverage,cost:Option<Cost>}
#[derive(Deserialize)]
#[serde(tag="type",rename_all="lowercase")]
enum Frame {
    Progress{stage:String,message:String},
    Error{code:String,message:String},
    Result{status:String,#[serde(flatten)]data:ResultData},
}
struct Terminal{status:String,data:Option<ResultData>,error:Option<String>}
impl Terminal {
    fn failure(message:&str)->Self{Self{status:"failed".into(),data:None,error:Some(message.into())}}
    fn interrupted()->Self{Self{status:"interrupted".into(),data:None,error:Some(INTERRUPTED.into())}}
}
struct Scope {repo:PathBuf,targets:Vec<String>,deadline:Instant,_locks:Vec<crate::source_scan::MetadataLease>}
struct Job<'a> {id:String,aid:String,prompt:String,objective_digest:String,acknowledge_sensitive:bool,scope:Scope,provider:ProviderId,model:String,cap:f64,key:Zeroizing<String>,output:PathBuf,cancel:Arc<AtomicBool>,_inference:std::sync::MutexGuard<'a,()>}
struct Active {aid:String,cancel:Arc<AtomicBool>}
pub(crate) struct Service{helper:PathBuf,node:PathBuf,active:Mutex<Option<Active>>,check_cache:Mutex<Option<(Instant,Value)>>}

fn decode<T:serde::de::DeserializeOwned>(value:Value)->Result<T,String>{serde_json::from_value(value).map_err(|_|"Campos inválidos para a análise OpenKoda.".into())}
// std::fs and Tauri can return a Win32 verbatim disk path. Node 24 treats
// that spelling as an invalid module entry point (EISDIR on the drive).
// Keep the exact drive and OS components, including non-ASCII characters.
fn node_path(path:&Path)->PathBuf{
    #[cfg(windows)]{
        use std::path::{Component,Prefix};
        let mut components=path.components();
        if let Some(Component::Prefix(prefix))=components.next(){
            if let Prefix::VerbatimDisk(drive)=prefix.kind(){
                if components.next()==Some(Component::RootDir){
                    let mut dos=PathBuf::from(format!("{}:\\",char::from(drive)));
                    dos.extend(components.map(|c|c.as_os_str()));
                    return dos;
                }
            }
        }
    }
    path.to_path_buf()
}
fn lookup<'a>(s:&'a Snapshot,aid:&str)->Result<&'a Assessment,String>{s.assessments.iter().find(|a|a.id==aid).ok_or_else(||"Avaliação não encontrada.".into())}
fn latest(a:&Assessment)->Option<&Receipt>{a.events.iter().rev().find_map(|e|e.security_scan.as_ref())}
fn public(a:&Assessment,r:&Receipt)->Result<Value,String>{
    let mut v=json!(r);
    let o=v.as_object_mut().unwrap();o.remove("requestDigest");o.remove("idempotencyKey");
    let result=if let Some(id)=&r.result_evidence_id{
        let ev=a.evidence.iter().find(|e|&e.id==id).ok_or("O recibo não possui a evidência vinculada.")?;
        let data=serde_json::from_str::<ResultData>(&ev.content).map_err(|_|"Relatório do SDK inválido no armazenamento.")?;
        json!(data)
    }else{Value::Null};
    o.insert("result".into(),result);Ok(json!({"scan":v}))
}
fn public_checked(store:&Store,s:&Snapshot,a:&Assessment,r:&Receipt)->Result<Value,String>{
    use crate::data_policy::{DataDestination,DataOperation,DataPolicyService,DataSubject,DataUseSpec,RequestFingerprint};
    use crate::data_projection::{classification_state,resolve_subject,subject_versions,RejectExternal};
    let mut value=public(a,r)?;
    let Some(eid)=r.result_evidence_id.as_ref()else{return Ok(value)};
    // SDK reports can contain complete source excerpts. They are a local view
    // of their canonical evidence, so a get or idempotency replay must respect
    // the same current/adopted policy and transitive classifications as reads.
    let decision=(||{
        let subject=DataSubject::Evidence{assessment_id:a.id.clone(),evidence_id:eid.clone()};
        let resolved=resolve_subject(store,s,&subject,&RejectExternal)?;
        let class=classification_state(s,&resolved)?;
        let evidence=a.evidence.iter().find(|e|e.id==*eid).ok_or("data_subject_missing")?;
        let mut spec=DataUseSpec::new(DataOperation::EvidenceRead,DataDestination::LocalRawView,vec![a.project_id.clone()],Some(a.id.clone()));
        spec.selection.package_sensitivity=class.sensitivity;spec.selection.categories=resolved.categories;spec.selection.origins=resolved.origins;
        spec.selection.evidence_ids=vec![eid.clone()];spec.subjects=subject_versions(store,s,&subject,&RejectExternal)?;
        spec.requests=vec![RequestFingerprint{index:0,input_sha256:evidence.sha256.clone(),output_sha256:evidence.sha256.clone(),input_bytes:evidence.content.len() as u64,output_bytes:evidence.content.len() as u64}];
        DataPolicyService::check_use(s,&spec)
    })();
    let reasons=match decision{Ok(d) if d.allowed=>return Ok(value),Ok(d)=>d.reason_codes,Err(_)=>vec!["data_source_unknown".into()]};
    value["scan"]["result"]=Value::Null;value["scan"]["resultUnavailable"]=json!(true);value["scan"]["resultReasonCodes"]=json!(reasons);
    Ok(value)
}
fn record(store:&Store,aid:&str,id:&str,edit:impl FnOnce(&mut Receipt)->Result<(),String>)->Result<(),String>{
    store.mutate(|s|{
        let a=s.assessments.iter_mut().find(|a|a.id==aid).ok_or("Avaliação ausente.")?;
        let e=a.events.iter_mut().find(|e|e.security_scan.as_ref().is_some_and(|r|r.id==id)).ok_or("Recibo do SDK ausente.")?;
        let r=e.security_scan.as_mut().unwrap();edit(r)?;e.message=r.message.clone();a.updated_at=r.updated_at.clone();Ok(())
    })
}
fn currency(value:f64)->Result<u64,String>{
    if !value.is_finite()||value<0.0||value>1_000_000.0{return Err("Valor monetário inválido.".into());}
    Ok((value*1_000_000.0).ceil() as u64)
}
fn check_data(s:&Snapshot,aid:&str,provider:ProviderId)->Result<bool,String>{
    use crate::data_policy::{self,DataCategory,DataDestination,DataSubject,Sensitivity};
    let a=lookup(s,aid)?;
    let Some(policy)=s.data_policies.iter().find(|p|p.project_id==a.project_id)else{return Ok(false)};
    if !policy.active{return Err("A política de dados deste projeto foi revogada.".into());}
    let binding=s.data_bindings.iter().find(|b|b.assessment_id==aid).ok_or("A avaliação precisa adotar a política de dados vigente antes de enviar fontes.")?;
    let scope=scope::current_ref(s,aid)?.ok_or("Revise o escopo da avaliação.")?;
    let body=scope_targets::current_body(s,aid)?;
    let entry=body.target_set.as_ref().and_then(|set|set.entries.iter().find(|e|e.entry_id==set.primary_entry_id));
    let target=entry.map_or("primary",|e|e.project_target_id.as_str());
    let subjects=[
        DataSubject::ScopeSource{assessment_id:aid.into(),scope_ref:scope,target_ref:entry.map(scope_targets::entry_ref)},
        DataSubject::AssessmentMetadata{assessment_id:aid.into(),category:DataCategory::HumanText}
    ];
    let mut sensitivity=Sensitivity::Internal;
    for subject in subjects{
        let fingerprint=crate::data_projection::canonical_fingerprint(s,&subject)?;
        let c=data_policy::current_classification(s,&subject).filter(|c|Some(&c.source_fingerprint)==fingerprint.as_ref())
            .ok_or("Revise a classificação da fonte e do objetivo desta avaliação.")?;
        if c.secret_material||c.sensitivity==Sensitivity::Unclassified{return Err("A classificação vigente impede o envio desta fonte.".into());}
        sensitivity=sensitivity.max(c.sensitivity);
    }
    for id in [&policy.current_revision_id,&binding.policy_ref.revision_id]{
        let rev=s.data_policy_revisions.iter().find(|r|&r.id==id&&r.project_id==a.project_id).ok_or("Revisão da política de dados ausente.")?;
        let rule=rev.body.destinations.iter().find(|r|r.destination==DataDestination::RemoteAi).ok_or("Destino de IA remota ausente.")?;
        if !rule.enabled||!rule.target_catalog_ids.iter().any(|t|t==target)||!rule.sensitivities.contains(&sensitivity)
            ||[DataCategory::HumanText,DataCategory::TargetMetadata,DataCategory::SourceExcerpt].iter().any(|cat|!rule.categories.contains(cat)){
            return Err("A política de dados vigente não permite enviar o objetivo e as fontes deste alvo ao provedor.".into());
        }
    }
    let slot=crate::credential_refs::provider_slot(s,provider).ok_or("Configure a credencial deste provedor.")?;
    let permitted=s.credential_references.iter().any(|r|r.project_id==a.project_id&&!r.revoked
        &&r.provider_slot_ref.as_ref().is_some_and(|p|p.slot_id==slot.id)
        &&crate::credential_refs::current(s,&a.project_id,&r.reference_id).is_some_and(|latest|latest.id==r.id));
    if !permitted{return Err("A referência deste provedor está ausente ou revogada no projeto.".into());}
    Ok(sensitivity==Sensitivity::Sensitive)
}
fn check_disclosure(s:&Snapshot,aid:&str,provider:ProviderId,prompt_digest:&str,acknowledge_sensitive:bool)->Result<(),String>{
    let a=lookup(s,aid)?;
    if s.data_policies.iter().any(|p|p.project_id==a.project_id)&&core::hash(a.objective.trim())!=prompt_digest{
        return Err("Este projeto usa classificação de dados. Use o objetivo revisado desta avaliação para a varredura.".into());
    }
    if check_data(s,aid,provider)?&&!acknowledge_sensitive{
        return Err("Confirme explicitamente o envio da fonte classificada como sensível ao provedor.".into());
    }
    Ok(())
}
fn approved_scope(s:&Snapshot,input:&Start)->Result<Scope,String>{
    let aid=&input.assessment_id;
    let a=lookup(s,aid)?;let p=s.projects.iter().find(|p|p.id==a.project_id&&!p.archived).ok_or("Projeto ausente ou arquivado.")?;
    if !matches!(a.kind.as_str(),"code"|"files")||!matches!(p.kind.as_str(),"code"|"files"){return Err("OpenKoda exige um repositório local de código ou arquivos.".into());}
    scope::check_new_local_input(s,aid)?;
    let body=scope_targets::current_body(s,aid)?;
    let refs=if let Some(set)=&body.target_set{
        if set.entries.len()!=1||!set.local_exclusions.is_empty()||!set.excluded_project_targets.is_empty(){return Err("O SDK precisa de uma seleção local única que corresponda às exclusões do escopo.".into());}
        vec![scope_targets::entry_ref(&set.entries[0])]
    }else{vec![]};
    for method in ["source.rules.v1","ai.remote-review.v1"]{
        scope::check_selected_work(s,aid,method,None,&refs)?;
    }
    let repo=PathBuf::from(&p.target);
    let mut locks=vec![crate::source_scan::lock_metadata_target(&repo)?];
    if !repo.is_dir(){return Err("Selecione a pasta do repositório como raiz do projeto.".into());}
    let target=Path::new(&a.target);
    let relative=target.strip_prefix(&repo).map_err(|_|"A avaliação não pertence à raiz deste projeto.")?;
    let mut paths=match &input.target{
        Some(Targets::One(v))=>vec![v.clone()],Some(Targets::Many(v)) if !v.is_empty()=>v.clone(),
        Some(_)=>return Err("Selecione pelo menos um caminho relativo.".into()),
        None if relative.as_os_str().is_empty()=>vec![],None=>vec![relative.to_string_lossy().into_owned()],
    };
    if paths.len()>64{return Err("Selecione no máximo 64 caminhos.".into());}
    let mut seen=HashSet::new();
    for path in &mut paths{
        *path=scope_targets::normalize_relative(path)?;
        if path.len()>512||path.contains(['[',']','{','}'])||!seen.insert(path.to_lowercase()){return Err("Caminho repetido, longo ou com expressão de seleção.".into());}
        let selected=repo.join(&*path);
        if !selected.starts_with(target){return Err("Seleção fora do alvo autorizado nesta avaliação.".into());}
        locks.push(crate::source_scan::lock_metadata_target(&selected)?);
    }
    let mut timeout=Duration::from_secs(2*60*60);
    if let Some(end)=&body.window.ends_at{let time=chrono::DateTime::parse_from_rfc3339(end).map_err(|_|"Janela de escopo inválida.")?;timeout=timeout.min((time.with_timezone(&chrono::Utc)-chrono::Utc::now()).to_std().map_err(|_|"Escopo expirado.")?);}
    Ok(Scope{repo,targets:paths,deadline:Instant::now()+timeout,_locks:locks})
}
fn check_repository_boundary(profile:&Path,repo:&Path,project:&model::Project)->Result<(),String>{
    let root=fs::canonicalize(profile).map_err(|_|"Perfil ausente.")?;
    let canonical_repo=fs::canonicalize(repo).map_err(|_|"Repositório ausente.")?;
    // The profile itself and its ancestors can never be an analysis root.
    // A descendant is allowed only for the exact managed GitHub checkout whose
    // ready import receipt still matches every persisted source identity field.
    if root.starts_with(&canonical_repo){return Err("O diretório de dados do aplicativo e o repositório da análise precisam ser separados.".into());}
    if canonical_repo.starts_with(&root){
        let declared=project.github_source.as_ref().ok_or("A pasta dentro do perfil não corresponde a uma importação GitHub revisada.")?;
        let attested=crate::github_repository::source(profile,&declared.import_id,&project.target)?;
        let target=fs::canonicalize(&project.target).map_err(|_|"A pasta importada não está disponível.")?;
        if canonical_repo!=target||attested.import_id!=declared.import_id||attested.url!=declared.url||attested.revision!=declared.revision{
            return Err("A identidade do repositório GitHub mudou. Crie uma nova importação antes de analisar.".into());
        }
    }
    Ok(())
}
impl Service {
    pub(crate) fn new(resources:&Path)->Self{
        let packaged=resources.join("security-runtime");
        #[cfg(debug_assertions)]
        let runtime=if packaged.join("cli.mjs").is_file(){packaged}else{PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../security-runtime")};
        #[cfg(not(debug_assertions))]
        let runtime=packaged;
        let bundled=runtime.join("bin/node.exe");
        let node=if bundled.is_file(){bundled}else{std::env::var_os("ProgramFiles").map(PathBuf::from).unwrap_or_else(||PathBuf::from(r"C:\Program Files")).join("nodejs/node.exe")};
        Self{node,helper:runtime.join("cli.mjs"),active:Mutex::new(None),check_cache:Mutex::new(None)}
    }
    fn command(&self)->Result<Command,String>{
        if !self.node.is_file()||!self.helper.is_file(){return Err("O runtime local do OpenKoda não está instalado.".into());}
        let mut cmd=Command::new(node_path(&self.node));
        cmd.env_clear();
        for name in ["SystemRoot","WINDIR","SystemDrive","TEMP","TMP","LOCALAPPDATA","APPDATA","ProgramFiles","ProgramFiles(x86)","ProgramW6432","USERPROFILE","PATH","PATHEXT","NUMBER_OF_PROCESSORS","PROCESSOR_ARCHITECTURE","COMSPEC"]{
            if let Some(v)=std::env::var_os(name){cmd.env(name,v);}
        }
        #[cfg(windows)] {use std::os::windows::process::CommandExt;cmd.creation_flags(0x08000000);}
        let helper=node_path(&self.helper);
        cmd.arg(&helper).current_dir(helper.parent().unwrap()).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null());
        Ok(cmd)
    }
    fn check(&self)->Value{
        let Ok(mut cache)=self.check_cache.lock()else{return json!({"available":false})};
        if let Some((at,v))=&*cache{if at.elapsed()<Duration::from_secs(15){return v.clone();}}
        let checked=(||->Result<Value,String>{
            let mut child=Managed::new(self.command()?.arg("--check").stdin(Stdio::null()).spawn().map_err(|_|"Node.js não iniciou.")?)?;
            let output=child.child.stdout.take().ok_or("Saída indisponível.")?;
            let rx=read_lines(output,64*1024);
            let deadline=Instant::now()+Duration::from_secs(45);
            let data=rx.recv_timeout(deadline.saturating_duration_since(Instant::now())).map_err(|err|match err{
                mpsc::RecvTimeoutError::Timeout=>"Tempo de verificação do runtime esgotado.",
                mpsc::RecvTimeoutError::Disconnected=>"O Node.js encerrou sem emitir o diagnóstico do runtime.",
            })??;
            let v:Value=serde_json::from_slice(&data).map_err(|_|"Resposta de diagnóstico do SDK inválida.")?;
            let exit=loop{
                if let Some(status)=child.child.try_wait().map_err(|_|"Falha no diagnóstico.")?{break status;}
                if Instant::now()>=deadline{return Err("Tempo de verificação do runtime esgotado.".into());}
                thread::sleep(Duration::from_millis(30));
            };
            if v["type"]!="check"||v["available"].as_bool()!=Some(exit.success()){return Err("Diagnóstico de runtime incompatível com o término do processo.".into());}
            Ok(v)
        })().unwrap_or_else(|e|json!({"available":false,"missing":[e]}));
        *cache=Some((Instant::now(),checked.clone()));checked
    }
    pub(crate) fn status(&self,store:&Store,ai:&AiService,payload:Value)->Result<Value,String>{
        let input:Status=decode(payload)?;
        let (provider,model,key)=ai.security_selection(store,input.assessment_id.as_deref())?;
        let runtime=self.check();
        let reason=if runtime["available"]!=true{Some("Confira os componentes do runtime OpenKoda.")}
            else if !matches!(provider,ProviderId::OpenAI|ProviderId::OpenRouter){Some("Selecione OpenAI ou OpenRouter para a análise SDK.")}
            else if !key{Some("Configure a chave deste provedor.")}
            else if !crate::ai_provider::valid_model(&model){Some("Selecione e salve um modelo.")}else{None};
        let sensitive=if let Some(aid)=input.assessment_id.as_deref(){check_data(&store.snapshot()?,aid,provider).unwrap_or(false)}else{false};
        Ok(json!({"available":reason.is_none(),"reason":reason,"runtime":runtime,"provider":provider,"model":model,"hasApiKey":key,"requiresSensitiveConfirmation":sensitive}))
    }
    pub(crate) fn get(&self,store:&Store,payload:Value)->Result<Value,String>{
        let q:Lookup=decode(payload)?;let s=store.snapshot()?;let a=lookup(&s,&q.assessment_id)?;
        latest(a).map_or(Ok(json!({"scan":null})),|r|public_checked(store,&s,a,r))
    }
    pub(crate) fn cancel(&self,store:&Store,payload:Value)->Result<Value,String>{
        let q:Lookup=decode(payload.clone())?;let active=self.active.lock().map_err(|_|"Estado do SDK indisponível.")?;
        if let Some(job)=active.as_ref().filter(|job|job.aid==q.assessment_id){
            let s=store.snapshot()?;let a=lookup(&s,&q.assessment_id)?;
            if let Some(r)=latest(a).filter(|r|r.active()){
                record(store,&q.assessment_id,&r.id,|r|{if r.active()&&r.status!="cancelling"{r.update("cancelling","cancel","Cancelamento da análise solicitado.");}Ok(())})?;
                job.cancel.store(true,Ordering::Release);
            }
        }
        drop(active);self.get(store,payload)
    }
    pub(crate) fn recover(store:&Store)->Result<(),String>{
        if !store.snapshot()?.assessments.iter().any(|a|a.events.iter().any(|e|e.security_scan.as_ref().is_some_and(Receipt::active))){return Ok(());}
        store.mutate(|s|{
            for a in &mut s.assessments{for e in &mut a.events{if let Some(r)=e.security_scan.as_mut().filter(|r|r.active()){
                r.update("interrupted","interrupted",INTERRUPTED);r.error=Some(INTERRUPTED.into());e.message=r.message.clone();e.kind="warning".into();a.updated_at=r.updated_at.clone();
            }}}
            Ok(())
        })
    }
    fn prepare<'a>(&self,store:&Store,ai:&'a AiService,input:Start)->Result<(Value,Option<Job<'a>>),String>{
        core::valid_id(&input.assessment_id)?;core::valid_id(&input.idempotency_key)?;
        let digest=core::hash(&serde_json::to_string(&input).map_err(|_|"Pedido inválido.")?);
        let mut active=self.active.lock().map_err(|_|"Estado da varredura indisponível.")?;
        let state=store.snapshot()?;let a=lookup(&state,&input.assessment_id)?;
        if let Some(r)=a.events.iter().filter_map(|e|e.security_scan.as_ref()).find(|r|r.idempotency_key==input.idempotency_key){
            if r.request_digest!=digest{return Err("A mesma chave de execução já representa outra solicitação.".into());}
            return Ok((public_checked(store,&state,a,r)?,None));
        }
        if active.is_some(){return Err("Já existe uma varredura OpenKoda em andamento.".into());}
        if !(0.01..=100.0).contains(&input.max_cost_usd){return Err("Informe um limite de US$ 0,01 a US$ 100.".into());}
        let prompt=input.prompt.as_deref().filter(|p|!p.trim().is_empty()).unwrap_or(&a.objective).trim();
        if prompt.len()>8192||prompt.contains('\0'){return Err("Descrição da varredura inválida.".into());}
        self.command()?;
        if self.check()["available"]!=true{return Err("O runtime local do OpenKoda está indisponível. Confira seu diagnóstico.".into());}
        let inference=ai.security_lease()?;
        let _gate=store.scope_gate.lock().map_err(|_|"Escopo indisponível.")?;
        let state=store.snapshot()?;let scope=approved_scope(&state,&input)?;
        let (provider,model,_)=ai.security_selection(store,Some(&input.assessment_id))?;
        let objective_digest=core::hash(prompt);
        check_disclosure(&state,&input.assessment_id,provider,&objective_digest,input.acknowledge_sensitive)?;
        let key=ai.security_key(store,&input.assessment_id,provider,&model)?;
        use crate::data_projection::{DataRedactor,RedactionMode};
        let redactor=ai.data_redactor();
        if redactor.mode()==RedactionMode::MetadataOnly{return Err("Credenciais indisponíveis para revisar o objetivo da varredura.".into());}
        let prompt=redactor.redact(prompt).text;
        let project=state.projects.iter().find(|p|p.id==a.project_id).ok_or("Projeto ausente.")?;
        check_repository_boundary(&store.root,&scope.repo,project)?;
        let id=model::id();let at=model::now();
        let mut r=Receipt{id:id.clone(),assessment_id:input.assessment_id.clone(),idempotency_key:input.idempotency_key,request_digest:digest,status:String::new(),stage:String::new(),message:String::new(),created_at:at.clone(),updated_at:at,provider:provider.id().into(),model:model.clone(),max_cost_usd:input.max_cost_usd,events:vec![],error:None,result_evidence_id:None,findings_total:None};
        r.update("starting","starting","Preparando o OpenKoda.");
        let parent=store.root.join("security-scans").join(&a.project_id);
        fs::create_dir_all(&parent).map_err(|_|"Não foi possível reservar a pasta de artefatos do SDK.")?;
        let output=parent.join(&id);
        fs::create_dir(&output).map_err(|_|"Não foi possível reservar a execução do SDK.")?;
        let mut scope=scope;
        scope._locks.push(crate::source_scan::lock_metadata_target(&output)?);
        ai.reserve_security(&id,&input.assessment_id,provider,&model,currency(input.max_cost_usd)?)?;
        let receipt=store.mutate(|s|{
            scope::check_new_local_input(s,&input.assessment_id)?;scope::freeze_review(s,&input.assessment_id,"security.scan.start")?;
            let a=s.assessments.iter_mut().find(|a|a.id==input.assessment_id).ok_or("Avaliação ausente.")?;
            let mut e=model::event(&r.message,"info");e.security_scan=Some(r.clone());a.events.push(e);
            a.updated_at=r.updated_at.clone();public(a,&r)
        });
        let receipt=match receipt{Ok(v)=>v,Err(e)=>{ai.security_not_sent(&id)?;return Err(e);}};
        let cancel=Arc::new(AtomicBool::new(false));
        *active=Some(Active{aid:input.assessment_id.clone(),cancel:cancel.clone()});
        Ok((receipt,Some(Job{id,aid:input.assessment_id,prompt,objective_digest,acknowledge_sensitive:input.acknowledge_sensitive,scope,provider,model,cap:input.max_cost_usd,key,output,cancel,_inference:inference})))
    }
    pub(crate) fn start_worker(&self,store:&Store,ai:&AiService,input:Value,ready:&mpsc::SyncSender<Result<Value,String>>)->Result<(),String>{
        let (receipt,job)=self.prepare(store,ai,decode(input)?)?;
        let _=ready.send(Ok(receipt));
        if let Some(job)=job{
            let mut dispatched=false;
            let result=self.drive(store,ai,&job,&mut dispatched);
            let mut terminal=result.unwrap_or_else(|e|Terminal::failure(&e));
            let estimate=terminal.data.as_ref().and_then(|d|d.cost.as_ref()).and_then(|c|c.estimated_usd).and_then(|v|currency(v).ok());
            let settled=if dispatched{ai.security_settle(&job.id,estimate)}else{ai.security_not_sent(&job.id)};
            if settled.is_err(){terminal.error=Some("A conciliação financeira da execução ainda está pendente.".into());}
            let saved=finish(store,&job.aid,&job.id,terminal);
            if let Ok(mut slot)=self.active.lock(){*slot=None;}
            saved?;
        }
        Ok(())
    }
    fn drive(&self,store:&Store,ai:&AiService,job:&Job,dispatched:&mut bool)->Result<Terminal,String>{
        let mut process=Managed::new(self.command()?.spawn().map_err(|_|"Não foi possível iniciar o Node.js do SDK.")?)?;
        let mut stdin=process.child.stdin.take().ok_or("Entrada do SDK indisponível.")?;
        let rx=read_lines(process.child.stdout.take().ok_or("Saída do SDK indisponível.")?,LIMIT);
        let cancel_before=||Terminal{status:"cancelled".into(),error:None,data:None};
        {
            let _scope=store.scope_gate.lock().map_err(|_|"Escopo indisponível.")?;
            let state=store.snapshot()?;scope::check_new_local_input(&state,&job.aid)?;check_disclosure(&state,&job.aid,job.provider,&job.objective_digest,job.acknowledge_sensitive)?;
            if ai.security_key(store,&job.aid,job.provider,&job.model)?.as_str()!=job.key.as_str(){return Err("A credencial selecionada mudou antes do envio.".into());}
            if job.cancel.load(Ordering::Acquire){return Ok(cancel_before());}
            // Record the dispatch intent before a single credential byte reaches
            // the helper. A partial pipe write cannot prove a remote no-op.
            record(store,&job.aid,&job.id,|r|{if r.status!="cancelling"{r.update("running","starting","Iniciando a análise do projeto.");}Ok(())})?;
            ai.security_dispatched(&job.id)?;*dispatched=true;
            #[derive(Serialize)]#[serde(rename_all="camelCase")]
            struct Wire<'a>{operation:&'a str,repository:&'a Path,output_dir:&'a Path,provider:&'a str,model:&'a str,api_key:&'a str,prompt:&'a str,max_cost_usd:f64,#[serde(skip_serializing_if="Option::is_none")]target:Option<&'a [String]>}
            let repository=node_path(&job.scope.repo);let output=node_path(&job.output);
            let wire=Wire{operation:"scan",repository:&repository,output_dir:&output,provider:job.provider.id(),model:&job.model,api_key:job.key.as_str(),prompt:&job.prompt,max_cost_usd:job.cap,target:(!job.scope.targets.is_empty()).then_some(&job.scope.targets)};
            let mut bytes=Zeroizing::new(serde_json::to_vec(&wire).map_err(|_|"Falha ao preparar o SDK.")?);
            bytes.push(b'\n');stdin.write_all(&bytes).and_then(|_|stdin.flush()).map_err(|_|INTERRUPTED)?;
        }
        let mut terminal=None;let mut closed=false;let mut cancelling=None;let mut last_scope_check=Instant::now();
        loop {
            let now=Instant::now();
            if last_scope_check.elapsed()>=Duration::from_secs(2){
                last_scope_check=now;let state=store.snapshot()?;
                if scope::check_new_local_input(&state,&job.aid).and_then(|_|check_disclosure(&state,&job.aid,job.provider,&job.objective_digest,job.acknowledge_sensitive)).is_err(){job.cancel.store(true,Ordering::Release);}
            }
            if cancelling.is_none()&&(job.cancel.load(Ordering::Acquire)||now>=job.scope.deadline){
                let _=stdin.write_all(b"{\"operation\":\"cancel\"}\n");let _=stdin.flush();cancelling=Some(now);
                record(store,&job.aid,&job.id,|r|{if r.status!="cancelling"{r.update("cancelling","cancel","Encerrando a análise em andamento.");}Ok(())})?;
            }
            if cancelling.is_some_and(|at:Instant|at.elapsed()>Duration::from_secs(20)){return Ok(Terminal::interrupted());}
            if !closed {match rx.recv_timeout(Duration::from_millis(100)){
                Ok(Ok(line))=>{
                    if terminal.is_some(){return Err(UNEXPECTED.into());}
                    let parsed:Frame=serde_json::from_slice(&line).map_err(|_|UNEXPECTED)?;
                    match parsed{
                        Frame::Progress{stage,message}=>{
                            let stage=clean(&stage,64,&job.key);let message=clean(&message,512,&job.key);
                            record(store,&job.aid,&job.id,|r|{if r.status=="running"{r.update("running",&stage,&message);}Ok(())})?;
                        }
                        Frame::Error{code,message}=>terminal=Some(Terminal::failure(&format!("{} · {}",clean(&code,64,&job.key),clean(&message,1000,&job.key)))),
                        Frame::Result{status,data}=>terminal=Some(validate_terminal(status,data,&job.key)?),
                    }
                }
                Ok(Err(e))=>return Err(e),Err(mpsc::RecvTimeoutError::Disconnected)=>closed=true,Err(mpsc::RecvTimeoutError::Timeout)=>{}
            }}else{thread::sleep(Duration::from_millis(50));}
            if closed{if let Some(exit)=process.child.try_wait().map_err(|_|INTERRUPTED)?{
                let t=terminal.ok_or(UNEXPECTED)?;
                if (t.status=="completed")!=exit.success(){return Err(UNEXPECTED.into());}
                return Ok(t);
            }}
        }
    }
}
fn clean(text:&str,max:usize,key:&str)->String{crate::ai::redact(text,Some(key)).chars().take(max).collect()}
fn validate_terminal(status:String,mut d:ResultData,key:&str)->Result<Terminal,String>{
    if !matches!(status.as_str(),"completed"|"cancelled"|"failed")||d.report.len()>256*1024||d.findings.len()>100
        ||d.coverage.scope.len()>128||d.coverage.excluded_paths.len()>1024||d.coverage.limitations.len()>128||d.cost.as_ref().and_then(|c|c.estimated_usd).is_some_and(|v|currency(v).is_err())
        ||status=="completed"&&(d.scan_id.as_deref().is_none_or(|v|v.trim().is_empty())||d.report.trim().is_empty()){return Err(UNEXPECTED.into());}
    d.scan_id=d.scan_id.map(|v|clean(&v,256,key));d.report=clean(&d.report,256*1024,key);
    for f in &mut d.findings{f.id=f.id.take().map(|v|clean(&v,256,key));f.title=clean(&f.title,512,key);f.description=clean(&f.description,8192,key);f.severity=clean(&f.severity,64,key);f.location=f.location.take().map(|v|clean(&v,2048,key));}
    for path in d.coverage.scope.iter_mut().chain(d.coverage.excluded_paths.iter_mut()){*path=clean(path,4096,key);}
    d.coverage.summary=clean(&d.coverage.summary,4096,key);
    for limitation in &mut d.coverage.limitations{*limitation=clean(limitation,2048,key);}
    let error=(status=="failed").then(||"O SDK encerrou a análise com falha.".into());
    Ok(Terminal{status,data:Some(d),error})
}
fn finish(store:&Store,aid:&str,id:&str,terminal:Terminal)->Result<(),String>{
    store.mutate(|s|{
        let a=s.assessments.iter_mut().find(|a|a.id==aid).ok_or("Avaliação ausente.")?;
        let e=a.events.iter_mut().find(|e|e.security_scan.as_ref().is_some_and(|r|r.id==id)).ok_or("Recibo ausente.")?;
        let r=e.security_scan.as_mut().unwrap();if !r.active(){return Err("Esta execução já terminou.".into());}
        if let Some(data)=terminal.data {
            let content=serde_json::to_string_pretty(&data).map_err(|_|"Relatório inválido.")?;
            let ev=Evidence{id:model::id(),name:"Relatório do OpenKoda".into(),sha256:core::hash(&content),content,created_at:model::now()};
            if terminal.status=="completed"{
                for (index,f) in data.findings.iter().enumerate(){
                    let severity=f.severity.to_lowercase();
                    // Unknown or informational severities stay verbatim in the
                    // source report. The domain has four risk levels.
                    if f.title.trim().is_empty()||!matches!(severity.as_str(),"low"|"medium"|"high"|"critical"){continue;}
                    let origin=f.id.clone().unwrap_or_else(||format!("{index}:{}",core::hash(&json!(f).to_string())));
                    let fid=scope_targets::derived_id("openkoda.codex-security.finding.v1",&[aid,id,&origin]);
                    if s.findings.iter().any(|f|f.id==fid){continue;}
                    let description=match f.location.as_deref(){Some(path)=>format!("{}\n\nLocal informado pelo SDK: {path}",f.description),None=>f.description.clone()};
                    s.findings.push(model::Finding{id:fid,assessment_id:aid.into(),title:f.title.clone(),severity,confidence:"candidate".into(),status:"candidate".into(),description,recommendation:String::new(),evidence_ids:vec![ev.id.clone()],created_at:model::now()});
                }
            }
            r.result_evidence_id=Some(ev.id.clone());r.findings_total=Some(data.coverage.findings_total);a.evidence.push(ev);
        }
        let message=terminal.error.as_deref().unwrap_or(match terminal.status.as_str(){"completed"=>"Análise concluída pelo OpenKoda.","cancelled"=>"Análise cancelada.",_=>INTERRUPTED});
        r.update(&terminal.status,&terminal.status,message);e.message=r.message.clone();
        e.kind=if r.status=="completed"{"success"}else{"warning"}.into();a.updated_at=r.updated_at.clone();r.error=terminal.error;Ok(())
    })
}
fn read_lines(reader:impl Read+Send+'static,limit:usize)->mpsc::Receiver<Result<Vec<u8>,String>>{
    let (tx,rx)=mpsc::sync_channel(8);
    thread::spawn(move||{
        let mut reader=BufReader::new(reader);
        loop{let mut line=Vec::new();
            match (&mut reader).take(limit as u64+1).read_until(b'\n',&mut line){
                Ok(0)=>break,
                Ok(n) if n<=limit&&line.last()==Some(&b'\n')=>{if tx.send(Ok(line)).is_err(){break;}},
                _=>{let _=tx.send(Err("Linha do SDK inválida ou maior que o limite.".into()));break;}
            }
        }
    });rx
}
struct Managed{
    child:Child,
    #[cfg(windows)] job:windows_sys::Win32::Foundation::HANDLE,
}
impl Managed{
    fn new(mut child:Child)->Result<Self,String>{
        #[cfg(windows)]{
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::{Foundation::CloseHandle,System::JobObjects::*};
            // The helper waits for stdin; no SDK work starts until after it
            // belongs to the job. Closing our last handle kills descendants.
            unsafe{
                let job=CreateJobObjectW(std::ptr::null(),std::ptr::null());
                let mut info:JOBOBJECT_EXTENDED_LIMIT_INFORMATION=std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if job.is_null()||SetInformationJobObject(job,JobObjectExtendedLimitInformation,(&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),std::mem::size_of_val(&info) as u32)==0||AssignProcessToJobObject(job,child.as_raw_handle())==0{
                    if !job.is_null(){CloseHandle(job);}let _=child.kill();let _=child.wait();
                    return Err("O Windows recusou o controle da árvore de processos do SDK.".into());
                }
                Ok(Self{child,job})
            }
        }
        #[cfg(not(windows))]{Ok(Self{child})}
    }
}
impl Drop for Managed{fn drop(&mut self){
    #[cfg(windows)] unsafe{windows_sys::Win32::Foundation::CloseHandle(self.job);}
    let _=self.child.kill();let _=self.child.wait();
}}

pub(crate) fn project(mut value:Value,redactor:&dyn crate::data_projection::DataRedactor)->Value{
    // The SDK report is already bounded by its wire schema. The generic
    // 2 KiB preview would silently discard most of a reviewed Markdown report.
    if redactor.mode()==crate::data_projection::RedactionMode::MetadataOnly{
        return crate::data_projection::project_value(value,redactor);
    }
    fn scrub(v:&mut Value,r:&dyn crate::data_projection::DataRedactor){match v{
        Value::String(s)=>*s=r.redact(s).text,
        Value::Array(rows)=>rows.iter_mut().for_each(|v|scrub(v,r)),
        Value::Object(o)=>o.values_mut().for_each(|v|scrub(v,r)),
        _=>{}
    }}
    scrub(&mut value,redactor);value
}

#[cfg(all(test,windows))]
mod tests {
    use super::*;
    const KEY:&str="fixture-security-key-not-for-a-provider";
    struct Fixture{dir:tempfile::TempDir,store:Store,ai:AiService,service:Service,aid:String}
    impl Fixture{
        fn new(relative:Option<&str>)->Self{
            Self::with_managed_source(relative,false)
        }
        fn with_managed_source(relative:Option<&str>,managed:bool)->Self{
            let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().join("profile")).unwrap();
            let import_id=managed.then(model::id);
            let repo=import_id.as_ref().map(|id|store.root.join("github-repositories").join(id).join("repository")).unwrap_or_else(||dir.path().join("repo"));
            fs::create_dir_all(repo.join("src")).unwrap();
            fs::write(repo.join("src/test.txt"),b"fixture").unwrap();fs::write(repo.join("outside.txt"),b"separate").unwrap();
            if let Some(id)=&import_id{
                let receipt=json!({"id":id,"url":"https://github.com/octocat/Hello-World","name":"Hello-World","status":"ready","message":"Synthetic ready import","revision":"a".repeat(40),"createdAt":model::now(),"updatedAt":model::now()});
                fs::write(repo.parent().unwrap().join("receipt.json"),serde_json::to_vec(&receipt).unwrap()).unwrap();
            }
            let ai=AiService::new_with_budget_root(store.root.clone(),store.root.clone()).unwrap();
            ai.handle("ai.key.save",json!({"key":KEY}),&store).unwrap();ai.save_settings(json!({"model":"fixture-model"}),&store).unwrap();
            let body=json!({"idempotencyKey":model::id(),"name":"Synthetic SDK","kind":"code","root":repo,"title":"Review","objective":"Local synthetic fixture","useRipgrep":false,"ports":[],"relativeTarget":relative,"githubImportId":import_id});
            let created=store.handle(core::Request{action:"project.launch".into(),payload:body}).unwrap();
            let aid=created["assessmentId"].as_str().unwrap().to_owned();
            let mut service=Service::new(dir.path());service.helper=dir.path().join("fixture.mjs");
            fs::write(&service.helper,r#"import {createInterface} from 'node:readline';import {writeFileSync} from 'node:fs';import {join} from 'node:path';
if(process.argv.includes('--check')){console.log(JSON.stringify({type:'check',available:true}));process.exit(0);}
const input=createInterface({input:process.stdin});let job;
input.on('line',l=>{const call=JSON.parse(l);
if(call.operation==='cancel'){console.log(JSON.stringify({type:'result',status:'cancelled',scanId:null,report:'',findings:[],coverage:{scope:[],excludedPaths:[],additionalExclusions:0,findingsTotal:0,reportTruncated:false},cost:null}));process.exit(1);}
job=call;if(process.argv.includes(job.apiKey)||Object.values(process.env).includes(job.apiKey))process.exit(9);
writeFileSync(join(job.outputDir,'fixture-started'),'1');console.log(JSON.stringify({type:'progress',stage:'review',message:'synthetic progress'}));
if(job.prompt==='wait')return;if(job.prompt==='missing-terminal')process.exit(0);
console.log(JSON.stringify({type:'result',status:'completed',scanId:'sdk-fixture-123',report:'Fixture report: '+job.apiKey,findings:[{id:'sdk-finding-1',title:'Fixture finding',severity:'high',description:'Fixture details',location:'src/test.txt:1'}],coverage:{scope:job.target||[],excludedPaths:[],additionalExclusions:0,findingsTotal:1,reportTruncated:false},cost:{estimatedUsd:0.0123}}));process.exit(0);});"#).unwrap();
            Self{dir,store,ai,service,aid}
        }
        fn input(&self,prompt:&str)->Value{json!({"assessmentId":self.aid,"idempotencyKey":model::id(),"prompt":prompt,"maxCostUsd":1.0})}
        fn run(&self,body:Value)->Value{let(tx,rx)=mpsc::sync_channel(1);self.service.start_worker(&self.store,&self.ai,body,&tx).unwrap();rx.recv().unwrap().unwrap()}
        fn get(&self)->Value{self.service.get(&self.store,json!({"assessmentId":self.aid})).unwrap()}
    }
    #[test]fn managed_github_checkout_runs_without_authorizing_profile_or_unattested_paths(){
        let f=Fixture::with_managed_source(None,true);
        let state=f.store.snapshot().unwrap();let project=&state.projects[0];let repo=Path::new(&project.target);
        check_repository_boundary(&f.store.root,repo,project).unwrap();
        for forbidden in [f.store.root.clone(),f.dir.path().to_path_buf(),f.store.root.join("github-repositories"),repo.join("src")]{
            assert!(check_repository_boundary(&f.store.root,&forbidden,project).is_err(),"{}",forbidden.display());
        }
        let mut changed=project.clone();changed.github_source=None;
        assert!(check_repository_boundary(&f.store.root,repo,&changed).is_err());
        for field in ["url","revision","importId"]{
            let mut changed=project.clone();let source=changed.github_source.as_mut().unwrap();
            match field{"url"=>source.url="https://github.com/octocat/Other".into(),"revision"=>source.revision="b".repeat(40),_=>source.import_id=model::id()};
            assert!(check_repository_boundary(&f.store.root,repo,&changed).is_err(),"{field}");
        }
        f.run(f.input("complete"));assert_eq!(f.get()["scan"]["status"],"completed");
        assert_eq!(f.store.snapshot().unwrap().findings.len(),1);
    }
    #[test]fn stored_report_respects_local_read_policy_and_revocation_on_get_and_replay(){
        use crate::data_policy::{DataPolicyService,test_support::TestRedactor};
        use crate::data_projection::RejectExternal;
        let f=Fixture::new(None);let input=f.input("complete");f.run(input.clone());
        f.ai.test_data_authority(&f.store,&f.aid);
        assert!(f.get()["scan"]["result"]["report"].as_str().unwrap().contains("Fixture report"));
        let policy=DataPolicyService::new();let state=f.store.snapshot().unwrap();let pid=state.projects[0].id.clone();
        let call=|action:&str,payload:Value|policy.handle(action,payload,&f.store,&TestRedactor,&RejectExternal,None).unwrap();
        let status=call("data.policy.status",json!({"projectId":pid}));
        let mut body=state.data_policy_revisions.last().unwrap().body.clone();body.local_raw_view.enabled=false;
        let review=call("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":body}));
        call("data.policy.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}));
        let blocked=f.get();assert!(blocked["scan"]["result"].is_null(),"{blocked}");
        assert_eq!(blocked["scan"]["resultUnavailable"],true);
        assert!(!blocked.to_string().contains("Fixture report"));
        assert!(f.run(input.clone())["scan"]["result"].is_null());
        let status=call("data.policy.status",json!({"projectId":pid}));
        let mut allowed=state.data_policy_revisions.last().unwrap().body.clone();allowed.local_raw_view.enabled=true;
        let review=call("data.policy.prepare",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"body":allowed}));
        call("data.policy.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}));
        assert!(f.get()["scan"]["result"]["report"].as_str().unwrap().contains("Fixture report"));
        let status=call("data.policy.status",json!({"projectId":pid}));
        call("data.policy.revoke",json!({"projectId":pid,"expectedStateDigest":status["stateDigest"],"idempotencyKey":model::id(),"acknowledgeRevocation":true}));
        assert!(f.get()["scan"]["result"].is_null());assert!(f.run(input)["scan"]["result"].is_null());
        let stored=f.store.snapshot().unwrap();assert!(stored.assessments[0].evidence[0].content.contains("Fixture report"));
        assert_eq!(stored.findings.len(),1);
    }
    #[test]fn synthetic_sdk_receipt_import_budget_and_retry(){
        let f=Fixture::new(None);let body=f.input("complete");assert!(f.get()["scan"].is_null());
        f.run(body.clone());let stored=f.get();let scan=&stored["scan"];
        assert_eq!(scan["status"],"completed","{scan}");assert_eq!(scan["findingsTotal"],1);
        assert_eq!(scan["result"]["scanId"],"sdk-fixture-123");assert!(!stored.to_string().contains(KEY));
        let state=f.store.snapshot().unwrap();assert_eq!(state.findings.len(),1);assert_eq!(state.findings[0].status,"candidate");
        assert_eq!(state.findings[0].evidence_ids[0],scan["resultEvidenceId"]);assert_eq!(state.assessments[0].evidence.len(),1);
        let ledger=f.ai.handle("ai.budget.status",json!({"assessmentId":f.aid}),&f.store).unwrap();
        assert_eq!(ledger["recentChargesTotal"],1);assert_eq!(ledger["recentCharges"][0]["assumedChargeUsd"].as_str().unwrap().parse::<f64>().unwrap(),0.0123);
        assert_eq!(f.run(body.clone()),stored);let mut changed=body;changed["prompt"]=json!("changed");
        assert!(f.service.prepare(&f.store,&f.ai,decode(changed).unwrap()).is_err());
        assert_eq!(f.store.snapshot().unwrap().findings.len(),1);
        assert!(f.dir.path().join("profile/security-scans").is_dir());
    }
    #[test]fn verbatim_tauri_paths_launch_node_and_invalid_exit_cannot_mark_available(){
        let mut f=Fixture::new(None);
        f.service.helper=f.service.helper.canonicalize().unwrap();
        f.service.node=f.service.node.canonicalize().unwrap();
        assert_eq!(f.service.check()["available"],true);
        assert_eq!(node_path(Path::new(r"\\?\C:\Área privada\openkoda")),Path::new(r"C:\Área privada\openkoda"));
        *f.service.check_cache.lock().unwrap()=None;
        fs::write(&f.service.helper,"console.log(JSON.stringify({type:'check',available:true}));process.exit(1);").unwrap();
        assert_eq!(f.service.check()["available"],false);
        *f.service.check_cache.lock().unwrap()=None;
        fs::write(&f.service.helper,"process.exit(1);").unwrap();
        assert!(f.service.check()["missing"][0].as_str().unwrap().contains("sem emitir"));
    }
    #[test]fn empty_exit_cannot_confirm_and_unknown_charge_remains(){
        let f=Fixture::new(None);f.run(f.input("missing-terminal"));assert_eq!(f.get()["scan"]["status"],"failed");
        assert!(f.store.snapshot().unwrap().findings.is_empty());
        let ledger=f.ai.handle("ai.budget.status",json!({}),&f.store).unwrap();assert_eq!(ledger["unknownChargesTotal"],1);
    }
    #[test]fn cancellation_waits_for_terminal_and_never_invents_results(){
        let f=Fixture::new(None);let p=f.input("wait");
        thread::scope(|t|{
            let(tx,rx)=mpsc::sync_channel(1);let f=&f;t.spawn(move||f.service.start_worker(&f.store,&f.ai,p,&tx).unwrap());rx.recv().unwrap().unwrap();
            let deadline=Instant::now()+Duration::from_secs(15);
            while f.get()["scan"]["stage"]!="review"{assert!(Instant::now()<deadline,"{}",f.get());thread::sleep(Duration::from_millis(25));}
            assert!(f.ai.handle("ai.key.remove",json!({}),&f.store).is_err());
            let pending=f.service.cancel(&f.store,json!({"assessmentId":f.aid})).unwrap();
            assert!(matches!(pending["scan"]["status"].as_str(),Some("cancelling"|"cancelled")));
        });assert_eq!(f.get()["scan"]["status"],"cancelled");assert!(f.store.snapshot().unwrap().findings.is_empty());
    }
    #[test]fn subpath_cannot_broaden_and_restart_never_replays(){
        let f=Fixture::new(Some("src"));let mut denied=f.input("complete");denied["target"]=json!("outside.txt");
        assert!(f.service.prepare(&f.store,&f.ai,decode(denied).unwrap()).is_err());assert!(f.get()["scan"].is_null());
        let body=f.input("wait");let (_,job)=f.service.prepare(&f.store,&f.ai,decode(body.clone()).unwrap()).unwrap();assert_eq!(job.as_ref().unwrap().scope.targets,vec!["src"]);
        drop(job);Service::recover(&f.store).unwrap();assert_eq!(f.get()["scan"]["status"],"interrupted");
        let again=Service::new(f.dir.path());let (_,job)=again.prepare(&f.store,&f.ai,decode(body).unwrap()).unwrap();assert!(job.is_none());
    }
    #[test]fn terminal_contract_rejects_false_completion_and_overlong_lines(){
        let frame=json!({"type":"result","status":"completed","scanId":null,"report":"","findings":[],"coverage":{"scope":[],"excludedPaths":[],"additionalExclusions":0,"findingsTotal":0,"reportTruncated":false},"cost":null});
        let Frame::Result{status,data}=serde_json::from_value(frame).unwrap()else{panic!()};assert!(validate_terminal(status,data,KEY).is_err());
        assert!(read_lines(&b"overlong\n"[..],3).recv().unwrap().is_err());
    }
    #[test]fn coverage_keeps_partial_scope_and_redacts_secrets(){
        let raw=json!({"scanId":"known","report":"Relatório parcial","findings":[],"coverage":{"scope":[],"excludedPaths":[],"additionalExclusions":0,"findingsTotal":0,"reportTruncated":false,"summary":"39 de 252 arquivos revisados","limitations":[format!("213 arquivos fora da revisão; {}",KEY)]},"cost":null});
        let data=validate_terminal("completed".into(),serde_json::from_value(raw).unwrap(),KEY).unwrap().data.unwrap();
        assert_eq!(data.coverage.summary,"39 de 252 arquivos revisados");
        assert!(data.coverage.limitations[0].contains("213 arquivos"));assert!(!data.coverage.limitations[0].contains(KEY));
    }
    #[test]fn existing_chat_policy_controls_project_source_and_revocation(){
        let f=Fixture::new(None);f.ai.test_data_authority(&f.store,&f.aid);
        let mut s=f.store.snapshot().unwrap();check_data(&s,&f.aid,ProviderId::OpenAI).unwrap();
        let objective=core::hash(s.assessments[0].objective.trim());
        assert!(check_disclosure(&s,&f.aid,ProviderId::OpenAI,&core::hash("other text"),true).is_err());
        let mut sensitive=s.clone();
        for c in &mut sensitive.data_classifications{c.sensitivity=crate::data_policy::Sensitivity::Sensitive;}
        assert!(check_disclosure(&sensitive,&f.aid,ProviderId::OpenAI,&objective,false).is_err());
        assert!(check_disclosure(&sensitive,&f.aid,ProviderId::OpenAI,&objective,true).is_ok());
        s.data_policies[0].active=false;assert!(check_data(&s,&f.aid,ProviderId::OpenAI).is_err());s.data_policies[0].active=true;
        let mut changed=s.clone();changed.data_bindings.clear();assert!(check_data(&changed,&f.aid,ProviderId::OpenAI).is_err());
        let mut changed=s.clone();for r in &mut changed.credential_references{r.revoked=true;}assert!(check_data(&changed,&f.aid,ProviderId::OpenAI).is_err());
        let remote=s.data_policy_revisions[0].body.destinations.iter_mut().find(|d|d.destination==crate::data_policy::DataDestination::RemoteAi).unwrap();
        remote.categories.retain(|c|*c!=crate::data_policy::DataCategory::SourceExcerpt);
        assert!(check_data(&s,&f.aid,ProviderId::OpenAI).is_err());
    }
}
