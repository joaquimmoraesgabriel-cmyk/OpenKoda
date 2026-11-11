//! Typed resource selection and negative policies. This module performs no
//! target I/O. Resolved metadata is not an admission or an approval.
use crate::{core::{self, Store}, model::{self, Project, Snapshot}, scope::{self, ScopeBody, ScopeRef}};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::{BTreeSet, HashSet}, net::{IpAddr, SocketAddr}};

pub const TARGET_SET_SCHEMA: &str = "openkoda.scope-target-set.v1";
pub const LIMITS: &[&str] = &[
    "Uma avaliação seleciona até oito entradas do catálogo; cadastrar não autoriza operações.",
    "Cada operação escolhe uma entrada e ainda exige plano e aprovação próprios.",
    "Exclusões de conexão valem para HTTP e TCP de toda a avaliação; DNS da URL escolhida pode ocorrer.",
    "Preparo local verifica apenas metadados das raízes locais selecionadas para impedir sobreposição; conteúdo permanece na entrada escolhida.",
    "Ao trabalhar com raízes locais, use exclusões relativas. Uma raiz local explicitamente excluída do catálogo bloqueia o preparo local: sua exclusão não autoriza abrir metadados para provar ausência de aliases.",
];
pub(crate) fn err(code: &str) -> String { format!("{code}: revise a seleção e as restrições do escopo.") }
fn digest<T: Serialize>(value: &T) -> String { core::hash(&serde_json::to_string(value).expect("serializable scope target")) }
fn text(value: &str, max: usize) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > max || value.chars().any(char::is_control) { return Err(err("scope_target_invalid_text")); }
    Ok(value.to_owned())
}
fn is_false(v: &bool) -> bool { !*v }
fn sha(v: &str) -> bool { v.len()==64 && v.bytes().all(|b|b.is_ascii_digit()||(b'a'..=b'f').contains(&b)) }

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ProjectTarget {
    pub id:String, pub label:String, pub kind:String, pub target:String, pub created_at:String,
    #[serde(default,skip_serializing_if="is_false")] pub inactive:bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct CatalogReceipt { pub idempotency_key:String, pub action:String, pub request_digest:String, pub target_id:String, pub created_at:String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeTargetRef { pub entry_id:String, pub entry_digest:String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeTarget { pub entry_id:String, pub project_target_id:String, pub kind:String, pub target:String, pub label:String, pub methods:Vec<String> }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct LocalExclusion { pub entry_id:String, pub relative_path:String, #[serde(rename="match")] pub match_kind:String, pub reason:String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ConnectionExclusion { pub address:String, #[serde(default,skip_serializing_if="Option::is_none")] pub ports:Option<Vec<u16>>, pub reason:String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ExcludedProjectTarget { pub project_target_id:String, pub kind:String, pub target:String, pub label:String, pub reason:String }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeTargetSet {
    pub schema:String, pub primary_entry_id:String, pub entries:Vec<ScopeTarget>, pub common_methods:Vec<String>,
    pub excluded_project_targets:Vec<ExcludedProjectTarget>, pub local_exclusions:Vec<LocalExclusion>, pub connection_exclusions:Vec<ConnectionExclusion>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ScopeOmission { pub kind:String, pub value:String, pub rule_digest:String, pub reason:String }
#[derive(Clone, Debug)]
pub struct ResolvedTarget {
    pub kind:String, pub target:String, pub label:String, pub methods:Vec<String>, pub target_ref:Option<ScopeTargetRef>,
    pub local_exclusions:Vec<LocalExclusion>, pub connection_exclusions:Vec<ConnectionExclusion>,
}
pub type ResolvedScopeTarget = ResolvedTarget;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TargetSelection { pub project_target_id:String, pub methods:Vec<String> }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct LocalExclusionInput { pub project_target_id:String, pub relative_path:String, #[serde(rename="match")] pub match_kind:String, pub reason:String }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct ExcludedTargetInput { pub project_target_id:String, pub reason:String }
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct TargetSetInput {
    pub primary_project_target_id:String, pub entries:Vec<TargetSelection>, pub common_methods:Vec<String>,
    pub excluded_project_targets:Vec<ExcludedTargetInput>, pub local_exclusions:Vec<LocalExclusionInput>, pub connection_exclusions:Vec<ConnectionExclusion>,
}

/// Stable host-generated UUIDs make pure previews repeatable. Client keys are
/// namespaced by project/assessment; callers cannot choose a persisted ID.
pub fn derived_id(namespace:&str,parts:&[&str])->String {
    let hash=digest(&json!({"namespace":namespace,"parts":parts}));
    let mut bytes=[0u8;16]; for (i,b) in bytes.iter_mut().enumerate(){*b=u8::from_str_radix(&hash[i*2..i*2+2],16).expect("hex digest");}
    bytes[6]=(bytes[6]&0x0f)|0x50;bytes[8]=(bytes[8]&0x3f)|0x80;uuid::Uuid::from_bytes(bytes).to_string()
}
pub fn entry_ref(entry:&ScopeTarget)->ScopeTargetRef { ScopeTargetRef{entry_id:entry.entry_id.clone(),entry_digest:digest(&json!({"schema":"openkoda.scope-target.v1","entry":entry}))} }
pub fn target_ref(entry:&ScopeTarget)->ScopeTargetRef { entry_ref(entry) }
pub fn is_combined(s:&Snapshot,aid:&str)->bool { s.scope_revisions.iter().any(|r|r.assessment_id==aid&&r.body.target_set.is_some()) }
pub fn catalog_targets(p:&Project)->Vec<ProjectTarget> {
    let mut entries=vec![ProjectTarget{id:"primary".into(),label:p.name.clone(),kind:p.kind.clone(),target:p.target.clone(),created_at:p.created_at.clone(),inactive:false}];
    entries.extend(p.additional_targets.clone()); entries
}
pub fn catalog_digest(p:&Project)->String { digest(&json!({"schema":"openkoda.project-targets.v1","projectId":p.id,"archived":p.archived,"targets":catalog_targets(p),"catalogVersion":scope::CATALOG_VERSION})) }
pub fn validate_catalog(p:&Project)->Result<(),String> {
    if p.additional_targets.len()>63 || p.target_receipts.len()>1000 {return Err(err("scope_catalog_limit"));}
    let mut ids=HashSet::new();let mut identities=HashSet::new();
    // Historical V1 primary targets keep their original parser contract. New
    // V2 selection validates its stricter path/address syntax prospectively.
    identities.insert(identity(&p.kind,&p.target).unwrap_or_else(|_|format!("legacy:{}:{}",p.kind,p.target)));
    for t in &p.additional_targets {
        core::valid_id(&t.id)?;if !ids.insert(&t.id)||t.label!=text(&t.label,200)?||t.target!=normalize_target(&t.kind,&t.target)?||!identities.insert(identity(&t.kind,&t.target)?){return Err(err("scope_catalog_integrity"));}
        chrono::DateTime::parse_from_rfc3339(&t.created_at).map_err(|_|err("scope_catalog_integrity"))?;
    }
    let mut keys=HashSet::new();for r in &p.target_receipts {core::valid_id(&r.idempotency_key)?;core::valid_id(&r.target_id)?;if !keys.insert(&r.idempotency_key)||!sha(&r.request_digest)||!["project.targetAdd","project.targetUpdate","project.targetRemove"].contains(&r.action.as_str()){return Err(err("scope_catalog_integrity"));}chrono::DateTime::parse_from_rfc3339(&r.created_at).map_err(|_|err("scope_catalog_integrity"))?;}
    Ok(())
}
fn local_key(target:&str)->Result<String,String> {
    let value=target.replace('\\',"/");
    // Absolute Windows paths and platform-native fixture paths are accepted;
    // ambiguous local components, ADS and device/UNC aliases are not.
    if value.starts_with("//")||value.chars().any(char::is_control){return Err(err("scope_target_ambiguous_path"));}
    let tail=if value.as_bytes().get(1)==Some(&b':'){if !value.as_bytes()[0].is_ascii_alphabetic(){return Err(err("scope_target_ambiguous_path"));}&value[2..]}else{value.as_str()};
    if tail.contains(':')||tail.split('/').filter(|v|!v.is_empty()).any(|v|v=="."||v==".."||v.ends_with('.')||v.ends_with(' ')){return Err(err("scope_target_ambiguous_path"));}
    Ok(value.trim_end_matches('/').to_uppercase())
}
fn normalize_target(kind:&str,target:&str)->Result<String,String> {
    let value=core::validate_target(kind,target)?;
    match kind {"network"=>Ok(parse_network(&value)?.canonical()),"code"|"files"=>{local_key(&value)?;Ok(value)},_=>Ok(value)}
}
fn identity(kind:&str,target:&str)->Result<String,String>{Ok(match kind{"code"|"files"=>format!("local:{}",local_key(target)?),"network"=>format!("network:{}",parse_network(target)?.canonical()),_=>format!("{kind}:{}",core::validate_target(kind,target)?)})}
fn overlap(a_kind:&str,a:&str,b_kind:&str,b:&str)->Result<bool,String>{
    if ["code","files"].contains(&a_kind)&&["code","files"].contains(&b_kind){let a=local_key(a)?;let b=local_key(b)?;return Ok(a==b||a.starts_with(&(b.clone()+"/"))||b.starts_with(&(a+"/")));}
    if a_kind=="network"&&b_kind=="network"{return Ok(parse_network(a)?.overlaps(&parse_network(b)?));}
    Ok(identity(a_kind,a)?==identity(b_kind,b)?)
}
#[derive(Clone,Copy)]
struct Network { address:IpAddr, prefix:u8 }
impl Network {
    fn canonical(&self)->String {let max=if self.address.is_ipv4(){32}else{128};if self.prefix==max{self.address.to_string()}else{format!("{}/{}",self.address,self.prefix)}}
    fn contains(&self,ip:IpAddr)->bool {match(self.address,ip){(IpAddr::V4(a),IpAddr::V4(b))=>masked(u32::from(a)as u128,self.prefix,32)==masked(u32::from(b)as u128,self.prefix,32),(IpAddr::V6(a),IpAddr::V6(b))=>masked(u128::from(a),self.prefix,128)==masked(u128::from(b),self.prefix,128),_=>false}}
    fn overlaps(&self,other:&Self)->bool{self.contains(other.address)||other.contains(self.address)}
}
fn masked(n:u128,prefix:u8,bits:u8)->u128 {if prefix==0{0}else{n & (u128::MAX << (bits-prefix))}}
fn parse_network(value:&str)->Result<Network,String>{
    let (raw,prefix)=value.split_once('/').map_or((value,None),|(a,p)|(a,Some(p)));
    let address:IpAddr=raw.parse().map_err(|_|err("scope_exclusion_invalid_address"))?;
    if matches!(address,IpAddr::V6(a) if a.to_ipv4_mapped().is_some()){return Err(err("scope_exclusion_invalid_address"));}
    let max=if address.is_ipv4(){32}else{128};
    let prefix=prefix.map(|p|p.parse::<u8>().map_err(|_|err("scope_exclusion_invalid_address"))).transpose()?.unwrap_or(max);
    if prefix>max{return Err(err("scope_exclusion_invalid_address"));}
    let address=match address{IpAddr::V4(a)=>IpAddr::V4((masked(u32::from(a)as u128,prefix,32)as u32).into()),IpAddr::V6(a)=>IpAddr::V6(masked(u128::from(a),prefix,128).into())};Ok(Network{address,prefix})
}
pub fn normalize_relative(value:&str)->Result<String,String>{
    let value=value.replace('\\',"/");
    if value.is_empty()||value.len()>4096||value.starts_with('/')||value.ends_with('/')||value.chars().any(|c|c.is_control()||matches!(c,':'|'*'|'?'|'"'|'<'|'>'|'|'))||value.split('/').any(|c|c.is_empty()||c=="."||c==".."||c.ends_with('.')||c.ends_with(' ')||reserved_component(c)){return Err(err("scope_exclusion_invalid_path"));}
    Ok(value)
}
fn reserved_component(value:&str)->bool{let name=value.split('.').next().unwrap_or("").to_uppercase();["CON","PRN","AUX","NUL"].contains(&name.as_str())||["COM","LPT"].iter().any(|prefix|name.strip_prefix(prefix).is_some_and(|tail|tail.len()==1&&matches!(tail.as_bytes()[0],b'1'..=b'9')))}
fn normalize_connection(rule:&ConnectionExclusion)->Result<ConnectionExclusion,String>{
    let ports=rule.ports.as_ref().map(|ports|{let mut sorted=ports.clone();sorted.sort();sorted.dedup();if ports.is_empty()||ports.len()>16||sorted.len()!=ports.len()||sorted.contains(&0){return Err(err("scope_exclusion_invalid_ports"));}Ok(sorted)}).transpose()?;
    Ok(ConnectionExclusion{address:parse_network(&rule.address)?.canonical(),ports,reason:text(&rule.reason,500)?})
}
pub fn local_excluded<'a>(rules:&'a [LocalExclusion],relative:&str)->Option<&'a LocalExclusion>{
    // Exact directory exclusions also prevent later consumers from reaching
    // a child directly. A file cannot have descendants, so this is conservative
    // for either kind without opening the excluded path to discover its type.
    let key=relative.replace('\\',"/").to_uppercase();rules.iter().find(|r|{let path=r.relative_path.to_uppercase();key==path||key.starts_with(&(path+"/"))})
}
pub fn connection_excluded(rules:&[ConnectionExclusion],peer:SocketAddr)->Option<&ConnectionExclusion>{rules.iter().find(|r|r.ports.as_ref().is_none_or(|p|p.contains(&peer.port()))&&parse_network(&r.address).is_ok_and(|n|n.contains(peer.ip())))}
pub fn local_omission(rule:&LocalExclusion,relative:&str)->ScopeOmission{ScopeOmission{kind:"local".into(),value:relative.replace('\\',"/"),rule_digest:digest(rule),reason:rule.reason.clone()}}
pub fn connection_omission(rule:&ConnectionExclusion,peer:SocketAddr)->ScopeOmission{ScopeOmission{kind:"connection".into(),value:peer.to_string(),rule_digest:digest(rule),reason:rule.reason.clone()}}

pub fn normalize_target_set(p:&Project,aid:&str,input:&TargetSetInput)->Result<ScopeTargetSet,String>{
    if p.archived{return Err(err("project_archived"));}core::valid_id(aid)?;
    if input.entries.is_empty()||input.entries.len()>8||input.excluded_project_targets.len()>32||input.local_exclusions.len()>128||input.connection_exclusions.len()>32{return Err(err("scope_target_limit"));}
    let catalog=catalog_targets(p);let lookup=|id:&str|catalog.iter().find(|t|t.id==id).ok_or_else(||err("scope_catalog_target_missing"));
    let mut entries=vec![];let mut selected=HashSet::new();
    for selection in &input.entries {let t=lookup(&selection.project_target_id)?;if t.inactive{return Err(err("scope_catalog_target_inactive"));}if !selected.insert(t.id.clone()){return Err(err("scope_target_duplicate"));}
        let methods=scope::normalize_methods(&t.kind,&selection.methods)?;if methods.is_empty()||methods.iter().any(|m|m=="kali.runtime-diagnostics.v1"){return Err(err("scope_target_invalid_methods"));}
        entries.push(ScopeTarget{entry_id:derived_id("openkoda.scope-entry.v1",&[aid,&t.id]),project_target_id:t.id.clone(),kind:t.kind.clone(),target:normalize_target(&t.kind,&t.target)?,label:t.label.clone(),methods});
    }
    entries.sort_by(|a,b|a.entry_id.cmp(&b.entry_id));
    let primary_entry_id=entries.iter().find(|t|t.project_target_id==input.primary_project_target_id).ok_or_else(||err("scope_primary_not_selected"))?.entry_id.clone();
    let mut common_methods=input.common_methods.clone();common_methods.sort();common_methods.dedup();if common_methods.len()!=input.common_methods.len()||common_methods.iter().any(|m|m!="kali.runtime-diagnostics.v1"){return Err(err("scope_common_method_invalid"));}
    let mut excluded_project_targets=vec![];let mut excluded=HashSet::new();
    for rule in &input.excluded_project_targets{let t=lookup(&rule.project_target_id)?;if selected.contains(&t.id)||!excluded.insert(t.id.clone()){return Err(err("scope_target_conflicting_exclusion"));}excluded_project_targets.push(ExcludedProjectTarget{project_target_id:t.id.clone(),kind:t.kind.clone(),target:normalize_target(&t.kind,&t.target)?,label:t.label.clone(),reason:text(&rule.reason,500)?});}
    excluded_project_targets.sort_by(|a,b|a.project_target_id.cmp(&b.project_target_id));
    let mut local_exclusions=vec![];
    for rule in &input.local_exclusions{let entry=entries.iter().find(|t|t.project_target_id==rule.project_target_id).ok_or_else(||err("scope_exclusion_entry_missing"))?;local_exclusions.push(LocalExclusion{entry_id:entry.entry_id.clone(),relative_path:normalize_relative(&rule.relative_path)?,match_kind:rule.match_kind.clone(),reason:text(&rule.reason,500)?});}
    local_exclusions.sort_by_key(|r|(r.entry_id.clone(),r.relative_path.to_uppercase(),r.match_kind.clone()));
    let mut connection_exclusions=input.connection_exclusions.iter().map(normalize_connection).collect::<Result<Vec<_>,_>>()?;connection_exclusions.sort_by_key(digest);
    let set=ScopeTargetSet{schema:TARGET_SET_SCHEMA.into(),primary_entry_id,entries,common_methods,excluded_project_targets,local_exclusions,connection_exclusions};validate_set(p,aid,&set)?;Ok(set)
}
pub fn validate_set(p:&Project,aid:&str,set:&ScopeTargetSet)->Result<(),String>{
    if set.schema!=TARGET_SET_SCHEMA||set.entries.is_empty()||set.entries.len()>8||set.excluded_project_targets.len()>32||set.local_exclusions.len()>128||set.connection_exclusions.len()>32{return Err(err("scope_target_integrity"));}
    let catalog=catalog_targets(p);let mut ids=HashSet::new();let mut projects=HashSet::new();
    for t in &set.entries{core::valid_id(&t.entry_id)?;let c=catalog.iter().find(|c|c.id==t.project_target_id).ok_or_else(||err("scope_catalog_target_missing"))?;
        if t.entry_id!=derived_id("openkoda.scope-entry.v1",&[aid,&t.project_target_id])||!ids.insert(&t.entry_id)||!projects.insert(&t.project_target_id)||t.kind!=c.kind||t.target!=normalize_target(&c.kind,&c.target)?||t.label!=text(&t.label,200)?||t.methods.is_empty()||t.methods!=scope::normalize_methods(&t.kind,&t.methods)?||t.methods.iter().any(|m|m=="kali.runtime-diagnostics.v1"){return Err(err("scope_target_integrity"));}
    }
    if !ids.contains(&set.primary_entry_id)||set.common_methods.len()>1||set.common_methods.iter().any(|m|m!="kali.runtime-diagnostics.v1"){return Err(err("scope_target_integrity"));}
    for (i,t) in set.entries.iter().enumerate(){for other in set.entries.iter().skip(i+1){if overlap(&t.kind,&t.target,&other.kind,&other.target)?{return Err(err("scope_target_overlap"));}}}
    let mut excluded=HashSet::new();for x in &set.excluded_project_targets{let c=catalog.iter().find(|t|t.id==x.project_target_id).ok_or_else(||err("scope_catalog_target_missing"))?;if projects.contains(&x.project_target_id)||!excluded.insert(&x.project_target_id)||x.kind!=c.kind||x.target!=normalize_target(&c.kind,&c.target)?||x.label!=text(&x.label,200)?||x.reason!=text(&x.reason,500)?{return Err(err("scope_target_integrity"));}for t in &set.entries{if overlap(&x.kind,&x.target,&t.kind,&t.target)?{return Err(err("scope_target_conflicting_exclusion"));}}}
    let mut locals=HashSet::new();for rule in &set.local_exclusions{let t=set.entries.iter().find(|t|t.entry_id==rule.entry_id).ok_or_else(||err("scope_exclusion_entry_missing"))?;if !["code","files"].contains(&t.kind.as_str())||!["exact","subtree"].contains(&rule.match_kind.as_str())||rule.relative_path!=normalize_relative(&rule.relative_path)?||rule.reason!=text(&rule.reason,500)?||!locals.insert((&rule.entry_id,rule.relative_path.to_uppercase())){return Err(err("scope_exclusion_invalid_path"));}if set.local_exclusions.iter().filter(|r|r.entry_id==t.entry_id).count()>32{return Err(err("scope_target_limit"));}}
    let mut connections=HashSet::new();for rule in &set.connection_exclusions{if *rule!=normalize_connection(rule)?||!connections.insert(digest(rule)){return Err(err("scope_exclusion_invalid_address"));}}
    let mut ordered=set.clone();ordered.entries.sort_by(|a,b|a.entry_id.cmp(&b.entry_id));ordered.excluded_project_targets.sort_by(|a,b|a.project_target_id.cmp(&b.project_target_id));ordered.local_exclusions.sort_by_key(|r|(r.entry_id.clone(),r.relative_path.to_uppercase(),r.match_kind.clone()));ordered.connection_exclusions.sort_by_key(digest);if ordered!=*set{return Err(err("scope_target_noncanonical"));}Ok(())
}
pub fn aggregate_methods(set:&ScopeTargetSet)->Vec<String>{set.common_methods.iter().chain(set.entries.iter().flat_map(|t|t.methods.iter())).cloned().collect::<BTreeSet<_>>().into_iter().collect()}
pub fn validate_body(s:&Snapshot,aid:&str,body:&ScopeBody)->Result<(),String>{
    let a=s.assessments.iter().find(|a|a.id==aid).ok_or_else(||err("scope_assessment_missing"))?;let p=s.projects.iter().find(|p|p.id==a.project_id).ok_or_else(||err("scope_project_missing"))?;let set=body.target_set.as_ref().ok_or_else(||err("scope_target_set_missing"))?;validate_set(p,aid,set)?;
    let primary=set.entries.iter().find(|e|e.entry_id==set.primary_entry_id).ok_or_else(||err("scope_primary_not_selected"))?;
    if body.kind!=primary.kind||body.target!=primary.target||body.methods!=aggregate_methods(set){return Err(err("scope_target_integrity"));}Ok(())
}
pub fn current_body<'a>(s:&'a Snapshot,aid:&str)->Result<&'a ScopeBody,String>{
    let current=scope::current_ref(s,aid)?;
    let reference=current.ok_or_else(||err(if s.assessments.iter().any(|a|a.id==aid&&a.scope_id.is_none()){"scope_legacy_requires_adoption"}else{"scope_unreviewed"}))?;
    s.scope_revisions.iter().find(|r|r.id==reference.revision_id).map(|r|&r.body).ok_or_else(||err("scope_integrity_error"))
}
pub fn resolve_body(body:&ScopeBody,target:Option<&ScopeTargetRef>)->Result<ResolvedTarget,String>{
    let Some(set)=&body.target_set else{if target.is_some(){return Err(err("scope_target_not_applicable"));}return Ok(ResolvedTarget{kind:body.kind.clone(),target:body.target.clone(),label:body.target.clone(),methods:body.methods.clone(),target_ref:None,local_exclusions:vec![],connection_exclusions:vec![]})};
    let target=target.ok_or_else(||err("scope_target_required"))?;let entry=set.entries.iter().find(|t|entry_ref(t)==*target).ok_or_else(||err("scope_target_changed"))?;
    let mut connections=set.connection_exclusions.clone();connections.extend(set.excluded_project_targets.iter().filter(|t|t.kind=="network").map(|t|ConnectionExclusion{address:t.target.clone(),ports:None,reason:t.reason.clone()}));
    Ok(ResolvedTarget{kind:entry.kind.clone(),target:entry.target.clone(),label:entry.label.clone(),methods:entry.methods.clone(),target_ref:Some(target.clone()),local_exclusions:set.local_exclusions.iter().filter(|r|r.entry_id==entry.entry_id).cloned().collect(),connection_exclusions:connections})
}
pub fn resolve_current_target(s:&Snapshot,aid:&str,target:Option<&ScopeTargetRef>)->Result<ResolvedTarget,String>{resolve_body(current_body(s,aid)?,target)}
pub fn resolve_target(s:&Snapshot,reference:&ScopeRef,target:Option<&ScopeTargetRef>)->Result<ResolvedTarget,String>{scope::validate_historical_ref(s,&reference.assessment_id,reference)?;let body=&s.scope_revisions.iter().find(|r|r.id==reference.revision_id&&r.digest==reference.digest).ok_or_else(||err("scope_integrity_error"))?.body;resolve_body(body,target)}
pub fn resolve_historical_target(s:&Snapshot,reference:&ScopeRef,target:Option<&ScopeTargetRef>)->Result<ResolvedTarget,String>{resolve_target(s,reference,target)}
pub fn validate_selection(body:&ScopeBody,refs:&[ScopeTargetRef],method:Option<&str>)->Result<(),String>{
    let Some(set)=&body.target_set else{if !refs.is_empty(){return Err(err("scope_target_not_applicable"));}return Ok(())};
    if method==Some("kali.runtime-diagnostics.v1"){if !refs.is_empty()||!set.common_methods.iter().any(|m|m=="kali.runtime-diagnostics.v1"){return Err(err("scope_common_method_not_selected"));}return Ok(())}
    if refs.is_empty()||refs.len()>8||refs.iter().map(|r|&r.entry_id).collect::<HashSet<_>>().len()!=refs.len(){return Err(err("scope_target_required"));}
    for reference in refs{let target=resolve_body(body,Some(reference))?;if method.is_some_and(|method|!target.methods.iter().any(|m|m==method)){return Err(err("scope_target_method_not_selected"));}}Ok(())
}
pub fn target_options(body:&ScopeBody)->Vec<Value>{body.target_set.as_ref().map(|set|set.entries.iter().map(|t|json!({"targetRef":entry_ref(t),"entry":t})).collect()).unwrap_or_default()}
pub fn selection_from_set(set:&ScopeTargetSet)->TargetSetInput {TargetSetInput{primary_project_target_id:set.entries.iter().find(|t|t.entry_id==set.primary_entry_id).map(|t|t.project_target_id.clone()).unwrap_or_default(),entries:set.entries.iter().map(|t|TargetSelection{project_target_id:t.project_target_id.clone(),methods:t.methods.clone()}).collect(),common_methods:set.common_methods.clone(),excluded_project_targets:set.excluded_project_targets.iter().map(|t|ExcludedTargetInput{project_target_id:t.project_target_id.clone(),reason:t.reason.clone()}).collect(),local_exclusions:set.local_exclusions.iter().filter_map(|r|set.entries.iter().find(|t|t.entry_id==r.entry_id).map(|t|LocalExclusionInput{project_target_id:t.project_target_id.clone(),relative_path:r.relative_path.clone(),match_kind:r.match_kind.clone(),reason:r.reason.clone()})).collect(),connection_exclusions:set.connection_exclusions.clone()}}

pub fn catalog_options(s:&Snapshot,pid:&str)->Result<Value,String>{core::valid_id(pid)?;let p=s.projects.iter().find(|p|p.id==pid).ok_or_else(||err("scope_project_missing"))?;let targets=catalog_targets(p).into_iter().map(|t|{let mut value=serde_json::to_value(&t).expect("target serializable");value["inactive"]=json!(t.inactive);value["methodCatalog"]=json!(scope::catalog(&t.kind));value}).collect::<Vec<_>>();Ok(json!({"schema":"openkoda.project-targets.v1","projectId":pid,"projectCatalogDigest":catalog_digest(p),"catalogVersion":scope::CATALOG_VERSION,"targets":targets,"canChange":!p.archived,"canCreate":!p.archived,"defaultSelection":{"primaryProjectTargetId":"","entries":[],"commonMethods":[],"excludedProjectTargets":[],"localExclusions":[],"connectionExclusions":[]},"limits":LIMITS}))}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct CatalogRead{project_id:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct CatalogAdd{project_id:String,expected_catalog_digest:String,idempotency_key:String,label:String,kind:String,target:String}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct CatalogUpdate{project_id:String,target_id:String,expected_catalog_digest:String,idempotency_key:String,#[serde(default)]label:Option<String>,#[serde(default)]inactive:Option<bool>}
#[derive(Deserialize)]#[serde(rename_all="camelCase",deny_unknown_fields)]struct CatalogRemove{project_id:String,target_id:String,expected_catalog_digest:String,idempotency_key:String}
pub fn catalog_supports(action:&str)->bool{matches!(action,"project.targetOptions"|"project.targetAdd"|"project.targetUpdate"|"project.targetRemove")}
pub fn catalog_mutates(action:&str)->bool{catalog_supports(action)&&action!="project.targetOptions"}
pub fn handle_catalog(action:&str,payload:Value,store:&Store)->Result<Value,String>{
    if !payload.is_object()||payload.to_string().len()>16384{return Err(err("scope_invalid_input"));}
    if action=="project.targetOptions"{let p:CatalogRead=serde_json::from_value(payload).map_err(|_|err("scope_invalid_input"))?;return catalog_options(&store.snapshot()?,&p.project_id);}
    let request=digest(&payload);let pid=payload["projectId"].as_str().ok_or_else(||err("scope_invalid_input"))?.to_owned();let key=payload["idempotencyKey"].as_str().ok_or_else(||err("scope_invalid_input"))?.to_owned();core::valid_id(&pid)?;core::valid_id(&key)?;
    let expected=payload["expectedCatalogDigest"].as_str().ok_or_else(||err("scope_invalid_input"))?.to_owned();
    let _guard=store.scope_gate.lock().map_err(|_|err("scope_busy"))?;
    let (target_id,replayed)=store.mutate(|s|{
        let p=s.projects.iter().find(|p|p.id==pid).ok_or_else(||err("scope_project_missing"))?;
        if let Some(r)=p.target_receipts.iter().find(|r|r.idempotency_key==key){if r.action!=action||r.request_digest!=request{return Err(err("scope_idempotency_conflict"));}return Ok((r.target_id.clone(),true));}
        if p.archived{return Err(err("project_archived"));}if catalog_digest(p)!=expected{return Err(err("scope_catalog_changed"));}if p.target_receipts.len()>=1000{return Err(err("scope_catalog_limit"));}
        let target_id=match action{
            "project.targetAdd"=>{let input:CatalogAdd=serde_json::from_value(payload.clone()).map_err(|_|err("scope_invalid_input"))?;let _=(&input.project_id,&input.expected_catalog_digest,&input.idempotency_key);let target=normalize_target(&input.kind,&input.target)?;let label=text(&input.label,200)?;if p.additional_targets.len()>=63||catalog_targets(p).iter().any(|t|identity(&t.kind,&t.target).ok()==identity(&input.kind,&target).ok()){return Err(err("scope_target_duplicate"));}let id=derived_id("openkoda.project-target.v1",&[&pid,&key]);s.projects.iter_mut().find(|p|p.id==pid).unwrap().additional_targets.push(ProjectTarget{id:id.clone(),label,kind:input.kind,target,created_at:model::now(),inactive:false});id},
            "project.targetUpdate"=>{let input:CatalogUpdate=serde_json::from_value(payload.clone()).map_err(|_|err("scope_invalid_input"))?;let _=(&input.project_id,&input.expected_catalog_digest,&input.idempotency_key);core::valid_id(&input.target_id)?;if input.label.is_none()&&input.inactive.is_none(){return Err(err("scope_invalid_input"));}let p=s.projects.iter_mut().find(|p|p.id==pid).unwrap();let t=p.additional_targets.iter_mut().find(|t|t.id==input.target_id).ok_or_else(||err("scope_catalog_target_missing"))?;if let Some(label)=input.label{t.label=text(&label,200)?;}if let Some(inactive)=input.inactive{t.inactive=inactive;}input.target_id},
            "project.targetRemove"=>{let input:CatalogRemove=serde_json::from_value(payload.clone()).map_err(|_|err("scope_invalid_input"))?;let _=(&input.project_id,&input.expected_catalog_digest,&input.idempotency_key);core::valid_id(&input.target_id)?;let used=s.scope_revisions.iter().filter(|r|s.assessments.iter().any(|a|a.id==r.assessment_id&&a.project_id==pid)).filter_map(|r|r.body.target_set.as_ref()).any(|set|set.entries.iter().any(|t|t.project_target_id==input.target_id)||set.excluded_project_targets.iter().any(|t|t.project_target_id==input.target_id));if used{return Err(err("scope_catalog_target_referenced"));}let p=s.projects.iter_mut().find(|p|p.id==pid).unwrap();let i=p.additional_targets.iter().position(|t|t.id==input.target_id).ok_or_else(||err("scope_catalog_target_missing"))?;p.additional_targets.remove(i);input.target_id},
            _=>return Err(err("scope_unknown_action")),
        };
        s.projects.iter_mut().find(|p|p.id==pid).unwrap().target_receipts.push(CatalogReceipt{idempotency_key:key.clone(),action:action.into(),request_digest:request.clone(),target_id:target_id.clone(),created_at:model::now()});Ok((target_id,false))
    })?;Ok(json!({"changed":!replayed,"replayed":replayed,"targetId":target_id,"catalog":catalog_options(&store.snapshot()?,&pid)?}))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture { _directory:tempfile::TempDir, store:Store, project:String, local:String, network:String }
    fn call(store:&Store,action:&str,payload:Value)->Result<Value,String>{if catalog_supports(action){handle_catalog(action,payload,store)}else if scope::supports(action){scope::handle(action,payload,store,str::to_owned)}else{store.handle(core::Request{action:action.into(),payload})}}
    fn add(store:&Store,pid:&str,label:&str,kind:&str,target:&str)->Value{let options=catalog_options(&store.snapshot().unwrap(),pid).unwrap();call(store,"project.targetAdd",json!({"projectId":pid,"expectedCatalogDigest":options["projectCatalogDigest"],"idempotencyKey":model::id(),"label":label,"kind":kind,"target":target})).unwrap()}
    fn fixture()->Fixture{
        let directory=tempfile::tempdir().unwrap();let store=Store::open(directory.path().join("profile")).unwrap();let p=call(&store,"project.create",json!({"name":"Owned target set","kind":"web","target":"http://127.0.0.1:47991/a"})).unwrap();let pid=p["id"].as_str().unwrap().to_owned();
        let local=add(&store,&pid,"Source","code",directory.path().join("source").to_str().unwrap())["targetId"].as_str().unwrap().to_owned();let network=add(&store,&pid,"Loopback","network","127.0.0.1")["targetId"].as_str().unwrap().to_owned();Fixture{_directory:directory,store,project:pid,local,network}
    }
    fn selection(f:&Fixture)->Value{json!({"primaryProjectTargetId":"primary","entries":[{"projectTargetId":"primary","methods":["http.headers.v1","ai.remote-review.v1"]},{"projectTargetId":f.local,"methods":["local.inventory.v1","source.rules.v1","source.capture.v1"]},{"projectTargetId":f.network,"methods":["network.tcp-connect.v1"]}],"commonMethods":["kali.runtime-diagnostics.v1"],"excludedProjectTargets":[],"localExclusions":[{"projectTargetId":f.local,"relativePath":"private","match":"exact","reason":"Private fixture files"}],"connectionExclusions":[{"address":"127.0.0.1","ports":[47992],"reason":"Excluded listener"}]})}
    fn create_payload(f:&Fixture)->Value{json!({"projectId":f.project,"title":"Combined fixture","objective":"Observe selected owned resources only","window":{"startsAt":null,"endsAt":null},"selection":selection(f),"projectCatalogDigest":catalog_options(&f.store.snapshot().unwrap(),&f.project).unwrap()["projectCatalogDigest"],"idempotencyKey":model::id()})}
    fn create(f:&Fixture)->Value{let mut payload=create_payload(f);let preview=call(&f.store,"assessment.combinedPreview",payload.clone()).unwrap();payload["reviewDigest"]=preview["reviewDigest"].clone();payload["acknowledgeScope"]=json!(true);call(&f.store,"assessment.combinedCreate",payload).unwrap()}
    fn refs(s:&Snapshot,aid:&str)->Vec<ScopeTargetRef>{current_body(s,aid).unwrap().target_set.as_ref().unwrap().entries.iter().map(entry_ref).collect()}
    #[test] fn combined_selection_never_uses_aggregate_methods_as_authority(){
        let f=fixture();let created=create(&f);let aid=created["assessmentId"].as_str().unwrap();let s=f.store.snapshot().unwrap();let body=current_body(&s,aid).unwrap();let set=body.target_set.as_ref().unwrap();assert_eq!(set.entries.len(),3);assert!(scope::check_work(&s,aid,"http.headers.v1",None).unwrap_err().contains("scope_target_required"));
        let web=entry_ref(set.entries.iter().find(|e|e.kind=="web").unwrap());let local=entry_ref(set.entries.iter().find(|e|e.kind=="code").unwrap());
        assert!(scope::check_selected_work(&s,aid,"http.headers.v1",None,&[]).is_err());assert!(scope::check_selected_work(&s,aid,"http.headers.v1",None,std::slice::from_ref(&local)).is_err());assert!(scope::check_selected_work(&s,aid,"ai.remote-review.v1",None,&[web.clone(),local.clone()]).is_err());
        assert!(scope::check_selected_work(&s,aid,"http.headers.v1",None,std::slice::from_ref(&web)).is_ok());assert!(scope::check_selected_work(&s,aid,"kali.runtime-diagnostics.v1",None,&[]).is_ok());assert!(scope::check_selected_work(&s,aid,"kali.runtime-diagnostics.v1",None,&[web.clone()]).is_err());
        let forged=ScopeTargetRef{entry_id:local.entry_id.clone(),entry_digest:web.entry_digest.clone()};assert!(resolve_current_target(&s,aid,Some(&forged)).is_err());let other=create(&f);assert!(resolve_current_target(&f.store.snapshot().unwrap(),other["assessmentId"].as_str().unwrap(),Some(&web)).is_err());
        assert!(s.executions.is_empty()&&s.plans.is_empty());assert!(s.assessments.iter().all(|a|a.evidence.is_empty()));
    }
    #[test] fn catalog_replay_and_inactivation_do_not_rewrite_existing_scope(){
        let f=fixture();let created=create(&f);let aid=created["assessmentId"].as_str().unwrap();let before=f.store.snapshot().unwrap();let old=serde_json::to_string(&before.scope_revisions).unwrap();let option=catalog_options(&before,&f.project).unwrap();
        let payload=json!({"projectId":f.project,"targetId":f.local,"expectedCatalogDigest":option["projectCatalogDigest"],"idempotencyKey":model::id(),"label":"Renamed source","inactive":true});let first=call(&f.store,"project.targetUpdate",payload.clone()).unwrap();assert_eq!(first["changed"],true);let repeat=call(&f.store,"project.targetUpdate",payload.clone()).unwrap();assert_eq!(repeat["replayed"],true);let after=f.store.snapshot().unwrap();assert_eq!(serde_json::to_string(&after.scope_revisions).unwrap(),old);
        let reference=refs(&after,aid).into_iter().find(|r|resolve_current_target(&after,aid,Some(r)).unwrap().kind=="code").unwrap();assert!(scope::check_selected_work(&after,aid,"source.rules.v1",None,&[reference]).is_ok());
        let mut changed=payload;changed["label"]=json!("Different request");assert!(call(&f.store,"project.targetUpdate",changed).unwrap_err().contains("idempotency_conflict"));assert!(call(&f.store,"assessment.combinedPreview",create_payload(&f)).unwrap_err().contains("target_inactive"));
        let remove=json!({"projectId":f.project,"targetId":f.local,"expectedCatalogDigest":catalog_options(&after,&f.project).unwrap()["projectCatalogDigest"],"idempotencyKey":model::id()});assert!(call(&f.store,"project.targetRemove",remove).unwrap_err().contains("referenced"));
    }
    #[test] fn pure_preview_commit_replay_and_stale_catalog_are_atomic(){
        let f=fixture();let before=serde_json::to_string(&f.store.snapshot().unwrap()).unwrap();let mut p=create_payload(&f);let first=call(&f.store,"assessment.combinedPreview",p.clone()).unwrap();let second=call(&f.store,"assessment.combinedPreview",p.clone()).unwrap();assert_eq!(first,second);assert_eq!(serde_json::to_string(&f.store.snapshot().unwrap()).unwrap(),before);
        p["reviewDigest"]=first["reviewDigest"].clone();p["acknowledgeScope"]=json!(true);let created=call(&f.store,"assessment.combinedCreate",p.clone()).unwrap();let aid=created["assessmentId"].as_str().unwrap();assert_eq!(first["assessmentId"],aid);
        add(&f.store,&f.project,"Unselected","web","http://127.0.0.1:47993/not-selected");let replay=call(&f.store,"assessment.combinedCreate",p.clone()).unwrap();assert_eq!(replay["assessmentId"],aid);assert_eq!(replay["replayed"],true);
        p["title"]=json!("Changed duplicate");assert!(call(&f.store,"assessment.combinedCreate",p).unwrap_err().contains("idempotency_conflict"));assert_eq!(f.store.snapshot().unwrap().assessments.len(),1);
    }
    #[test] fn draft_anchor_changes_keep_history_and_freeze_then_revoke_blocks_all_entries(){
        let f=fixture();let created=create(&f);let aid=created["assessmentId"].as_str().unwrap();let old=f.store.snapshot().unwrap().scope_revisions[0].clone();let public=scope::status(&f.store,aid).unwrap();let mut changed=selection(&f);changed["primaryProjectTargetId"]=json!(f.local);
        let mut edit=json!({"assessmentId":aid,"expectedStateDigest":public["stateDigest"],"projectCatalogDigest":catalog_options(&f.store.snapshot().unwrap(),&f.project).unwrap()["projectCatalogDigest"],"objective":"Updated bounded objective","window":{"startsAt":null,"endsAt":null},"selection":changed});let preview=call(&f.store,"assessment.combinedScopePreview",edit.clone()).unwrap();edit["reviewDigest"]=preview["reviewDigest"].clone();edit["idempotencyKey"]=json!(model::id());edit["acknowledgeScope"]=json!(true);call(&f.store,"assessment.combinedScopeSave",edit).unwrap();let s=f.store.snapshot().unwrap();assert_eq!(s.scope_revisions[0],old);assert_eq!(s.assessments[0].kind,"code");
        let reference=refs(&s,aid).into_iter().find(|r|resolve_current_target(&s,aid,Some(r)).unwrap().kind=="web").unwrap();let reviewed=scope::check_selected_work(&s,aid,"http.headers.v1",None,std::slice::from_ref(&reference)).unwrap();assert!(resolve_target(&s,&reviewed,Some(&reference)).is_err());scope::with_selected_admission(&f.store,aid,"http.headers.v1",Some(&reviewed),std::slice::from_ref(&reference),"fixture.observe",|_|Ok(())).unwrap();let frozen=scope::status(&f.store,aid).unwrap();assert_eq!(frozen["phase"],"frozen");
        call(&f.store,"assessment.scopeRevoke",json!({"assessmentId":aid,"expectedStateDigest":frozen["stateDigest"],"idempotencyKey":model::id(),"rationale":"Synthetic revocation","acknowledgeRevocation":true})).unwrap();let after=f.store.snapshot().unwrap();assert!(scope::check_selected_work(&after,aid,"http.headers.v1",None,std::slice::from_ref(&reference)).unwrap_err().contains("scope_revoked"));assert!(scope::check_selected_work(&after,aid,"kali.runtime-diagnostics.v1",None,&[]).is_err());assert!(resolve_target(&after,&reviewed,Some(&reference)).is_ok());
    }
    #[test] fn combined_derivation_remaps_entries_and_copies_no_authority(){
        let f=fixture();let created=create(&f);let aid=created["assessmentId"].as_str().unwrap();let old_refs=refs(&f.store.snapshot().unwrap(),aid);let public=scope::status(&f.store,aid).unwrap();let mut payload=json!({"assessmentId":aid,"expectedStateDigest":public["stateDigest"],"projectCatalogDigest":catalog_options(&f.store.snapshot().unwrap(),&f.project).unwrap()["projectCatalogDigest"],"idempotencyKey":model::id(),"title":"Derived fixture","copyFields":["objective","window","targets","exclusions"]});let preview=call(&f.store,"assessment.combinedDerivePreview",payload.clone()).unwrap();payload["reviewDigest"]=preview["reviewDigest"].clone();payload["acknowledgeNewAssessment"]=json!(true);let result=call(&f.store,"assessment.combinedDeriveCreate",payload.clone()).unwrap();assert_eq!(result["scopeStatus"]["phase"],"unreviewed");let derived=result["assessmentId"].as_str().unwrap();let snapshot=f.store.snapshot().unwrap();let body=&snapshot.scope_revisions.iter().find(|r|r.assessment_id==derived).unwrap().body;let set=body.target_set.as_ref().unwrap();assert_eq!(set.entries.len(),3);assert!(set.entries.iter().all(|e|old_refs.iter().all(|r|r.entry_id!=e.entry_id)));assert!(set.local_exclusions.iter().all(|r|set.entries.iter().any(|e|e.entry_id==r.entry_id)));assert!(scope::check_selected_work(&snapshot,derived,"http.headers.v1",None,&[entry_ref(&set.entries[0])]).is_err());assert!(snapshot.plans.is_empty()&&snapshot.executions.is_empty());assert!(snapshot.assessments.iter().all(|a|a.evidence.is_empty()));assert_eq!(call(&f.store,"assessment.combinedDeriveCreate",payload).unwrap()["replayed"],true);
    }
    #[test]
    fn combined_derivation_replays_committed_receipt_while_worker_holds_read_lease() {
        let f = fixture();
        let created = create(&f);
        let source_id = created["assessmentId"].as_str().unwrap();
        let key = model::id();
        let mut payload = json!({
            "assessmentId":source_id, "expectedStateDigest":created["scopeStatus"]["stateDigest"],
            "projectCatalogDigest":catalog_options(&f.store.snapshot().unwrap(), &f.project).unwrap()["projectCatalogDigest"],
            "idempotencyKey":key, "title":"Committed draft",
            "copyFields":["objective","window","targets","exclusions"]
        });
        let preview = call(&f.store, "assessment.combinedDerivePreview", payload.clone()).unwrap();
        payload["reviewDigest"] = preview["reviewDigest"].clone();
        payload["acknowledgeNewAssessment"] = json!(true);
        let gate = std::sync::RwLock::new(());
        let dispatch = |store: &Store, payload: Value| {
            scope::dispatch_combined_derivation(payload, store, &gate, || Ok(()))
        };
        let reader = gate.read().unwrap();
        assert!(dispatch(&f.store, payload.clone()).unwrap_err().starts_with("scope_busy"));
        drop(reader);
        let created = dispatch(&f.store, payload.clone()).unwrap();
        assert_eq!(created["created"], true);
        assert_eq!(created["assessmentId"], preview["assessmentId"]);

        let root = f.store.root.clone();
        drop(f.store);
        let reopened = Store::open(root).unwrap();
        let state = reopened.snapshot().unwrap();
        assert_eq!(state.assessments.len(), 2);
        assert_eq!(state.scope_receipts.iter().filter(|r| r.idempotency_key == key).count(), 1);
        let baseline = serde_json::to_value(&state).unwrap();

        let _reader = gate.read().unwrap();
        reopened.fail_commit_when(|_| true);
        let result = dispatch(&reopened, payload.clone()).unwrap();
        assert_eq!(result["replayed"], true);
        assert_eq!(result["created"], false);
        assert_eq!(result["assessmentId"], created["assessmentId"]);
        assert_eq!(result["scopeStatus"]["stateDigest"], created["scopeStatus"]["stateDigest"]);
        assert_eq!(result["scopeStatus"]["revision"], created["scopeStatus"]["revision"]);
        let denied = scope::dispatch_combined_derivation(payload.clone(), &reopened, &gate, || Err("denied".into()));
        assert_eq!(denied.unwrap_err(), "denied");
        drop(_reader);
        // Hold the first retry inside its lease until the second retry completes.
        let overlapping = scope::dispatch_combined_derivation(payload.clone(), &reopened, &gate, || {
            let other = std::thread::scope(|threads| {
                threads.spawn(|| dispatch(&reopened, payload.clone())).join().unwrap()
            })?;
            assert_eq!(other["assessmentId"], created["assessmentId"]);
            assert_eq!(other["replayed"], true);
            Ok(())
        }).unwrap();
        assert_eq!(overlapping["assessmentId"], created["assessmentId"]);
        assert_eq!(overlapping["replayed"], true);
        let writer = gate.write().unwrap();
        assert!(dispatch(&reopened, payload.clone()).unwrap_err().starts_with("scope_busy"));
        drop(writer);
        let _reader = gate.read().unwrap();
        payload["title"] = json!("Changed draft");
        assert!(dispatch(&reopened, payload.clone()).unwrap_err().starts_with("scope_idempotency_conflict"));
        payload["idempotencyKey"] = json!(model::id());
        assert!(dispatch(&reopened, payload).unwrap_err().starts_with("scope_busy"));
        assert_eq!(serde_json::to_value(reopened.snapshot().unwrap()).unwrap(), baseline);
    }
    #[test] fn manual_evidence_has_explicit_unverified_association_and_no_nested_authority(){
        let f=fixture();let created=create(&f);let aid=created["assessmentId"].as_str().unwrap();let s=f.store.snapshot().unwrap();let reference=refs(&s,aid)[0].clone();let mut p=json!({"assessmentId":aid,"name":"User text","sourceDescription":"Synthetic supplied text","content":"{\"scopeRef\":\"forged\",\"sourceVerified\":true}","acknowledgeUnverifiedSource":true});let before=serde_json::to_string(&s).unwrap();assert!(call(&f.store,"evidence.import",p.clone()).is_err());assert_eq!(serde_json::to_string(&f.store.snapshot().unwrap()).unwrap(),before);
        p["contextTargetRefs"]=json!([reference]);let e=call(&f.store,"evidence.import",p.clone()).unwrap();let content:Value=serde_json::from_str(e["content"].as_str().unwrap()).unwrap();assert_eq!(content["schema"],"openkoda.manual-evidence.v2");assert_eq!(content["sourceVerified"],false);assert_eq!(content["text"],p["content"]);assert_eq!(content["targetRefs"],p["contextTargetRefs"]);assert_eq!(scope::status(&f.store,aid).unwrap()["phase"],"frozen");assert_eq!(call(&f.store,"evidence.import",p).unwrap()["id"],e["id"]);
    }
    #[test] fn negative_policy_is_component_based_and_shared_between_transports(){
        let rule=LocalExclusion{entry_id:model::id(),relative_path:"Private".into(),match_kind:"exact".into(),reason:"Restricted".into()};assert!(local_excluded(std::slice::from_ref(&rule),"private/secret.txt").is_some());assert!(local_excluded(std::slice::from_ref(&rule),"private2/secret.txt").is_none());
        let greek=LocalExclusion{entry_id:model::id(),relative_path:"Σ".into(),match_kind:"subtree".into(),reason:"Case variants".into()};assert!(local_excluded(std::slice::from_ref(&greek),"ς/secret.txt").is_some());assert!(local_excluded(std::slice::from_ref(&greek),"σ/secret.txt").is_some());
        for path in ["../secret","/absolute","a//b","a/./b","a:stream","NUL.txt","a./b","a /b","a\\..\\b","\\\\server\\share"]{assert!(normalize_relative(path).is_err(),"{path}");}
        let rule=normalize_connection(&ConnectionExclusion{address:"127.0.0.9/24".into(),ports:Some(vec![443,80]),reason:"Restricted peers".into()}).unwrap();assert_eq!(rule.address,"127.0.0.0/24");assert_eq!(rule.ports,Some(vec![80,443]));assert!(connection_excluded(std::slice::from_ref(&rule),"127.0.0.1:443".parse().unwrap()).is_some());assert!(connection_excluded(std::slice::from_ref(&rule),"127.0.0.1:444".parse().unwrap()).is_none());assert!(connection_excluded(std::slice::from_ref(&rule),"127.0.1.1:443".parse().unwrap()).is_none());
        let all=normalize_connection(&ConnectionExclusion{address:"0.0.0.0/0".into(),ports:None,reason:"All IPv4 connections".into()}).unwrap();assert!(connection_excluded(&[all],"127.0.0.1:80".parse().unwrap()).is_some());assert!(parse_network("::ffff:127.0.0.1").is_err());assert!(parse_network("hostname.invalid/24").is_err());
    }
    #[test] fn target_overlap_unknown_fields_and_unselected_exclusions_are_rejected_without_io(){
        let f=fixture();let source=f.store.snapshot().unwrap().projects[0].additional_targets.iter().find(|t|t.id==f.local).unwrap().target.clone();let nested=add(&f.store,&f.project,"Nested","files",std::path::Path::new(&source).join("private").to_str().unwrap())["targetId"].as_str().unwrap().to_owned();let mut p=create_payload(&f);p["selection"]["entries"].as_array_mut().unwrap().push(json!({"projectTargetId":nested,"methods":["local.inventory.v1"]}));assert!(call(&f.store,"assessment.combinedPreview",p).unwrap_err().contains("overlap"));
        let mut p=create_payload(&f);p["selection"]["localExclusions"][0]["projectTargetId"]=json!(nested);assert!(call(&f.store,"assessment.combinedPreview",p).is_err());let mut p=create_payload(&f);p["selection"]["entries"][0]["target"]=json!("http://evil.invalid");assert!(call(&f.store,"assessment.combinedPreview",p).is_err());assert!(f.store.snapshot().unwrap().assessments.is_empty());
    }
    #[test] fn legacy_contract_cannot_upgrade_in_place_or_accept_target_refs(){
        let f=fixture();let p=f.store.snapshot().unwrap().projects[0].clone();let a=call(&f.store,"assessment.create",json!({"projectId":p.id,"title":"V1 fixture","kind":p.kind,"target":p.target,"objective":"V1 only","scope":scope::test_creation(&f.store,&p.id,&["http.headers.v1"])})).unwrap();let aid=a["id"].as_str().unwrap();let s=f.store.snapshot().unwrap();let body=current_body(&s,aid).unwrap();assert!(!serde_json::to_value(body).unwrap().as_object().unwrap().contains_key("targetSet"));let unknown=ScopeTargetRef{entry_id:model::id(),entry_digest:"0".repeat(64)};assert!(scope::check_selected_work(&s,aid,"http.headers.v1",None,&[unknown]).is_err());assert!(scope::check_work(&s,aid,"http.headers.v1",None).is_ok());
        let mut legacy=s.clone();legacy.assessments.iter_mut().find(|a|a.id==aid).unwrap().scope_id=None;assert!(current_body(&legacy,aid).unwrap_err().contains("scope_legacy_requires_adoption"));
        let mut unreviewed=s.clone();for r in unreviewed.scope_revisions.iter_mut().filter(|r|r.assessment_id==aid){r.review=None;}assert!(current_body(&unreviewed,aid).unwrap_err().contains("scope_unreviewed"));
        let edit=json!({"assessmentId":aid,"expectedStateDigest":scope::status(&f.store,aid).unwrap()["stateDigest"],"projectCatalogDigest":catalog_digest(&p),"objective":"No conversion","window":{"startsAt":null,"endsAt":null},"selection":selection(&f)});assert!(call(&f.store,"assessment.combinedScopePreview",edit).unwrap_err().contains("derivation_required"));
        let v2=create(&f);let id=v2["assessmentId"].as_str().unwrap();let old_action=json!({"assessmentId":id,"expectedStateDigest":scope::status(&f.store,id).unwrap()["stateDigest"],"objective":"No downgrade","window":{"startsAt":null,"endsAt":null},"methods":["http.headers.v1"]});assert!(call(&f.store,"assessment.scopePreview",old_action).unwrap_err().contains("combined_action_required"));
    }
    #[test] fn combined_window_and_duplicate_refs_fail_before_admission(){
        let f=fixture();let mut payload=create_payload(&f);payload["window"]["startsAt"]=json!((chrono::Utc::now()+chrono::Duration::hours(1)).to_rfc3339());let preview=call(&f.store,"assessment.combinedPreview",payload.clone()).unwrap();payload["reviewDigest"]=preview["reviewDigest"].clone();payload["acknowledgeScope"]=json!(true);let made=call(&f.store,"assessment.combinedCreate",payload).unwrap();let aid=made["assessmentId"].as_str().unwrap();let s=f.store.snapshot().unwrap();let reference=refs(&s,aid).into_iter().find(|r|resolve_current_target(&s,aid,Some(r)).unwrap().kind=="web").unwrap();let before=serde_json::to_string(&s).unwrap();let count=std::cell::Cell::new(0);
        assert!(scope::with_selected_admission(&f.store,aid,"http.headers.v1",None,std::slice::from_ref(&reference),"fixture.denied",|_|{count.set(count.get()+1);Ok(())}).unwrap_err().contains("not_yet_open"));assert_eq!(count.get(),0);assert_eq!(serde_json::to_string(&f.store.snapshot().unwrap()).unwrap(),before);
        let body=current_body(&s,aid).unwrap();assert!(validate_selection(body,&[reference.clone(),reference],Some("http.headers.v1")).is_err());
    }
}
