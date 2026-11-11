//! Deliberately narrow native adapters. Never executes a target program or an arbitrary shell.
use crate::{core::{self, Store}, model::*};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use base64::{engine::general_purpose::STANDARD,Engine};
use std::{collections::HashMap, fs, io::Read, net::ToSocketAddrs, path::Path,
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};

const MAX_FILES: usize = 1000;
const MAX_INPUT: u64 = 16 * 1024 * 1024;
const ADAPTER_LOCAL: &str = "local.inventory.v1";
const ADAPTER_HTTP: &str = "http.headers.v1";

#[derive(Default)]
pub struct Executor { running: Mutex<HashMap<String, Arc<AtomicBool>>>, arbiter:Arc<crate::work_supervisor::WorkArbiter> }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IdInput { id: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DecisionInput { id: String, digest: String }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NetworkInput { id:String, ports:Vec<u16> }
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct TargetPlanInput {assessment_id:String,scope_ref:crate::scope::ScopeRef,target_ref:crate::scope_targets::ScopeTargetRef,adapter:String,ports:Option<Vec<u16>>}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ReconcileInput { id:String, note:String, acknowledge_unknown_effects:bool }

/// Host-only inputs: the workflow resolves these from an immutable action node.
/// This type is deliberately not deserializable from IPC.
pub(crate) struct NativeCompileInput {
    pub assessment_id:String,
    pub adapter:String,
    pub scope_ref:crate::scope::ScopeRef,
    pub target_ref:Option<crate::scope_targets::ScopeTargetRef>,
    pub ports:Option<Vec<u16>>,
}

/// Canonical finalization facts for the work journal. No bytes enter work DTOs.
pub(crate) struct NativeCompletion {
    pub execution:Execution,
    pub evidence:Option<Evidence>,
    pub partial:bool,
}

fn decode<T: serde::de::DeserializeOwned>(payload: Value) -> Result<T,String> {
    serde_json::from_value(payload).map_err(|_| "Campos inválidos na operação.".into())
}
fn value<T: serde::Serialize>(input: T) -> Result<Value,String> {
    serde_json::to_value(input).map_err(|_| "Falha ao serializar a operação.".into())
}
pub fn plan_digest(p: &ExecutionPlan) -> String {
    let mut contents=json!({"id":p.id,"assessmentId":p.assessment_id,"adapter":p.adapter,
        "target":p.target,"timeoutSeconds":p.timeout_seconds,"outputLimitKiB":p.output_limit_ki_b,
        "inputFiles":p.input_files,"resolvedAddresses":p.resolved_addresses,
        "createdAt":p.created_at,"expiresAt":p.expires_at});
    if let Some(scope)=&p.source_scope {contents["sourceScope"]=json!(scope);}
    if let Some(scope)=&p.scope_ref {contents["scopeRef"]=json!(scope);}
    if let Some(target)=&p.target_ref{contents["targetRef"]=json!(target);}
    if let Some(ports)=&p.requested_ports{contents["requestedPorts"]=json!(ports);}
    if !p.identity_preflight_refs.is_empty(){contents["identityPreflightRefs"]=json!(p.identity_preflight_refs);}
    if !p.scope_omissions.is_empty(){contents["scopeOmissions"]=json!(p.scope_omissions);}
    core::hash(&contents.to_string())
}
pub fn supports(action:&str)->bool {
    matches!(action,"assessment.start"|"assessment.plan"|"assessment.networkPlan"|"assessment.sourcePlan"|"assessment.targetPlan"|"assessment.cancel"|"assessment.reconcile"|"plan.approve"|"plan.revoke"|"plan.run")
}

pub fn validate_state(s:&Snapshot)->Result<(),String>{
    let mut ids=std::collections::HashSet::new();
    for p in &s.plans {
        core::valid_id(&p.id)?;
        let a=s.assessments.iter().find(|a|a.id==p.assessment_id).ok_or("Plano sem avaliação.")?;
        let selected=plan_target(s,p)?;let kind=selected.as_ref().map_or(a.kind.as_str(),|target|target.kind.as_str());
        if !ids.insert(&p.id)||p.digest!=plan_digest(p)||![ADAPTER_LOCAL,ADAPTER_HTTP,crate::network::ADAPTER,crate::source_scan::ADAPTER].contains(&p.adapter.as_str()) {return Err("Plano inconsistente; dados preservados.".into());}
        if !["proposed","approved","running","canceling","revoked","canceled","completed","failed","unknown"].contains(&p.status.as_str()) || !(10..=3600).contains(&p.timeout_seconds) || !(16..=4096).contains(&p.output_limit_ki_b) {return Err("Estado ou limites inválidos no plano.".into());}
        chrono::DateTime::parse_from_rfc3339(&p.created_at).map_err(|_|"Data do plano inválida.")?;
        chrono::DateTime::parse_from_rfc3339(&p.expires_at).map_err(|_|"Validade do plano inválida.")?;
        if let Some(at)=&p.approved_at {chrono::DateTime::parse_from_rfc3339(at).map_err(|_|"Data da aprovação inválida.")?;}
        if ["approved","running","completed"].contains(&p.status.as_str())&&p.approved_at.is_none(){return Err("Plano autorizado sem decisão humana.".into());}
        if p.adapter==ADAPTER_LOCAL && (!p.resolved_addresses.is_empty()||!["code","files"].contains(&kind)||p.input_files.len()>MAX_FILES||p.input_files.iter().fold(0u64,|sum,f|sum.saturating_add(f.size))>MAX_INPUT){return Err("Entradas locais do plano inválidas.".into());}
        if p.adapter==ADAPTER_HTTP && (kind!="web"||!p.input_files.is_empty()||p.resolved_addresses.is_empty()||p.resolved_addresses.len()>32||p.resolved_addresses.iter().any(|addr|addr.parse::<std::net::SocketAddr>().is_err())){return Err("Destinos HTTP do plano inválidos.".into());}
        if p.adapter==crate::source_scan::ADAPTER||(p.adapter==ADAPTER_LOCAL&&p.target_ref.is_some()) {
            if !["code","files"].contains(&kind)||!p.resolved_addresses.is_empty(){return Err("Modalidade do plano estático inválida.".into());}
            crate::source_scan::validate(Path::new(&p.target),&p.input_files,p.source_scope.as_ref().ok_or("Plano estático sem manifest de identidade.")?)?;
        } else if p.source_scope.is_some(){return Err("Manifest estático em adaptador incompatível.".into());}
        if p.adapter==crate::network::ADAPTER {
            if kind!="network"||!p.input_files.is_empty(){return Err("Modalidade do plano TCP inválida.".into());}
            if p.target_ref.is_some(){crate::network::validate_addresses_selected(&p.target,p.requested_ports.as_deref().ok_or("Plano TCP combinado sem portas originais.")?,&p.resolved_addresses,&selected.as_ref().ok_or("Alvo combinado ausente.")?.connection_exclusions)?;}
            else{crate::network::validate_addresses(&p.target,&p.resolved_addresses)?;}
        }
        validate_selected_plan(s,p,selected.as_ref())?;
    }
    ids.clear();
    for e in &s.executions {
        core::valid_id(&e.id)?;
        let p=s.plans.iter().find(|p|p.id==e.plan_id&&p.assessment_id==e.assessment_id).ok_or("Execução sem plano correspondente.")?;
        if p.scope_ref.is_some()&&e.scope_admission.is_none(){return Err("Execução com escopo sem recibo de admissão.".into());}
        if let Some(receipt)=&e.scope_admission{crate::scope::validate_historical_ref(s,&e.assessment_id,&receipt.scope_ref)?;
            if p.scope_ref.as_ref()!=Some(&receipt.scope_ref)||receipt.target_ref!=p.target_ref||receipt.identity_preflight_refs!=p.identity_preflight_refs||receipt.configured_timeout_ms!=p.timeout_seconds as u64*1000||receipt.effective_timeout_ms==0||receipt.effective_timeout_ms>receipt.configured_timeout_ms||receipt.limiting_factor!=if receipt.effective_timeout_ms<receipt.configured_timeout_ms{"scope_window_end"}else{"plan_timeout"}||chrono::DateTime::parse_from_rfc3339(&receipt.admitted_at).is_err(){return Err("Recibo de admissão do escopo inconsistente.".into());}}
        if !ids.insert(&e.id)||!["running","canceling","completed","canceled","failed","unknown"].contains(&e.status.as_str()){return Err("Execução inconsistente.".into());}
        if let Some(eid)=&e.evidence_id {if !s.assessments.iter().any(|a|a.id==p.assessment_id&&a.evidence.iter().any(|v|v.id==*eid)){return Err("Execução referencia evidência inválida.".into());}}
        if ["completed","canceled","failed"].contains(&e.status.as_str())&&e.finished_at.is_none(){return Err("Execução encerrada sem confirmação de término.".into());}
        if e.status=="completed"&&e.evidence_id.is_none(){return Err("Execução concluída sem evidência.".into());}
    }Ok(())
}

/// Resolve provenance without checking today's clock or revocation. Historical
/// evidence remains readable; this function never authorizes another dispatch.
pub(crate) fn plan_target(s:&Snapshot,p:&ExecutionPlan)->Result<Option<crate::scope_targets::ResolvedTarget>,String>{
    let assessment=s.assessments.iter().find(|a|a.id==p.assessment_id).ok_or("Plano sem avaliação.")?;
    if let Some(reference)=&p.scope_ref{
        let selected=crate::scope_targets::resolve_historical_target(s,reference,p.target_ref.as_ref())?;
        if reference.assessment_id!=p.assessment_id||selected.target!=p.target{return Err("Plano associado a outro alvo ou avaliação.".into());}
        let revision=s.scope_revisions.iter().find(|revision|revision.id==reference.revision_id).ok_or("Revisão ausente.")?;
        let methods=if let Some(target)=&p.target_ref{&revision.body.target_set.as_ref().ok_or("Referência extra em escopo simples.")?.entries.iter().find(|entry|entry.entry_id==target.entry_id).ok_or("Entrada ausente.")?.methods}else{&revision.body.methods};
        if !methods.contains(&p.adapter){return Err("Plano excede os métodos da entrada revisada.".into());}
        Ok(Some(selected))
    }else{
        if p.target_ref.is_some()||p.target!=assessment.target||s.scope_revisions.iter().any(|revision|revision.assessment_id==assessment.id&&revision.body.target_set.is_some()){return Err("Plano legado não pode representar entrada combinada.".into());}
        Ok(None)
    }
}

fn validate_selected_plan(s:&Snapshot,p:&ExecutionPlan,selected:Option<&crate::scope_targets::ResolvedTarget>)->Result<(),String>{
    if p.target_ref.is_none(){if p.requested_ports.is_some()||!p.identity_preflight_refs.is_empty()||!p.scope_omissions.is_empty(){return Err("Metadados combinados em plano simples.".into());}return Ok(());}
    let selected=selected.ok_or("Entrada de plano combinado ausente.")?;
    let reference=p.scope_ref.as_ref().ok_or("Plano combinado sem revisão.")?;
    if p.scope_omissions.len()>5032{return Err("Omissões de plano excedem o limite.".into());}
    let mut keys=std::collections::HashSet::new();
    for omitted in &p.scope_omissions{
        if !keys.insert((&omitted.kind,&omitted.value)){return Err("Omissão duplicada no plano.".into());}
        let expected=match omitted.kind.as_str(){
            "local"=>{if crate::scope_targets::normalize_relative(&omitted.value)?!=omitted.value{return Err("Omissão local não canônica.".into());}crate::scope_targets::local_excluded(&selected.local_exclusions,&omitted.value).map(|rule|crate::scope_targets::local_omission(rule,&omitted.value))},
            "connection"=>{let peer=omitted.value.parse::<std::net::SocketAddr>().map_err(|_|"Omissão de conexão inválida.")?;crate::scope_targets::connection_excluded(&selected.connection_exclusions,peer).map(|rule|crate::scope_targets::connection_omission(rule,peer))},
            _=>None,
        }.ok_or("Omissão sem regra na revisão.")?;
        if serde_json::to_value(expected).map_err(|_|"Omissão inválida.")?!=serde_json::to_value(omitted).map_err(|_|"Omissão inválida.")?{return Err("Motivo ou digest de omissão divergente.".into());}
    }
    if matches!(selected.kind.as_str(),"code"|"files"){
        crate::local_scope::validate_refs(s,reference,p.target_ref.as_ref(),&p.identity_preflight_refs)?;
        crate::source_scan::validate_selected_paths(&p.input_files,&selected.local_exclusions)?;
        if p.requested_ports.is_some()||p.scope_omissions.iter().any(|omitted|omitted.kind!="local"){return Err("Política de plano local incompatível.".into());}
    }else{
        if !p.identity_preflight_refs.is_empty()||p.scope_omissions.iter().any(|omitted|omitted.kind!="connection"){return Err("Preflight local em conexão não autorizado.".into());}
        for value in &p.resolved_addresses{let peer=value.parse::<std::net::SocketAddr>().map_err(|_|"Endpoint inválido.")?;if crate::scope_targets::connection_excluded(&selected.connection_exclusions,peer).is_some(){return Err("Plano contém conexão excluída.".into());}}
        if p.adapter==crate::network::ADAPTER{
            let (expected,omitted)=crate::network::plan_addresses_selected(&p.target,p.requested_ports.as_deref().ok_or("Portas originais ausentes.")?,&selected.connection_exclusions)?;
            if expected!=p.resolved_addresses||serde_json::to_value(omitted).map_err(|_|"Omissões inválidas.")?!=serde_json::to_value(&p.scope_omissions).map_err(|_|"Omissões inválidas.")?{return Err("Produto TCP diverge da revisão.".into());}
        }else if p.requested_ports.is_some(){return Err("Portas TCP extras no plano HTTP.".into());}
        if p.adapter==ADAPTER_HTTP{
            validate_http_peers(&p.target,&p.resolved_addresses,&selected.connection_exclusions)?;
        }
    }
    Ok(())
}

fn validate_http_peers(target:&str,addresses:&[String],rules:&[crate::scope_targets::ConnectionExclusion])->Result<(),String>{
    let url=url::Url::parse(target).map_err(|_|"URL inválida.")?;let port=url.port_or_known_default().ok_or("Porta HTTP ausente.")?;let mut distinct=std::collections::HashSet::new();
    if addresses.is_empty()||addresses.len()>32{return Err("Peers HTTP vazios ou acima do limite.".into());}
    for value in addresses{let peer=value.parse::<std::net::SocketAddr>().map_err(|_|"Peer HTTP inválido.")?;if peer.port()!=port||!distinct.insert(peer)||matches!(peer.ip(),std::net::IpAddr::V6(ip) if ip.to_ipv4_mapped().is_some())||crate::scope_targets::connection_excluded(rules,peer).is_some(){return Err("Plano HTTP contém conexão excluída ou fora da origem.".into());}}
    // Hyper may bypass custom DNS for an IP-literal URL. Consequently a forged
    // peer list cannot replace that literal and bypass a connection exclusion.
    if let Ok(ip)=url.host_str().ok_or("Host HTTP ausente.")?.trim_matches(['[',']']).parse::<std::net::IpAddr>(){if addresses!=[std::net::SocketAddr::new(ip,port).to_string()]{return Err("Peer HTTP difere do IP literal da URL.".into());}}
    Ok(())
}

fn filter_http_addresses(candidates:&[String],rules:&[crate::scope_targets::ConnectionExclusion])->Result<(Vec<String>,Vec<crate::scope_targets::ScopeOmission>),String>{
    if candidates.is_empty()||candidates.len()>32{return Err("Resolução HTTP acima do limite antes de exclusões.".into());}
    let mut allowed=vec![];let mut omitted=vec![];
    for value in candidates{
        let peer=value.parse::<std::net::SocketAddr>().map_err(|_|"Endpoint HTTP inválido.")?;
        if matches!(peer.ip(),std::net::IpAddr::V6(ip) if ip.to_ipv4_mapped().is_some()){return Err("IPv4 mapeado não é aceito no plano HTTP combinado.".into());}
        if let Some(rule)=crate::scope_targets::connection_excluded(rules,peer){omitted.push(crate::scope_targets::connection_omission(rule,peer));}else{allowed.push(value.clone());}
    }
    if allowed.is_empty(){return Err("scope_all_connections_excluded: nenhum destino HTTP permitido.".into());}
    Ok((allowed,omitted))
}

impl Executor {
    pub(crate) fn with_arbiter(mut self,arbiter:Arc<crate::work_supervisor::WorkArbiter>)->Self {self.arbiter=arbiter;self}
    pub(crate) fn signal_cancel(&self,assessment_id:&str)->Result<(),String> {
        let running=self.running.lock().map_err(|_|"Executor indisponível.")?;
        if let Some(token)=running.get(assessment_id){token.store(true,Ordering::SeqCst);}Ok(())
    }
    pub fn handle(&self, action:&str, payload:Value, store:&Store)->Result<Value,String> {
        if !payload.is_object() || payload.to_string().len()>4096 {return Err("Solicitação de operação inválida.".into());}
        match action {
            "assessment.start"|"assessment.plan"=>self.prepare(store,decode::<IdInput>(payload)?.id),
            "assessment.networkPlan"=>{let input:NetworkInput=decode(payload)?;self.prepare_with_options(store,input.id,Some(input.ports),false)},
            "assessment.sourcePlan"=>self.prepare_with_options(store,decode::<IdInput>(payload)?.id,None,true),
            "assessment.targetPlan"=>{
                let input:TargetPlanInput=decode(payload)?;
                if ![ADAPTER_LOCAL,ADAPTER_HTTP,crate::network::ADAPTER,crate::source_scan::ADAPTER].contains(&input.adapter.as_str())||(input.adapter==crate::network::ADAPTER)!=input.ports.is_some(){return Err("Adaptador ou portas incompatíveis com o plano de alvo.".into());}
                self.prepare_selected(store,input.assessment_id,input.ports,input.adapter==crate::source_scan::ADAPTER,Some(input.target_ref),Some(input.scope_ref),Some(input.adapter))
            },
            "plan.approve"=>{let d=decode::<DecisionInput>(payload)?;self.approve(store,d)},
            "plan.revoke"=>{let d=decode::<DecisionInput>(payload)?;self.revoke(store,d)},
            "plan.run"=>{let d=decode::<DecisionInput>(payload)?;self.run(store,d)},
            "assessment.cancel"=>self.cancel(store,decode::<IdInput>(payload)?.id),
            "assessment.reconcile"=>self.reconcile(store,decode::<ReconcileInput>(payload)?),
            _=>Err("Operação desconhecida.".into())
        }
    }

    fn reconcile(&self,store:&Store,input:ReconcileInput)->Result<Value,String>{
        core::valid_id(&input.id)?;
        let note=input.note.trim();
        if !input.acknowledge_unknown_effects||note.is_empty()||note.chars().count()>2000||note.chars().any(|c|c.is_control()&&c!='\n'&&c!='\t'){return Err("Registre a inspeção e confirme que efeitos externos desconhecidos não foram considerados desfeitos.".into());}
        let running=self.running.lock().map_err(|_|"Supervisor indisponível.")?;
        if running.contains_key(&input.id){return Err("Há um adaptador ativo. Solicite cancelamento antes de reconciliar.".into());}
        store.work_transaction(|conn,s|{
            for execution in s.executions.iter().filter(|e|e.assessment_id==input.id) {
                if !s.plans.iter().any(|p|p.id==execution.plan_id&&[ADAPTER_LOCAL,ADAPTER_HTTP,crate::network::ADAPTER,crate::source_scan::ADAPTER].contains(&p.adapter.as_str())){return Err("Esta execução exige um reconciliador específico para seu backend.".into());}
            }
            let a=s.assessments.iter_mut().find(|a|a.id==input.id).ok_or("Avaliação não encontrada.")?;
            if a.status!="unknown" {return Err("Esta avaliação não tem interrupção pendente de reconciliação.".into());}
            // The installation lock proves no other live OpenKoda process owns this
            // store. These adapters create no subprocesses; remote effects remain unknown.
            let message=format!("Reconciliação humana: {note}. Supervisor nativo anterior encerrado; efeitos externos não confirmados permanecem desconhecidos. Nenhuma tentativa foi repetida.");
            a.status=if a.evidence.is_empty(){"failed"}else{"partial"}.into();a.summary=message.clone();a.updated_at=now();a.events.push(event(message.clone(),"warning"));
            for e in s.executions.iter_mut().filter(|e|e.assessment_id==input.id&&e.status=="unknown") {e.status="failed".into();e.finished_at=Some(now());e.message=message.clone();}
            for p in s.plans.iter_mut().filter(|p|p.assessment_id==input.id&&p.status=="unknown") {p.status="failed".into();}
            let result=a.clone();crate::work_supervisor::sync_native_reconcile_locked(conn,s,&input.id)?;value(result)
        })
    }

    fn prepare(&self,store:&Store,assessment_id:String)->Result<Value,String> {
        self.prepare_with_options(store,assessment_id,None,false)
    }
    fn prepare_with_options(&self,store:&Store,assessment_id:String,ports:Option<Vec<u16>>,source:bool)->Result<Value,String> {
        self.prepare_selected(store,assessment_id,ports,source,None,None,None)
    }
    fn prepare_selected(&self,store:&Store,assessment_id:String,ports:Option<Vec<u16>>,source:bool,target_ref:Option<crate::scope_targets::ScopeTargetRef>,expected_scope:Option<crate::scope::ScopeRef>,requested_adapter:Option<String>)->Result<Value,String>{
        let plan=self.compile_options(store,&assessment_id,ports,source,target_ref,expected_scope,requested_adapter,&id())?;
        let Some(plan)=plan else{return store.mutate(|s|{
            let a=s.assessments.iter_mut().find(|a|a.id==assessment_id).ok_or("Avaliação não encontrada.")?;
            a.status="blocked".into();a.summary="Escolha as portas para preparar uma observação TCP delimitada. Execução genérica de ferramentas de rede permanece indisponível.".into();
            a.updated_at=now();a.events.push(event(a.summary.clone(),"warning"));value(a.clone())
        });};
        store.mutate(|s|value(persist_prepared_locked(s,&plan)?))
    }

    /// The caller reserves the work command before entering this method. A
    /// compiled plan has no authority until persisted with its ownership row.
    pub(crate) fn compile_selected(&self,store:&Store,input:&NativeCompileInput,reserved_plan_id:&str)->Result<ExecutionPlan,String>{
        if ![ADAPTER_LOCAL,ADAPTER_HTTP,crate::network::ADAPTER,crate::source_scan::ADAPTER].contains(&input.adapter.as_str())
            ||(input.adapter==crate::network::ADAPTER)!=input.ports.is_some(){return Err("Adaptador ou portas incompatíveis com o plano de trabalho.".into());}
        self.compile_options(store,&input.assessment_id,input.ports.clone(),input.adapter==crate::source_scan::ADAPTER,
            input.target_ref.clone(),Some(input.scope_ref.clone()),Some(input.adapter.clone()),reserved_plan_id)?
            .ok_or_else(||"Portas TCP são obrigatórias para compilar esta operação.".into())
    }

    #[allow(clippy::too_many_arguments)]
    fn compile_options(&self,store:&Store,assessment_id:&str,ports:Option<Vec<u16>>,source:bool,target_ref:Option<crate::scope_targets::ScopeTargetRef>,expected_scope:Option<crate::scope::ScopeRef>,requested_adapter:Option<String>,reserved_plan_id:&str)->Result<Option<ExecutionPlan>,String>{
        core::valid_id(&assessment_id)?;
        core::valid_id(reserved_plan_id)?;
        let state=store.snapshot()?;
        if state.plans.iter().any(|plan|plan.id==reserved_plan_id){return Err("Identidade de plano já utilizada.".into());}
        let a=state.assessments.iter().find(|a|a.id==assessment_id).ok_or("Avaliação não encontrada.")?;
        ensure_startable(&state,a)?;
        let selected=crate::scope_targets::resolve_current_target(&state,&assessment_id,target_ref.as_ref())?;
        if ports.is_some()&&selected.kind!="network"{return Err("Portas TCP só podem ser escolhidas para alvo de rede.".into());}
        if source&&!["code","files"].contains(&selected.kind.as_str()){return Err("Análise estática exige alvo de código ou arquivos.".into());}
        let method=match selected.kind.as_str(){"code"|"files" if source=>crate::source_scan::ADAPTER,"code"|"files"=>ADAPTER_LOCAL,"web"=>ADAPTER_HTTP,"network"=>crate::network::ADAPTER,_=>return Err("Modalidade indisponível.".into())};
        if requested_adapter.as_deref().is_some_and(|adapter|adapter!=method){return Err("Método não corresponde à entrada escolhida.".into());}
        let refs=target_ref.iter().cloned().collect::<Vec<_>>();
        let scope_ref=crate::scope::with_selected_admission(store,&assessment_id,method,expected_scope.as_ref(),&refs,if target_ref.is_some(){"assessment.targetPlan"}else if source{"assessment.sourcePlan"}else if ports.is_some(){"assessment.networkPlan"}else{"assessment.plan"},|reference|Ok(reference.clone()))?;
        crate::scope::recheck_selected(store,&scope_ref,method,&refs)?;
        let preparation_deadline=crate::scope::deadline(store,&scope_ref,Duration::from_secs(30))?;
        let preflight=if matches!(selected.kind.as_str(),"code"|"files"){Some(crate::local_scope::preflight(&store.snapshot()?,&scope_ref,target_ref.as_ref())?)}else{None};
        let created=chrono::Utc::now();
        let mut plan=ExecutionPlan{id:reserved_plan_id.into(),assessment_id:a.id.clone(),adapter:String::new(),target:selected.target.clone(),
            timeout_seconds:state.settings.timeout_seconds,output_limit_ki_b:state.settings.output_limit_ki_b,
            input_files:vec![],resolved_addresses:vec![],source_scope:None,digest:String::new(),created_at:created.to_rfc3339(),
            expires_at:(created+chrono::Duration::minutes(15)).to_rfc3339(),approved_at:None,status:"proposed".into(),scope_ref:Some(scope_ref.clone()),target_ref:target_ref.clone(),requested_ports:if target_ref.is_some(){ports.clone()}else{None},identity_preflight_refs:preflight.as_ref().map(|lease|lease.refs.clone()).unwrap_or_default(),scope_omissions:vec![]};
        match selected.kind.as_str() {
            "code"|"files" if source=>{
                let(prepared,omissions)=if target_ref.is_some(){crate::source_scan::plan_selected(Path::new(&plan.target),&AtomicBool::new(false),preparation_deadline,&selected.local_exclusions)?}else{(crate::source_scan::plan(Path::new(&plan.target),&AtomicBool::new(false),preparation_deadline)?,vec![])};
                plan.scope_omissions=omissions;
                plan.adapter=crate::source_scan::ADAPTER.into();plan.input_files=prepared.input_files;plan.source_scope=Some(prepared.scope);
            }
            "code"|"files"=>{
                plan.adapter=ADAPTER_LOCAL.into();
                if target_ref.is_some(){let(prepared,omissions)=crate::source_scan::plan_selected(Path::new(&plan.target),&AtomicBool::new(false),preparation_deadline,&selected.local_exclusions)?;plan.input_files=prepared.input_files;plan.source_scope=Some(prepared.scope);plan.scope_omissions=omissions;}
                else{plan.input_files=inventory(Path::new(&plan.target),&AtomicBool::new(false),preparation_deadline)?;}
            }
            "web"=>{
                plan.adapter=ADAPTER_HTTP.into();
                let url=url::Url::parse(&plan.target).map_err(|_|"URL inválida.")?;
                let host=url.host_str().ok_or("URL sem host.")?.trim_matches(['[',']']);
                let port=url.port_or_known_default().ok_or("Porta inválida.")?;
                plan.resolved_addresses=(host,port).to_socket_addrs().map_err(|_|"Não foi possível resolver o alvo. Nenhum tráfego de avaliação foi enviado.")?.map(|a|a.to_string()).collect();
                plan.resolved_addresses.sort();plan.resolved_addresses.dedup();
                if plan.resolved_addresses.is_empty() || plan.resolved_addresses.len()>32{return Err("Resolução do alvo vazia ou excede os limites.".into());}
                if target_ref.is_some(){let(allowed,omitted)=filter_http_addresses(&plan.resolved_addresses,&selected.connection_exclusions)?;plan.resolved_addresses=allowed;plan.scope_omissions=omitted;}
            }
            "network" if ports.is_some()=>{
                plan.adapter=crate::network::ADAPTER.into();
                if target_ref.is_some(){let(allowed,omitted)=crate::network::plan_addresses_selected(&plan.target,ports.as_deref().unwrap(),&selected.connection_exclusions)?;plan.resolved_addresses=allowed;plan.scope_omissions=omitted;}
                else{plan.resolved_addresses=crate::network::plan_addresses(&plan.target,ports.as_deref().unwrap())?;}
            }
            _=>return Ok(None),
        }
        plan.digest=plan_digest(&plan);
        Ok(Some(plan))
    }

    fn approve(&self,store:&Store,d:DecisionInput)->Result<Value,String> {
        store.work_transaction(|conn,s|{crate::work_store::assert_direct_unowned(conn,&d.id)?;value(approve_locked(s,&d.id,&d.digest)?)})
    }

    fn revoke(&self,store:&Store,d:DecisionInput)->Result<Value,String> {
        let running=self.running.lock().map_err(|_|"Executor indisponível.")?;
        store.work_transaction(|conn,s|{
            let index=plan_index(s,&d)?;
            let assessment_id=s.plans[index].assessment_id.clone();
            if !["proposed","approved","running"].contains(&s.plans[index].status.as_str()){return Err("Este plano já foi encerrado.".into());}
            let active=s.plans[index].status=="running";
            s.plans[index].status=if active {"canceling"} else {"revoked"}.into();
            if let Some(token)=running.get(&assessment_id){token.store(true,Ordering::SeqCst);}
            let a=s.assessments.iter_mut().find(|a|a.id==assessment_id).unwrap();
            a.status=if active{"canceling"}else{"blocked"}.into();a.updated_at=now();
            a.events.push(event("Autorização revogada. Novas operações estão bloqueadas; término ativo exige confirmação.","warning"));
            crate::work_supervisor::sync_native_plan_locked(conn,s,&d.id)?;value(s.plans[index].clone())
        })
    }

    fn cancel(&self,store:&Store,assessment_id:String)->Result<Value,String> {
        core::valid_id(&assessment_id)?;
        let running=self.running.lock().map_err(|_|"Executor indisponível.")?;
        let snapshot=store.snapshot()?;
        if !snapshot.plans.iter().any(|p|p.assessment_id==assessment_id) {
            return store.handle(core::Request{action:"assessment.cancel".into(),payload:json!({"id":assessment_id})});
        }
        store.work_transaction(|conn,s|{
            let a=s.assessments.iter_mut().find(|a|a.id==assessment_id).ok_or("Avaliação não encontrada.")?;
            if a.status=="unknown"{return Err("Efeito desconhecido exige reconciliação, não cancelamento presumido.".into());}
            if !["awaiting_approval","queued","running","canceling"].contains(&a.status.as_str()){return Err("Avaliação sem operação cancelável.".into());}
            let active=matches!(a.status.as_str(),"running"|"canceling");
            if active && !running.contains_key(&assessment_id){return Err("Supervisor não encontrado. Reconcilie o estado antes de continuar.".into());}
            if let Some(token)=running.get(&assessment_id){token.store(true,Ordering::SeqCst);}
            a.status=if active{"canceling"}else{"canceled"}.into();a.updated_at=now();
            a.events.push(event(if active{"Encerramento solicitado; aguardando confirmação do adaptador."}else{"Plano cancelado antes da execução."},"warning"));
            for p in s.plans.iter_mut().filter(|p|p.assessment_id==assessment_id && ["proposed","approved","running"].contains(&p.status.as_str())) {p.status=if active{"canceling"}else{"canceled"}.into();}
            let result=a.clone();
            let ids=s.plans.iter().filter(|p|p.assessment_id==assessment_id).map(|p|p.id.clone()).collect::<Vec<_>>();
            for plan_id in ids {crate::work_supervisor::sync_native_plan_locked(conn,s,&plan_id)?;}
            value(result)
        })
    }

    fn run(&self,store:&Store,d:DecisionInput)->Result<Value,String> {
        value(self.run_internal(store,d,crate::work_supervisor::DispatchOrigin::Direct)?.execution)
    }

    pub(crate) fn run_queued(&self,store:&Store,plan_id:&str,digest:&str,token:crate::work_supervisor::QueuedAttemptToken)->Result<NativeCompletion,String> {
        self.run_internal(store,DecisionInput{id:plan_id.into(),digest:digest.into()},crate::work_supervisor::DispatchOrigin::Queued(token))
    }

    fn run_internal(&self,store:&Store,d:DecisionInput,origin:crate::work_supervisor::DispatchOrigin)->Result<NativeCompletion,String> {
        use crate::work_supervisor::{self,EffectOwner,EffectOwnerKind,EffectTermination};
        use crate::workflow::EffectRef;
        // The arbiter claim comes first. No Store callback acquires runtime or
        // arbiter locks, and no SQLite transaction spans adapter I/O.
        let mut permit=match origin {
            work_supervisor::DispatchOrigin::Direct=>{
                let assessment_id=store.work_read(|conn,s|{crate::work_store::assert_direct_unowned(conn,&d.id)?;let i=plan_index(s,&d)?;check_plan(s,&s.plans[i])?;Ok(s.plans[i].assessment_id.clone())})?;
                self.arbiter.claim_direct(store,EffectOwner{id:id(),assessment_id:Some(assessment_id),kind:EffectOwnerKind::Native})?
            },
            work_supervisor::DispatchOrigin::Queued(token)=>self.arbiter.claim_queued(store,token)?,
        };
        let mut running=self.running.lock().map_err(|_|"Executor indisponível.")?;
        let token=Arc::new(AtomicBool::new(false));
        let scope_guard=store.scope_gate.lock().map_err(|_|"Escopo indisponível.")?;
        let admitted=store.work_transaction(|conn,s|{
            let context=if permit.attempt_id().is_some(){Some(work_supervisor::validate_queued_native_locked(conn,s,&permit,&d.id,&d.digest)?)}else{crate::work_store::assert_direct_unowned(conn,&d.id)?;None};
            let index=plan_index(s,&d)?;
            if running.contains_key(&s.plans[index].assessment_id){return Err("Já há uma operação ativa nesta avaliação.".into());}
            let(plan,execution)=admit_locked(s,&d.id,&d.digest,&id())?;
            work_supervisor::bind_effect_locked(conn,&permit,&EffectRef::NativeExecution{id:execution.id.clone()})?;
            let capture=context.as_ref().map(|context|crate::capture::create_native_locked(conn,s,&execution,&context.attempt_id,context.max_capture_bytes)).transpose()?;
            work_supervisor::mark_native_running_locked(conn,&permit,&execution,capture.as_ref().map(|m|m.id.as_str()))?;
            Ok((plan,execution,capture,context.map(|c|c.max_capture_bytes)))
        });
        drop(scope_guard);
        let(plan,execution,capture,cap)=match admitted {Ok(result)=>result,Err(error)=>{drop(running);self.arbiter.finish(permit,EffectTermination::NotStarted)?;return Err(error);}};
        permit.acknowledge_bound(EffectRef::NativeExecution{id:execution.id.clone()});
        running.insert(plan.assessment_id.clone(),token.clone());drop(running);
        let mut sink=match capture {Some(manifest)=>Some(crate::capture::CaptureSink::new(store,manifest,cap.ok_or("Reserva de captura ausente.")?)?),None=>None};
        let outcome=(||{
            let scope=plan.scope_ref.as_ref().ok_or("scope_legacy_plan_requires_reprepare")?;
            let refs=plan.target_ref.iter().cloned().collect::<Vec<_>>();crate::scope::recheck_selected(store,scope,&plan.adapter,&refs)?;
            let snapshot=store.snapshot()?;let selected=plan_target(&snapshot,&plan)?;
            let _preflight=if matches!(selected.as_ref().map(|target|target.kind.as_str()),Some("code"|"files")){let lease=crate::local_scope::preflight(&snapshot,scope,plan.target_ref.as_ref())?;if lease.refs!=plan.identity_preflight_refs{return Err("As referências locais do plano mudaram.".into());}Some(lease)}else{None};
            crate::scope::recheck_selected(store,scope,&plan.adapter,&refs)?;
            let deadline=crate::scope::deadline(store,scope,Duration::from_millis(execution.scope_admission.as_ref().ok_or("Recibo de escopo ausente.")?.effective_timeout_ms))?;
            execute_selected_captured(&plan,&token,deadline,selected.as_ref(),&mut |record|match sink.as_mut(){Some(sink)=>sink.record(record),None=>Ok(())})
        })();
        let mut running=self.running.lock().map_err(|_|"Executor indisponível.")?;
        let canceled=token.load(Ordering::SeqCst);
        let interrupted=canceled||outcome.is_err()||outcome.as_ref().ok().and_then(|text|serde_json::from_str::<Value>(text).ok()).is_some_and(|v|v["timedOut"]==true||v["stopReason"]=="timeout"||v["termination"]=="read_error");
        // Reaching here proves this synchronous native adapter has stopped.
        // Evidence, capture seal, work result and lease release commit together.
        let result=store.work_transaction(|conn,s|{
            let mut completion=finish_locked(s,&plan,&execution,outcome,canceled)?;
            let sealed=if let Some(sink)=&sink {let(observed,loss)=sink.seal_info();completion.partial|=loss.is_some();Some(crate::capture::seal_locked(conn,sink.capture_id(),if interrupted{None}else{observed},loss,interrupted)?)}else{None};
            work_supervisor::finish_native_locked(conn,s,&permit,&completion,sealed.as_ref())?;
            work_supervisor::finish_permit_locked(conn,&permit,EffectTermination::Stopped)?;
            Ok(completion)
        });
        running.remove(&plan.assessment_id);drop(running);
        if result.is_ok(){permit.acknowledge_finished();}
        // On persistence failure Drop retains an unknown lease; no new effect
        // can follow merely because the process-local runtime token was removed.
        result
    }
}

/// These helpers mutate only the caller's snapshot. The caller owns the single
/// transaction that also writes work bindings, attempts, captures and events.
pub(crate) fn persist_prepared_locked(s:&mut Snapshot,plan:&ExecutionPlan)->Result<ExecutionPlan,String>{
    core::valid_id(&plan.id)?;
    if s.plans.iter().any(|p|p.id==plan.id)||plan.status!="proposed"||plan.approved_at.is_some()||plan.digest!=plan_digest(plan){return Err("Plano compilado inválido ou já persistido.".into());}
    let scope_ref=plan.scope_ref.as_ref().ok_or("scope_legacy_plan_requires_reprepare")?;
    let refs=plan.target_ref.iter().cloned().collect::<Vec<_>>();
    crate::scope::check_selected_work(s,&plan.assessment_id,&plan.adapter,Some(scope_ref),&refs)?;
    let a=s.assessments.iter().find(|a|a.id==plan.assessment_id).ok_or("Avaliação não encontrada.")?;ensure_startable(s,a)?;
    if crate::scope_targets::resolve_historical_target(s,scope_ref,plan.target_ref.as_ref())?.target!=plan.target{return Err("O escopo mudou durante a preparação.".into());}
    if s.settings.timeout_seconds!=plan.timeout_seconds||s.settings.output_limit_ki_b!=plan.output_limit_ki_b{return Err("Os limites mudaram durante a preparação.".into());}
    s.plans.push(plan.clone());
    let a=s.assessments.iter_mut().find(|a|a.id==plan.assessment_id).unwrap();
    a.status="awaiting_approval".into();a.updated_at=now();
    a.summary=match plan.adapter.as_str(){crate::source_scan::ADAPTER=>"Plano preparado: sinais estáticos locais com identidade e hashes fixados. Candidatos exigem validação; nenhum código é executado ou enviado.",ADAPTER_LOCAL=>"Plano preparado: inventário de arquivos e hashes, sem executar código do alvo.",crate::network::ADAPTER=>"Plano preparado: conexões TCP somente nos endereços e portas exibidos, sem DNS, payload ou subprocessos.",_=>"Plano preparado: uma requisição HTTP HEAD à URL exata, sem seguir redirects ou enviar credenciais."}.into();
    a.events.push(event(a.summary.clone(),"info"));
    Ok(plan.clone())
}

pub(crate) fn approve_locked(s:&mut Snapshot,plan_id:&str,digest:&str)->Result<ExecutionPlan,String>{
    let index=plan_index(s,&DecisionInput{id:plan_id.into(),digest:digest.into()})?;
    let plan=&s.plans[index];
    if plan.status!="proposed"{return Err("O plano não está aguardando aprovação.".into());}
    check_plan(s,plan)?;
    s.plans[index].status="approved".into();s.plans[index].approved_at=Some(now());
    let assessment_id=s.plans[index].assessment_id.clone();
    let a=s.assessments.iter_mut().find(|a|a.id==assessment_id).unwrap();
    a.status="queued".into();a.updated_at=now();a.events.push(event("Aprovação humana registrada para o digest e limites exatos do plano.","info"));
    Ok(s.plans[index].clone())
}

pub(crate) fn admit_locked(s:&mut Snapshot,plan_id:&str,digest:&str,reserved_execution_id:&str)->Result<(ExecutionPlan,Execution),String>{
    core::valid_id(reserved_execution_id)?;
    if s.executions.iter().any(|e|e.id==reserved_execution_id){return Err("Identidade de execução já utilizada.".into());}
    let index=plan_index(s,&DecisionInput{id:plan_id.into(),digest:digest.into()})?;
    let p=s.plans[index].clone();check_plan(s,&p)?;
    if p.status!="approved"||p.approved_at.is_none(){return Err("Execução exige aprovação humana válida e ainda não consumida.".into());}
    s.plans[index].status="running".into();
    let mut receipt=crate::scope::admission_receipt_selected(s,&p.assessment_id,&p.adapter,p.timeout_seconds as u64*1000,&p.target_ref.iter().cloned().collect::<Vec<_>>())?;
    receipt.identity_preflight_refs=p.identity_preflight_refs.clone();
    let execution=Execution{scope_admission:Some(receipt),id:reserved_execution_id.into(),plan_id:p.id.clone(),assessment_id:p.assessment_id.clone(),status:"running".into(),started_at:now(),finished_at:None,evidence_id:None,message:String::new()};
    s.executions.push(execution.clone());
    let a=s.assessments.iter_mut().find(|a|a.id==p.assessment_id).unwrap();
    a.status="running".into();a.updated_at=now();a.events.push(event("Execução iniciada sob o plano aprovado.","info"));
    Ok((p,execution))
}

pub(crate) fn finish_locked(s:&mut Snapshot,plan:&ExecutionPlan,execution:&Execution,outcome:Result<String,String>,canceled:bool)->Result<NativeCompletion,String>{
    let index=s.executions.iter().position(|e|e.id==execution.id&&e.plan_id==plan.id&&e.assessment_id==plan.assessment_id).ok_or("Execução não encontrada.")?;
    if !matches!(s.executions[index].status.as_str(),"running"|"canceling"){return Err("Execução já finalizada; o efeito não será repetido.".into());}
    let parsed=outcome.as_ref().ok().and_then(|text|serde_json::from_str::<Value>(text).ok());
    let partial=parsed.as_ref().is_some_and(|v|v["partial"]==true||v["capturePartial"]==true||v["canceled"]==true||v["timedOut"]==true||v["stopReason"]=="timeout"||v["termination"]=="read_error");
    let (status,message,content)=match outcome {
        Ok(output)=>{
            let timed_out=parsed.as_ref().is_some_and(|v|v["stopReason"]=="timeout"||v["timedOut"]==true);
            let read_error=parsed.as_ref().is_some_and(|v|v["termination"]=="read_error");
            (if canceled{"canceled"}else if timed_out||read_error{"failed"}else{"completed"},if canceled{"Adaptador encerrado após solicitação de cancelamento. Evidência parcial preservada; efeitos já enviados não foram desfeitos.".into()}else if timed_out{"Tempo limite atingido. Observações parciais preservadas; entradas restantes não foram avaliadas.".into()}else if read_error{"Erro de leitura interrompeu a análise. Evidência parcial preservada; entradas restantes não foram avaliadas.".into()}else{"Operação delimitada concluída. A avaliação permanece parcial; não equivale a uma auditoria completa.".into()},Some(output))
        },
        Err(error)=>(if canceled{"canceled"}else{"failed"},error,None),
    };
    let a=s.assessments.iter_mut().find(|a|a.id==plan.assessment_id).ok_or("Avaliação não encontrada.")?;
    let evidence=content.map(|content|Evidence{id:id(),name:format!("{}-{}.json",plan.adapter,execution.id),sha256:core::hash(&content),content,created_at:now()});
    if let Some(e)=&evidence {
        a.evidence.push(e.clone());
        a.coverage.push(format!("{}; alvo exato {}; execução {}. Sem execução de código do alvo, exploração ou garantia de ausência de falhas.",plan.adapter,plan.target,execution.id));
    }
    a.status=match status{"completed"=>"partial",other=>other}.into();a.summary=message.clone();a.updated_at=now();a.events.push(event(message.clone(),if status=="completed"{"success"}else{"warning"}));
    let e=&mut s.executions[index];e.status=status.into();e.message=message;e.finished_at=Some(now());e.evidence_id=evidence.as_ref().map(|e|e.id.clone());
    s.plans.iter_mut().find(|p|p.id==plan.id).ok_or("Plano não encontrado.")?.status=status.into();
    Ok(NativeCompletion{execution:e.clone(),evidence,partial:partial||canceled||status!="completed"})
}

fn ensure_startable(s:&Snapshot,a:&Assessment)->Result<(),String>{
    if !s.projects.iter().any(|p|p.id==a.project_id&&!p.archived){return Err("Projeto arquivado não aceita operações.".into());}
    if !["draft","blocked","paused","partial","failed","canceled"].contains(&a.status.as_str()){return Err("Esta avaliação não pode preparar um novo plano neste estado.".into());}
    if s.plans.iter().filter(|p|p.assessment_id==a.id).count()>=100{return Err("Esta avaliação atingiu o limite de 100 planos. Preserve o histórico e prepare outra avaliação.".into());}
    Ok(())
}
fn plan_index(s:&Snapshot,d:&DecisionInput)->Result<usize,String>{
    core::valid_id(&d.id)?;
    let i=s.plans.iter().position(|p|p.id==d.id).ok_or("Plano não encontrado.")?;
    if s.plans[i].digest!=d.digest || plan_digest(&s.plans[i])!=d.digest{return Err("O plano foi alterado. A aprovação anterior não é válida.".into());}Ok(i)
}
fn check_plan(s:&Snapshot,p:&ExecutionPlan)->Result<(),String>{
    let reference=p.scope_ref.as_ref().ok_or("scope_legacy_plan_requires_reprepare: prepare um novo plano após revisar o escopo.")?;
    crate::scope::check_selected_work(s,&p.assessment_id,&p.adapter,Some(reference),&p.target_ref.iter().cloned().collect::<Vec<_>>())?;
    let selected=plan_target(s,p)?;validate_selected_plan(s,p,selected.as_ref())?;
    if chrono::DateTime::parse_from_rfc3339(&p.expires_at).map_err(|_|"Validade do plano inválida.")?<=chrono::Utc::now(){return Err("Plano expirado. Prepare uma nova avaliação.".into());}
    let a=s.assessments.iter().find(|a|a.id==p.assessment_id).ok_or("Avaliação não encontrada.")?;
    if !s.projects.iter().any(|project|project.id==a.project_id&&!project.archived) || !["awaiting_approval","queued"].contains(&a.status.as_str()){return Err("Escopo, projeto ou estado mudou; a operação não foi iniciada.".into());}
    if s.settings.timeout_seconds!=p.timeout_seconds || s.settings.output_limit_ki_b!=p.output_limit_ki_b{return Err("Os limites mudaram. O plano precisa ser preparado novamente.".into());}
    Ok(())
}
fn checkpoint(cancel:&AtomicBool,deadline:Instant)->Result<(),String>{
    if cancel.load(Ordering::SeqCst){return Err("Adaptador local encerrado por cancelamento.".into());}
    if Instant::now()>=deadline{return Err("Tempo limite atingido; nenhuma operação será repetida automaticamente.".into());}Ok(())
}
fn is_link(meta:&fs::Metadata)->bool {
    #[cfg(windows)] {use std::os::windows::fs::MetadataExt;meta.file_type().is_symlink() || meta.file_attributes()&0x400!=0}
    #[cfg(not(windows))] {meta.file_type().is_symlink()}
}
fn inventory(root:&Path,cancel:&AtomicBool,deadline:Instant)->Result<Vec<InputFile>,String>{
    inventory_with_hook(root,cancel,deadline,&mut |_|{})
}
fn inventory_with_hook(root:&Path,cancel:&AtomicBool,deadline:Instant,before_open:&mut dyn FnMut(&Path))->Result<Vec<InputFile>,String>{
    inventory_captured(root,cancel,deadline,before_open,&mut |_|Ok(()))
}
fn inventory_captured(root:&Path,cancel:&AtomicBool,deadline:Instant,before_open:&mut dyn FnMut(&Path),capture:&mut dyn FnMut(&Value)->Result<(),String>)->Result<Vec<InputFile>,String>{
    #[cfg(windows)]
    let _root_guard={
        let directory=fs::symlink_metadata(root).map_err(|_|"Não foi possível ler o alvo selecionado.")?.is_dir();
        crate::source_scan::lock_read_entry(root,directory)?
    };
    let metadata=fs::symlink_metadata(root).map_err(|_|"Não foi possível ler o alvo selecionado.")?;
    if is_link(&metadata){return Err("Links e reparse points não são aceitos como entrada deste adaptador.".into());}
    let canonical=fs::canonicalize(root).map_err(|_|"Não foi possível resolver o alvo.")?;
    let mut files=vec![];let mut total=0;
    visit(&canonical,&canonical,0,&mut files,&mut total,cancel,deadline,before_open,capture)?;
    files.sort_by(|a,b|a.path.cmp(&b.path));Ok(files)
}
#[allow(clippy::too_many_arguments)]
fn visit(root:&Path,path:&Path,depth:usize,files:&mut Vec<InputFile>,total:&mut u64,cancel:&AtomicBool,deadline:Instant,before_open:&mut dyn FnMut(&Path),capture:&mut dyn FnMut(&Value)->Result<(),String>)->Result<(),String>{
    checkpoint(cancel,deadline)?;
    if depth>16{return Err("Profundidade máxima de 16 diretórios atingida; selecione um alvo menor.".into());}
    let metadata=fs::symlink_metadata(path).map_err(|_|"Entrada indisponível durante o inventário.")?;
    before_open(path);
    #[cfg(windows)]
    let (locked_entry,_ancestor_guards)=crate::source_scan::lock_read_entry(path,metadata.is_dir())?;
    if is_link(&metadata){return Err("Inventário recusado: entrada contém link/reparse point.".into());}
    let resolved=fs::canonicalize(path).map_err(|_|"Entrada não pôde ser resolvida.")?;
    if !resolved.starts_with(root){return Err("Entrada saiu do diretório autorizado.".into());}
    if metadata.is_dir(){
        for entry in fs::read_dir(path).map_err(|_|"Diretório selecionado não pôde ser lido.")? {
            let entry=entry.map_err(|_|"Falha na enumeração do diretório.")?;
            let name=entry.file_name();
            if [".git","node_modules","target"].iter().any(|skip|name==*skip){continue;}
            visit(root,&entry.path(),depth+1,files,total,cancel,deadline,before_open,capture)?;
        }
    } else if metadata.is_file(){
        if files.len()>=MAX_FILES || metadata.len()>MAX_INPUT.saturating_sub(*total){return Err("Inventário limitado a 1.000 arquivos e 16 MiB. Selecione uma entrada menor.".into());}
        #[cfg(windows)]
        let file=locked_entry;
        #[cfg(not(windows))]
        let file=fs::File::open(path).map_err(|_|"Arquivo selecionado não pôde ser aberto.")?;
        if is_link(&file.metadata().map_err(|_|"Metadados de arquivo indisponíveis.")?){return Err("Identidade do arquivo mudou.".into());}
        let mut reader=file.take(MAX_INPUT.saturating_sub(*total)+1);let mut digest=Sha256::new();let mut bytes=0u64;let mut buffer=[0u8;8192];
        loop {checkpoint(cancel,deadline)?;let n=reader.read(&mut buffer).map_err(|_|"Falha ao ler entrada selecionada.")?;if n==0{break;}bytes+=n as u64;if bytes>MAX_INPUT.saturating_sub(*total){return Err("Arquivo cresceu além do limite de entrada.".into());}digest.update(&buffer[..n]);}
        *total+=bytes;
        let name=if root==path{path.file_name().ok_or("Arquivo sem nome.")?.to_string_lossy().into_owned()}else{path.strip_prefix(root).map_err(|_|"Entrada fora do escopo.")?.to_string_lossy().into_owned()};
        let input=InputFile{path:name,size:bytes,sha256:format!("{:x}",digest.finalize())};
        capture(&json!({"kind":"inventory_file","observation":input}))?;files.push(input);
    } else {return Err("Entrada não é arquivo regular nem diretório.".into());}Ok(())
}
#[cfg(test)]
fn execute(plan:&ExecutionPlan,cancel:&AtomicBool,scope_deadline:Instant)->Result<String,String>{
    execute_selected(plan,cancel,scope_deadline,None)
}
fn execute_selected(plan:&ExecutionPlan,cancel:&AtomicBool,scope_deadline:Instant,selected:Option<&crate::scope_targets::ResolvedTarget>)->Result<String,String>{
    execute_selected_captured(plan,cancel,scope_deadline,selected,&mut |_|Ok(()))
}
fn execute_selected_captured(plan:&ExecutionPlan,cancel:&AtomicBool,scope_deadline:Instant,selected:Option<&crate::scope_targets::ResolvedTarget>,capture:&mut dyn FnMut(&Value)->Result<(),String>)->Result<String,String>{
    if plan.target_ref.is_some()&&selected.is_none(){return Err("Plano combinado exige política resolvida pelo host.".into());}
    if plan.target_ref.is_some(){crate::source_scan::validate_selected_paths(&plan.input_files,&selected.ok_or("Política local ausente.")?.local_exclusions)?;}
    let deadline=scope_deadline.min(Instant::now()+Duration::from_secs(plan.timeout_seconds as u64));checkpoint(cancel,deadline)?;
    if plan.adapter==crate::source_scan::ADAPTER {
        return crate::source_scan::scan_captured(Path::new(&plan.target),&plan.input_files,plan.source_scope.as_ref().ok_or("Manifest estático ausente.")?,cancel,deadline,plan.output_limit_ki_b as usize*1024,capture)?.to_json();
    }
    let output=match plan.adapter.as_str(){
        crate::network::ADAPTER=>{
            let mut output=if plan.target_ref.is_some(){crate::network::observe_selected_captured(&plan.target,plan.requested_ports.as_deref().ok_or("Portas originais ausentes.")?,&plan.resolved_addresses,&selected.ok_or("Política TCP ausente.")?.connection_exclusions,cancel,deadline,capture)?}else{crate::network::observe_captured(&plan.target,&plan.resolved_addresses,cancel,deadline,capture)?};
            output["planDigest"]=Value::String(plan.digest.clone());output
        }
        ADAPTER_LOCAL=>{
            let current=if plan.target_ref.is_some(){crate::source_scan::verify_manifest_captured(Path::new(&plan.target),&plan.input_files,plan.source_scope.as_ref().ok_or("Inventário combinado sem manifesto.")?,cancel,deadline,capture)?;plan.input_files.clone()}else{inventory_captured(Path::new(&plan.target),cancel,deadline,&mut |_|{},capture)?};
            if serde_json::to_string(&current).map_err(|_|"Entrada inválida.")?!=serde_json::to_string(&plan.input_files).map_err(|_|"Manifest inválido.")?{return Err("Os arquivos mudaram após a aprovação. Nenhuma evidência foi confirmada; prepare um novo plano.".into());}
            json!({"adapter":plan.adapter,"planDigest":plan.digest,"target":plan.target,"files":current,"excludedDirectories":[".git","node_modules","target"],"limits":"Apenas inventário/hash; conteúdo não executado nem enviado à IA."})
        }
        ADAPTER_HTTP=>{
            let url=url::Url::parse(&plan.target).map_err(|_|"URL inválida.")?;
            let host=url.host_str().ok_or("Host inválido.")?.trim_matches(['[',']']);
            let addresses=plan.resolved_addresses.iter().map(|value|value.parse::<std::net::SocketAddr>()).collect::<Result<Vec<_>,_>>().map_err(|_|"Endpoint resolvido inválido.")?;
            if let Some(selected)=selected{if plan.target_ref.is_some(){validate_http_peers(&plan.target,&plan.resolved_addresses,&selected.connection_exclusions)?;}for peer in &addresses{if crate::scope_targets::connection_excluded(&selected.connection_exclusions,*peer).is_some(){return Err("scope_connection_excluded: HTTP bloqueado antes do socket.".into());}}}
            let client=reqwest::blocking::Client::builder().no_proxy().redirect(reqwest::redirect::Policy::none())
                .timeout(deadline.saturating_duration_since(Instant::now())).connect_timeout(Duration::from_secs(10).min(deadline.saturating_duration_since(Instant::now())))
                .resolve_to_addrs(host,&addresses).build().map_err(|_|"Cliente HTTP indisponível.")?;
            checkpoint(cancel,deadline)?;
            let response=client.head(url.clone()).header("User-Agent","OpenKoda/0.1 (authorized HEAD observation)").send().map_err(|_|"Requisição não concluída (conexão, TLS ou timeout). Nenhum retry foi enviado; o alvo pode ter recebido a requisição.")?;
            capture(&json!({"kind":"http_status","status":response.status().as_u16(),"remoteAddress":response.remote_addr().map(|peer|peer.to_string()),"method":"HEAD"}))?;
            let mut headers=serde_json::Map::new();
            let mut header_values=serde_json::Map::new();
            for name in crate::http_validation::HEADER_NAMES {
                for v in response.headers().get_all(name) {
                    let record=match std::str::from_utf8(v.as_bytes()) {
                        Ok(text)=>json!({"kind":"http_header","name":name,"encoding":"utf8","valueText":text}),
                        Err(_)=>json!({"kind":"http_header","name":name,"encoding":"unknown","valueBase64":STANDARD.encode(v.as_bytes())}),
                    };
                    capture(&record)?;
                }
                if let Some(v)=response.headers().get(name){headers.insert(name.into(),Value::String(v.to_str().unwrap_or("[valor não textual]").to_owned()));}
                header_values.insert(name.into(),json!(response.headers().get_all(name).iter().map(|v|STANDARD.encode(v.as_bytes())).collect::<Vec<_>>()));
            }
            json!({"schema":crate::http_validation::OBSERVATION_SCHEMA,"adapter":plan.adapter,"planDigest":plan.digest,"target":plan.target,"status":response.status().as_u16(),"headers":headers,"headerValues":header_values,"resolvedAddresses":plan.resolved_addresses,"remoteAddress":response.remote_addr().map(|peer|peer.to_string()),"method":"HEAD","captureComplete":true,"redirectFollowed":false,"limits":"Uma requisição HEAD; sem corpo, cookies, autenticação, exploração ou navegação."})
        }
        _=>return Err("Adaptador não permitido.".into())
    };
    let encoded=serde_json::to_string_pretty(&output).map_err(|_|"Evidência inválida.")?;
    if encoded.len()>plan.output_limit_ki_b as usize*1024{return Err("A saída excedeu o limite aprovado. Operação encerrada sem repetir o alvo.".into());}Ok(encoded)
}

#[cfg(test)]
pub(crate) mod combined_fixture {
    use super::*;
    pub struct Fixture {pub _dir:tempfile::TempDir,pub store:Store,pub executor:Executor,pub aid:String,pub refs:Vec<crate::scope_targets::ScopeTargetRef>}
    impl Fixture {
        pub fn new(targets:&[(&str,&str,&[&str])],local_rules:&[(usize,&str,&str)],connection_rules:Value)->Self{
            Self::with_excluded(targets,local_rules,connection_rules,&[])
        }
        pub fn with_excluded(targets:&[(&str,&str,&[&str])],local_rules:&[(usize,&str,&str)],connection_rules:Value,excluded:&[(&str,&str)])->Self{
            let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().join("data")).unwrap();
            let primary=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Combined bounded execution fixture","kind":targets[0].0,"target":targets[0].1})}).unwrap();let pid=primary["id"].as_str().unwrap();let mut ids=vec!["primary".to_string()];
            for (index,(kind,target,_)) in targets.iter().enumerate().skip(1){let options=crate::scope_targets::catalog_options(&store.snapshot().unwrap(),pid).unwrap();let added=crate::scope_targets::handle_catalog("project.targetAdd",json!({"projectId":pid,"expectedCatalogDigest":options["projectCatalogDigest"],"idempotencyKey":id(),"label":format!("Selected fixture {index}"),"kind":kind,"target":target}),&store).unwrap();ids.push(added["targetId"].as_str().unwrap().into());}
            let mut excluded_ids=vec![];for (kind,target) in excluded{let options=crate::scope_targets::catalog_options(&store.snapshot().unwrap(),pid).unwrap();let added=crate::scope_targets::handle_catalog("project.targetAdd",json!({"projectId":pid,"expectedCatalogDigest":options["projectCatalogDigest"],"idempotencyKey":id(),"label":"Never open excluded fixture","kind":kind,"target":target}),&store).unwrap();excluded_ids.push(added["targetId"].clone());}
            let options=crate::scope_targets::catalog_options(&store.snapshot().unwrap(),pid).unwrap();let mut input=json!({"projectId":pid,"title":"Synthetic combined assessment","objective":"Observe only explicitly selected fixture entries","window":{"startsAt":null,"endsAt":null},"projectCatalogDigest":options["projectCatalogDigest"],"idempotencyKey":id(),"selection":{"primaryProjectTargetId":"primary","entries":targets.iter().enumerate().map(|(i,(_,_,methods))|json!({"projectTargetId":ids[i],"methods":methods})).collect::<Vec<_>>(),"commonMethods":[],"excludedProjectTargets":[],"localExclusions":local_rules.iter().map(|(i,path,kind)|json!({"projectTargetId":ids[*i],"relativePath":path,"match":kind,"reason":"Synthetic exclusion sentinel"})).collect::<Vec<_>>(),"connectionExclusions":connection_rules}});
            input["selection"]["excludedProjectTargets"]=json!(excluded_ids.iter().map(|id|json!({"projectTargetId":id,"reason":"Never open or traverse this root"})).collect::<Vec<_>>());
            let preview=crate::scope::handle("assessment.combinedPreview",input.clone(),&store,str::to_owned).unwrap();input["reviewDigest"]=preview["reviewDigest"].clone();input["acknowledgeScope"]=json!(true);let created=crate::scope::handle("assessment.combinedCreate",input,&store,str::to_owned).unwrap();let aid=created["assessmentId"].as_str().unwrap().to_owned();
            let snapshot=store.snapshot().unwrap();let set=crate::scope_targets::current_body(&snapshot,&aid).unwrap().target_set.as_ref().unwrap();let refs=ids.iter().map(|pid|crate::scope_targets::entry_ref(set.entries.iter().find(|entry|&entry.project_target_id==pid).unwrap())).collect();Self{_dir:dir,store,executor:Executor::default(),aid,refs}
        }
        pub fn input(&self,index:usize,adapter:&str)->Value{json!({"assessmentId":self.aid,"scopeRef":crate::scope::current_ref(&self.store.snapshot().unwrap(),&self.aid).unwrap().unwrap(),"targetRef":self.refs[index],"adapter":adapter})}
        pub fn plan(&self,index:usize,adapter:&str)->Value{self.executor.handle("assessment.targetPlan",self.input(index,adapter),&self.store).unwrap()}
        pub fn run(&self,plan:&Value)->Value{let decision=json!({"id":plan["id"],"digest":plan["digest"]});self.executor.handle("plan.approve",decision.clone(),&self.store).unwrap();self.executor.handle("plan.run",decision,&self.store).unwrap()}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test] fn explicitly_excluded_local_root_refuses_local_preflight_without_opening_any_root(){
        let dir=tempfile::tempdir().unwrap();let missing=dir.path().join("not-created-selected");let excluded=dir.path().join("not-created-excluded");let f=combined_fixture::Fixture::with_excluded(&[("code",missing.to_str().unwrap(),&[crate::source_scan::ADAPTER]),("web","http://127.0.0.1:43129/",&[ADAPTER_HTTP])],&[],json!([]),&[("files",excluded.to_str().unwrap())]);
        let error=f.executor.handle("assessment.targetPlan",f.input(0,crate::source_scan::ADAPTER),&f.store).unwrap_err();assert!(error.contains("scope_excluded_local_identity_unproven"));assert!(f.store.snapshot().unwrap().plans.is_empty());assert!(!missing.exists()&&!excluded.exists());
        // A web plan only resolves its selected literal peer; it never needs a
        // local identity lease, even when both local paths are unavailable.
        let web=f.plan(1,ADAPTER_HTTP);assert_eq!(web["resolvedAddresses"],json!(["127.0.0.1:43129"]));assert!(web.get("identityPreflightRefs").is_none());
    }
    #[cfg(windows)]
    #[test] fn combined_four_kinds_use_explicit_entries_and_preserve_negative_sentinels(){
        use std::io::Write;use std::os::windows::fs::OpenOptionsExt;
        let data=tempfile::tempdir().unwrap();let code=data.path().join("code");fs::create_dir(&code).unwrap();fs::write(code.join("visible.py"),b"print('allowed')\n").unwrap();fs::write(code.join("secret.txt"),b"EXCLUDED-LOCAL-SENTINEL").unwrap();let offline=data.path().join("offline.txt");fs::write(&offline,b"offline fixture").unwrap();
        let _locked=fs::OpenOptions::new().read(true).share_mode(0).open(code.join("secret.txt")).unwrap();
        let http=std::net::TcpListener::bind("127.0.0.1:0").unwrap();http.set_nonblocking(true).unwrap();let url=format!("http://{}/chosen",http.local_addr().unwrap());
        let denied=std::net::TcpListener::bind("127.0.0.1:0").unwrap();denied.set_nonblocking(true).unwrap();let allowed=std::net::TcpListener::bind("127.0.0.1:0").unwrap();allowed.set_nonblocking(true).unwrap();let ports=[denied.local_addr().unwrap().port(),allowed.local_addr().unwrap().port()];
        let code_name=code.to_str().unwrap();let offline_name=offline.to_str().unwrap();let f=combined_fixture::Fixture::new(&[("code",code_name,&[crate::source_scan::ADAPTER,"source.capture.v1"]),("files",offline_name,&[ADAPTER_LOCAL]),("web",&url,&[ADAPTER_HTTP]),("network","127.0.0.1",&[crate::network::ADAPTER])],&[(0,"secret.txt","exact")],json!([{"address":"127.0.0.1","ports":[ports[0]],"reason":"Never contact the denied listener"}]));
        assert!(f.executor.handle("assessment.plan",json!({"id":f.aid}),&f.store).is_err());let mut missing=f.input(0,crate::source_scan::ADAPTER);missing.as_object_mut().unwrap().remove("targetRef");assert!(f.executor.handle("assessment.targetPlan",missing,&f.store).is_err());
        let mut forged=f.input(0,crate::source_scan::ADAPTER);forged["targetRef"]["entryDigest"]=json!("0".repeat(64));assert!(f.executor.handle("assessment.targetPlan",forged,&f.store).is_err());assert!(f.store.snapshot().unwrap().plans.is_empty());
        let source=f.plan(0,crate::source_scan::ADAPTER);assert_eq!(source["inputFiles"].as_array().unwrap().len(),1);assert_eq!(source["scopeOmissions"].as_array().unwrap().len(),1);assert_eq!(source["identityPreflightRefs"].as_array().unwrap().len(),2);assert_eq!(f.run(&source)["status"],"completed");
        let local=f.plan(1,ADAPTER_LOCAL);assert_eq!(local["target"],offline_name);assert_eq!(local["targetRef"],json!(f.refs[1]));assert_eq!(f.run(&local)["status"],"completed");
        let web=f.plan(2,ADAPTER_HTTP);let server=std::thread::spawn(move||{let end=Instant::now()+Duration::from_secs(5);loop{match http.accept(){Ok((mut stream,_))=>{stream.set_nonblocking(false).unwrap();stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();let mut data=Vec::new();let mut block=[0u8;1024];while !data.windows(4).any(|part|part==b"\r\n\r\n"){let n=stream.read(&mut block).unwrap();assert!(n>0,"fixture request ended before headers");data.extend_from_slice(&block[..n]);assert!(data.len()<=4096,"fixture request exceeded bounded headers");}assert!(String::from_utf8_lossy(&data).starts_with("HEAD /chosen "));stream.write_all(b"HTTP/1.1 200 OK\r\nX-Content-Type-Options: nosniff\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();break;},Err(e)if e.kind()==std::io::ErrorKind::WouldBlock&&Instant::now()<end=>std::thread::sleep(Duration::from_millis(5)),Err(e)=>panic!("fixture listener: {e}")}}});assert_eq!(f.run(&web)["status"],"completed");server.join().unwrap();
        let mut input=f.input(3,crate::network::ADAPTER);input["ports"]=json!(ports);let tcp=f.executor.handle("assessment.targetPlan",input,&f.store).unwrap();assert_eq!(tcp["requestedPorts"],json!(ports));assert_eq!(tcp["resolvedAddresses"],json!([allowed.local_addr().unwrap().to_string()]));assert_eq!(f.run(&tcp)["status"],"completed");assert!(allowed.accept().is_ok());assert_eq!(denied.accept().unwrap_err().kind(),std::io::ErrorKind::WouldBlock);
        let snapshot=f.store.snapshot().unwrap();assert_eq!(snapshot.plans.len(),4);assert_eq!(snapshot.executions.len(),4);assert_eq!(snapshot.assessments[0].evidence.len(),4);assert!(!serde_json::to_string(&snapshot.assessments[0].evidence).unwrap().contains("EXCLUDED-LOCAL-SENTINEL"));for execution in &snapshot.executions{let plan=snapshot.plans.iter().find(|p|p.id==execution.plan_id).unwrap();assert_eq!(execution.scope_admission.as_ref().unwrap().target_ref,plan.target_ref);}
    }
    #[test] fn combined_http_negative_peers_refuse_before_socket_and_cap_before_filter(){
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();listener.set_nonblocking(true).unwrap();let peer=listener.local_addr().unwrap();let url=format!("http://{peer}/");let rules=json!([{"address":"127.0.0.1","ports":[peer.port()],"reason":"Never connect"}]);let f=combined_fixture::Fixture::new(&[("web",&url,&[ADAPTER_HTTP])],&[],rules);
        assert!(f.executor.handle("assessment.targetPlan",f.input(0,ADAPTER_HTTP),&f.store).unwrap_err().contains("scope_all_connections_excluded"));assert_eq!(listener.accept().unwrap_err().kind(),std::io::ErrorKind::WouldBlock);assert!(f.store.snapshot().unwrap().plans.is_empty());
        let rules=vec![crate::scope_targets::ConnectionExclusion{address:"127.0.0.1".into(),ports:None,reason:"Denied".into()}];let(allowed,omitted)=filter_http_addresses(&[peer.to_string(),format!("127.0.0.2:{}",peer.port())],&rules).unwrap();assert_eq!(allowed,vec![format!("127.0.0.2:{}",peer.port())]);assert_eq!(omitted.len(),1);assert!(filter_http_addresses(&vec![peer.to_string();33],&rules).unwrap_err().contains("antes"));
        assert!(validate_http_peers(&url,&[format!("127.0.0.2:{}",peer.port())],&rules).is_err());assert_eq!(listener.accept().unwrap_err().kind(),std::io::ErrorKind::WouldBlock);
    }
    #[cfg(windows)]
    #[test]fn inventory_refuses_a_junction_swapped_after_metadata_before_open(){
        use std::os::windows::process::CommandExt;
        let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input");let child=input.join("child");let outside=dir.path().join("outside");
        fs::create_dir_all(&child).unwrap();fs::create_dir(&outside).unwrap();fs::write(child.join("inside.txt"),b"selected").unwrap();fs::write(outside.join("private.txt"),b"UNSELECTED-OUTSIDE-SENTINEL").unwrap();
        let mut swapped=false;
        let result=inventory_with_hook(&input,&AtomicBool::new(false),Instant::now()+Duration::from_secs(5),&mut |path|{
            if !swapped&&path.file_name().is_some_and(|name|name=="child"){
                fs::rename(&child,dir.path().join("original-child")).unwrap();
                let output=std::process::Command::new("cmd.exe").args(["/C","mklink","/J"]).arg(&child).arg(&outside).creation_flags(0x08000000).output().unwrap();assert!(output.status.success());swapped=true;
            }
        });
        assert!(swapped);assert!(result.is_err());fs::remove_dir(&child).unwrap();assert_eq!(fs::read(outside.join("private.txt")).unwrap(),b"UNSELECTED-OUTSIDE-SENTINEL");
    }
    #[cfg(windows)]
    #[test]fn inventory_locks_parent_and_file_but_preserves_empty_directory_support(){
        let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input");fs::create_dir(&input).unwrap();
        assert!(inventory(&input,&AtomicBool::new(false),Instant::now()+Duration::from_secs(2)).unwrap().is_empty());
        let path=input.join("file.txt");fs::write(&path,b"original").unwrap();
        let (mut file,parents)=crate::source_scan::lock_read_entry(&path,false).unwrap();
        assert!(fs::write(&path,b"different").is_err());assert!(fs::rename(&input,dir.path().join("moved")).is_err());
        let mut text=String::new();file.read_to_string(&mut text).unwrap();assert_eq!(text,"original");drop(file);drop(parents);
        fs::write(&path,b"different").unwrap();let manifest=inventory(&input,&AtomicBool::new(false),Instant::now()+Duration::from_secs(2)).unwrap();assert_eq!(manifest[0].sha256,core::hash("different"));
    }
    #[test] fn a_followup_operation_keeps_prior_evidence_and_requires_a_new_approval(){
        let (_dir,store,executor,id)=setup();let first=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();let d=json!({"id":first["id"],"digest":first["digest"]});
        executor.handle("plan.approve",d.clone(),&store).unwrap();executor.handle("plan.run",d.clone(),&store).unwrap();let prior=store.snapshot().unwrap().assessments[0].evidence[0].clone();
        let next=executor.handle("assessment.sourcePlan",json!({"id":id}),&store).unwrap();assert_ne!(next["id"],first["id"]);let second=json!({"id":next["id"],"digest":next["digest"]});
        assert!(executor.handle("plan.run",second.clone(),&store).is_err());assert!(executor.handle("plan.run",d,&store).is_err());
        executor.handle("plan.approve",second.clone(),&store).unwrap();executor.handle("plan.run",second,&store).unwrap();
        let snapshot=store.snapshot().unwrap();assert_eq!(snapshot.executions.len(),2);assert_eq!(snapshot.assessments[0].evidence.len(),2);assert_eq!(snapshot.assessments[0].evidence[0].sha256,prior.sha256);assert_eq!(snapshot.assessments[0].evidence[0].content,prior.content);
        store.handle(core::Request{action:"project.archive".into(),payload:json!({"id":snapshot.projects[0].id})}).unwrap();assert!(executor.handle("assessment.plan",json!({"id":id}),&store).is_err());
    }
    #[test] fn source_scan_requires_review_preserves_candidates_and_rejects_changed_sources(){
        let (dir,store,executor,id)=setup();
        let input=dir.path().join("input/sample.txt");fs::write(&input,"const api_key = 'synthetic-fixture-value';\neval(user_input);\n").unwrap();
        let p=executor.handle("assessment.sourcePlan",json!({"id":id}),&store).unwrap();
        assert_eq!(p["adapter"],crate::source_scan::ADAPTER);assert!(p["sourceScope"].is_object());
        let d=json!({"id":p["id"],"digest":p["digest"]});assert!(executor.handle("plan.run",d.clone(),&store).is_err());
        let mut altered:ExecutionPlan=serde_json::from_value(p.clone()).unwrap();altered.source_scope.as_mut().unwrap().bindings[0].identity.index="0".into();assert_ne!(plan_digest(&altered),p["digest"]);
        executor.handle("plan.approve",d.clone(),&store).unwrap();assert_eq!(executor.handle("plan.run",d.clone(),&store).unwrap()["status"],"completed");
        assert!(executor.handle("plan.run",d,&store).is_err());
        let snapshot=store.snapshot().unwrap();assert!(snapshot.findings.is_empty());assert_eq!(snapshot.assessments[0].status,"partial");
        let output=&snapshot.assessments[0].evidence[0].content;assert!(!output.contains("synthetic-fixture-value"));
        assert!(serde_json::from_str::<Value>(output).unwrap()["counts"]["candidates"].as_u64().unwrap()>0);
        let (dir,store,executor,id)=setup();let p=executor.handle("assessment.sourcePlan",json!({"id":id}),&store).unwrap();let d=json!({"id":p["id"],"digest":p["digest"]});
        executor.handle("plan.approve",d.clone(),&store).unwrap();fs::write(dir.path().join("input/sample.txt"),"changed").unwrap();
        assert_eq!(executor.handle("plan.run",d,&store).unwrap()["status"],"failed");assert!(store.snapshot().unwrap().assessments[0].evidence.is_empty());
    }
    #[test] fn legacy_plan_digest_and_serialization_remain_compatible(){
        let (_dir,store,executor,id)=setup();let plan=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();
        assert!(plan.get("sourceScope").is_none());
        let mut p:ExecutionPlan=serde_json::from_value(plan).unwrap();
        let new_digest=p.digest.clone();p.scope_ref=None;
        let expected=core::hash(&json!({"id":p.id,"assessmentId":p.assessment_id,"adapter":p.adapter,"target":p.target,"timeoutSeconds":p.timeout_seconds,"outputLimitKiB":p.output_limit_ki_b,"inputFiles":p.input_files,"resolvedAddresses":p.resolved_addresses,"createdAt":p.created_at,"expiresAt":p.expires_at}).to_string());
        assert_ne!(new_digest,expected);p.digest=expected.clone();assert_eq!(plan_digest(&p),expected);assert!(serde_json::to_value(&p).unwrap().get("scopeRef").is_none());
    }
    #[test] fn tcp_observation_requires_port_review_and_preserves_exact_scope(){
        let dir=tempfile::tempdir().unwrap();let store=Store::open(dir.path().join("data")).unwrap();let executor=Executor::default();
        let listener=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let port=listener.local_addr().unwrap().port();
        let project=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"TCP fixture","kind":"network","target":"127.0.0.1"})}).unwrap();
        let assessment=store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":project["id"],"kind":"network","target":"127.0.0.1","title":"TCP fixture","objective":"Selected local port","scope":crate::scope::test_creation(&store,project["id"].as_str().unwrap(),&["network.tcp-connect.v1"])})}).unwrap();
        let plan=executor.handle("assessment.networkPlan",json!({"id":assessment["id"],"ports":[port]}),&store).unwrap();
        assert_eq!(plan["adapter"],crate::network::ADAPTER);
        let decision=json!({"id":plan["id"],"digest":plan["digest"]});
        assert!(executor.handle("plan.run",decision.clone(),&store).is_err());
        assert!(store.mutate(|s|{s.plans[0].resolved_addresses=vec![format!("127.0.0.2:{port}")];s.plans[0].digest=plan_digest(&s.plans[0]);Ok(())}).is_err());
        executor.handle("plan.approve",decision.clone(),&store).unwrap();
        assert_eq!(executor.handle("plan.run",decision.clone(),&store).unwrap()["status"],"completed");
        assert!(executor.handle("plan.run",decision,&store).is_err());
        let snapshot=store.snapshot().unwrap();assert_eq!(snapshot.assessments[0].status,"partial");
        let evidence=&snapshot.assessments[0].evidence[0];assert_eq!(core::hash(&evidence.content),evidence.sha256);
        let observed:Value=serde_json::from_str(&evidence.content).unwrap();assert_eq!(observed["observations"][0]["outcome"],"connected");
        assert_eq!(observed["applicationBytesSent"],0);
    }
    fn setup()->(tempfile::TempDir,Store,Executor,String){
        let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input");fs::create_dir(&input).unwrap();fs::write(input.join("sample.txt"),"fixture").unwrap();
        let store=Store::open(dir.path().join("state")).unwrap();
        let p=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Fixture","kind":"files","target":input.to_string_lossy()})}).unwrap();
        let a=store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":p["id"],"title":"Inventory","kind":"files","target":p["target"],"objective":"Inventory only","scope":crate::scope::test_creation(&store,p["id"].as_str().unwrap(),&["local.inventory.v1","source.rules.v1"])})}).unwrap();
        (dir,store,Executor::default(),a["id"].as_str().unwrap().into())
    }
    #[test] fn capture_keeps_inventory_evidence_bytes_and_hash_format_unchanged(){
        let (_dir,store,executor,aid)=setup();let raw=executor.handle("assessment.plan",json!({"id":aid}),&store).unwrap();let plan:ExecutionPlan=serde_json::from_value(raw).unwrap();
        let legacy=execute(&plan,&AtomicBool::new(false),Instant::now()+Duration::from_secs(20)).unwrap();let mut records=vec![];
        let captured=execute_selected_captured(&plan,&AtomicBool::new(false),Instant::now()+Duration::from_secs(20),None,&mut |record|{records.push(record.clone());Ok(())}).unwrap();
        assert_eq!(captured,legacy);assert_eq!(core::hash(&captured),core::hash(&legacy));assert_eq!(records.len(),plan.input_files.len());
        assert_eq!(records[0]["kind"],"inventory_file");assert_eq!(records[0]["observation"]["sha256"],plan.input_files[0].sha256);
        assert!(!serde_json::to_string(&records).unwrap().contains("fixture"));
    }
    #[test] fn owned_native_plan_cannot_bypass_workflow_through_legacy_commands(){
        let(_dir,store,_aid,_capture)=crate::capture::fixture::build(b"",crate::capture::CaptureState::Open);
        let before=store.snapshot().unwrap();let plan=&before.plans[0];let decision=json!({"id":plan.id,"digest":plan.digest});let executor=Executor::default();
        for action in ["plan.approve","plan.run"] {assert!(executor.handle(action,decision.clone(),&store).unwrap_err().starts_with("work_owned_operation"));}
        assert_eq!(serde_json::to_value(store.snapshot().unwrap()).unwrap(),serde_json::to_value(before).unwrap());
    }
    #[test] fn shared_effect_lease_blocks_legacy_native_dispatch_without_consuming_approval(){
        use crate::work_supervisor::{WorkArbiter,EffectOwner,EffectOwnerKind,EffectTermination};
        let(_dir,store,executor,aid)=setup();let arbiter=Arc::new(WorkArbiter::default());let executor=executor.with_arbiter(arbiter.clone());
        let plan=executor.handle("assessment.plan",json!({"id":aid}),&store).unwrap();let decision=json!({"id":plan["id"],"digest":plan["digest"]});executor.handle("plan.approve",decision.clone(),&store).unwrap();
        let permit=arbiter.claim_direct(&store,EffectOwner{id:id(),assessment_id:Some(aid),kind:EffectOwnerKind::AiRequest}).unwrap();
        assert!(executor.handle("plan.run",decision,&store).unwrap_err().starts_with("work_busy"));let state=store.snapshot().unwrap();assert!(state.executions.is_empty());assert_eq!(state.plans[0].status,"approved");
        arbiter.finish(permit,EffectTermination::NotStarted).unwrap();
    }
    #[test] fn approval_is_required_bound_and_single_use(){
        let (_dir,store,executor,id)=setup();let p=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();
        let d=json!({"id":p["id"],"digest":p["digest"]});
        assert!(executor.handle("plan.run",d.clone(),&store).is_err());
        assert!(executor.handle("plan.approve",json!({"id":p["id"],"digest":"forged"}),&store).is_err());
        executor.handle("plan.approve",d.clone(),&store).unwrap();
        let result=executor.handle("plan.run",d.clone(),&store).unwrap();assert_eq!(result["status"],"completed");
        assert!(executor.handle("plan.run",d,&store).is_err());
        let s=store.snapshot().unwrap();assert_eq!(s.assessments[0].status,"partial");assert_eq!(s.assessments[0].evidence.len(),1);
    }
    #[test] fn changed_inputs_and_revocation_never_execute_old_plan(){
        let (dir,store,executor,id)=setup();let p=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();let d=json!({"id":p["id"],"digest":p["digest"]});
        executor.handle("plan.approve",d.clone(),&store).unwrap();fs::write(dir.path().join("input/sample.txt"),"changed").unwrap();
        let e=executor.handle("plan.run",d,&store).unwrap();assert_eq!(e["status"],"failed");assert!(store.snapshot().unwrap().assessments[0].evidence.is_empty());
        let (_dir,store,executor,id)=setup();let p=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();let d=json!({"id":p["id"],"digest":p["digest"]});
        executor.handle("plan.approve",d.clone(),&store).unwrap();executor.handle("plan.revoke",d.clone(),&store).unwrap();assert!(executor.handle("plan.run",d,&store).is_err());
    }
    #[test] fn cancel_before_dispatch_preserves_empty_evidence(){
        let (_dir,store,executor,id)=setup();executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();executor.handle("assessment.cancel",json!({"id":id}),&store).unwrap();
        let s=store.snapshot().unwrap();assert_eq!(s.assessments[0].status,"canceled");assert!(s.executions.is_empty());
    }
    #[test] fn cancellation_and_timeout_checkpoints_are_observed(){
        assert!(checkpoint(&AtomicBool::new(true),Instant::now()+Duration::from_secs(1)).is_err());
        assert!(checkpoint(&AtomicBool::new(false),Instant::now()).is_err());
    }
    #[test] fn pinned_http_target_does_not_follow_redirects(){
        use std::io::{Read,Write};
        let server=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let address=server.local_addr().unwrap();
        let peer=std::thread::spawn(move||{
            let (mut connection,_)=server.accept().unwrap();connection.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut buffer=[0u8;2048];let count=connection.read(&mut buffer).unwrap();let request=String::from_utf8_lossy(&buffer[..count]).into_owned();
            connection.write_all(format!("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:{}/forbidden\r\nServer: TEXT-SENTINEL\r\nSet-Cookie: ignored=COOKIE-SENTINEL\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",address.port()).as_bytes()).unwrap();request
        });
        let mut p=ExecutionPlan{target_ref:None,requested_ports:None,identity_preflight_refs:vec![],scope_omissions:vec![],scope_ref:None,id:id(),assessment_id:id(),adapter:ADAPTER_HTTP.into(),target:format!("http://pinned.invalid:{}/allowed",address.port()),timeout_seconds:10,output_limit_ki_b:16,input_files:vec![],resolved_addresses:vec![address.to_string()],source_scope:None,digest:String::new(),created_at:now(),expires_at:now(),approved_at:None,status:"proposed".into()};p.digest=plan_digest(&p);
        let mut records=vec![];let output=execute_selected_captured(&p,&AtomicBool::new(false),Instant::now()+Duration::from_secs(30),None,&mut |record|{records.push(record.clone());Ok(())}).unwrap();let observed:Value=serde_json::from_str(&output).unwrap();
        assert_eq!(observed["status"],302);assert_eq!(observed["redirectFollowed"],false);
        let server=records.iter().find(|record|record["name"]=="server").unwrap();assert_eq!(server["valueText"],"TEXT-SENTINEL");assert!(server.get("valueBase64").is_none());assert!(!serde_json::to_string(&records).unwrap().contains("COOKIE-SENTINEL"));
        let request=peer.join().unwrap();assert!(request.starts_with("HEAD /allowed HTTP/1.1"));assert!(request.to_lowercase().contains("host: pinned.invalid:"));assert!(!request.to_lowercase().contains("authorization:"));
    }
    #[test] fn plan_tampering_and_expiry_are_rejected(){
        let (_dir,store,executor,id)=setup();let p=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();
        assert!(store.mutate(|s|{s.plans[0].target="C:\\outside".into();Ok(())}).is_err());
        store.mutate(|s|{s.plans[0].expires_at=(chrono::Utc::now()-chrono::Duration::minutes(1)).to_rfc3339();s.plans[0].digest=plan_digest(&s.plans[0]);Ok(())}).unwrap();
        let digest=store.snapshot().unwrap().plans[0].digest.clone();
        assert!(executor.handle("plan.approve",json!({"id":p["id"],"digest":digest}),&store).is_err());
    }
    #[test] fn restart_revokes_approval_and_does_not_replay(){
        let (dir,store,executor,id)=setup();let p=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();let d=json!({"id":p["id"],"digest":p["digest"]});
        executor.handle("plan.approve",d.clone(),&store).unwrap();drop(store);
        let store=Store::open(dir.path().join("state")).unwrap();assert_eq!(store.snapshot().unwrap().plans[0].status,"revoked");assert!(executor.handle("plan.run",d,&store).is_err());assert!(store.snapshot().unwrap().executions.is_empty());
    }
    #[test] fn input_quota_and_changed_limits_block_execution(){
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("oversized.bin");fs::File::create(&path).unwrap().set_len(MAX_INPUT+1).unwrap();
        assert!(inventory(&path,&AtomicBool::new(false),Instant::now()+Duration::from_secs(1)).is_err());
        let (_dir,store,executor,id)=setup();let p=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();let d=json!({"id":p["id"],"digest":p["digest"]});
        executor.handle("plan.approve",d.clone(),&store).unwrap();
        store.handle(core::Request{action:"settings.save".into(),payload:json!({"timeoutSeconds":42})}).unwrap();
        assert!(executor.handle("plan.run",d,&store).is_err());assert!(store.snapshot().unwrap().executions.is_empty());
    }
    #[cfg(windows)]
    #[test] fn directory_junction_is_not_followed(){
        let dir=tempfile::tempdir().unwrap();let input=dir.path().join("input");let outside=dir.path().join("outside");fs::create_dir(&input).unwrap();fs::create_dir(&outside).unwrap();fs::write(outside.join("unselected.txt"),"fixture outside selected root").unwrap();
        let result=std::process::Command::new("cmd.exe").args(["/C","mklink","/J"]).arg(input.join("link")).arg(&outside).output().unwrap();
        assert!(result.status.success(),"temporary junction fixture creation must succeed");
        assert!(inventory(&input,&AtomicBool::new(false),Instant::now()+Duration::from_secs(2)).is_err());
    }
    #[test] fn reconciliation_records_uncertainty_without_replaying(){
        let (dir,store,executor,id)=setup();let p=executor.handle("assessment.plan",json!({"id":id}),&store).unwrap();let d=json!({"id":p["id"],"digest":p["digest"]});executor.handle("plan.approve",d.clone(),&store).unwrap();
        store.mutate(|s|{s.plans[0].status="running".into();s.assessments[0].status="running".into();let receipt=crate::scope::admission_receipt(s,&id,&s.plans[0].adapter,s.plans[0].timeout_seconds as u64*1000)?;s.executions.push(Execution{scope_admission:Some(receipt),id:crate::model::id(),plan_id:s.plans[0].id.clone(),assessment_id:id.clone(),status:"running".into(),started_at:now(),finished_at:None,evidence_id:None,message:String::new()});Ok(())}).unwrap();drop(store);
        let store=Store::open(dir.path().join("state")).unwrap();assert_eq!(store.snapshot().unwrap().assessments[0].status,"unknown");
        assert!(executor.handle("assessment.reconcile",json!({"id":id,"note":"Reviewed fixture","acknowledgeUnknownEffects":false}),&store).is_err());
        executor.handle("assessment.reconcile",json!({"id":id,"note":"Reviewed synthetic interruption","acknowledgeUnknownEffects":true}),&store).unwrap();
        let state=store.snapshot().unwrap();assert_eq!(state.assessments[0].status,"failed");assert!(state.assessments[0].evidence.is_empty());assert_eq!(state.executions.len(),1);assert!(state.executions[0].message.contains("permanecem desconhecidos"));assert!(executor.handle("plan.run",d,&store).is_err());
    }
}
