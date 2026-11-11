//! Bounded metadata-only identity preflight for explicitly selected local roots.
//! The lease must live through the caller's content operation. No enumeration,
//! hashing or source reads are performed for the other roots.
use crate::{model::Snapshot,scope::ScopeRef,scope_targets::ScopeTargetRef,source_scan::{self,MetadataLease}};
use std::path::Path;

pub struct LocalPreflight {pub refs:Vec<ScopeTargetRef>,_leases:Vec<MetadataLease>}
impl LocalPreflight {
    /// The exclusive consumer must compare this identity before reading. Only
    /// its selected file handle is released; all ancestors and other roots stay.
    pub fn release_selected_file(&mut self,target:Option<&ScopeTargetRef>)->Result<Option<crate::source_scan::FileIdentity>,String>{
        let Some(target)=target else{return Ok(None)};let index=self.refs.iter().position(|reference|reference==target).ok_or("Entrada escolhida ausente no preflight.")?;Ok(self._leases[index].release_file_for_exclusive())
    }
}

fn canonical(path:&Path)->String{path.to_string_lossy().trim_start_matches("\\\\?\\").replace('/',"\\").trim_end_matches('\\').to_uppercase()}
fn overlaps(left:&str,right:&str)->bool{left==right||left.starts_with(&(right.to_owned()+"\\"))||right.starts_with(&(left.to_owned()+"\\"))}

pub fn preflight(snapshot:&Snapshot,scope:&ScopeRef,selected:Option<&ScopeTargetRef>)->Result<LocalPreflight,String>{
    let resolved=crate::scope_targets::resolve_historical_target(snapshot,scope,selected)?;
    if !matches!(resolved.kind.as_str(),"code"|"files"){return Err("Preflight local exige uma fonte local explicitamente escolhida.".into());}
    let revision=snapshot.scope_revisions.iter().find(|r|r.id==scope.revision_id&&r.assessment_id==scope.assessment_id&&r.digest==scope.digest).ok_or("Escopo do preflight ausente.")?;
    let Some(set)=&revision.body.target_set else{return Ok(LocalPreflight{refs:vec![],_leases:vec![]})};
    reject_ambiguous_excluded_roots(set)?;
    let mut leases:Vec<MetadataLease>=vec![];let mut refs=vec![];
    for entry in &set.entries {
        if !matches!(entry.kind.as_str(),"code"|"files")||!entry.methods.iter().any(|method|matches!(method.as_str(),"local.inventory.v1"|"source.rules.v1"|"source.capture.v1"|"local.text-remediation.v1")){continue;}
        let reference=crate::scope_targets::target_ref(entry);
        let lease=source_scan::lock_metadata_target(Path::new(&entry.target))?;
        let name=canonical(&lease.path);
        for previous in &leases{if previous.identity==lease.identity||overlaps(&canonical(&previous.path),&name){return Err("scope_local_roots_overlap: raízes selecionadas duplicadas, sobrepostas ou aliases.".into());}}
        if lease.is_file&&set.local_exclusions.iter().any(|rule|rule.entry_id==entry.entry_id){return Err("Uma raiz de arquivo único não aceita exclusões relativas.".into());}
        refs.push(reference);leases.push(lease);
    }
    if refs.is_empty()||selected.is_none_or(|reference|!refs.contains(reference)){return Err("Preflight não possui autoridade local para a entrada escolhida.".into());}
    Ok(LocalPreflight{refs,_leases:leases})
}

pub fn validate_refs(snapshot:&Snapshot,scope:&ScopeRef,selected:Option<&ScopeTargetRef>,refs:&[ScopeTargetRef])->Result<(),String>{
    crate::scope_targets::resolve_historical_target(snapshot,scope,selected)?;
    let revision=snapshot.scope_revisions.iter().find(|r|r.id==scope.revision_id&&r.assessment_id==scope.assessment_id&&r.digest==scope.digest).ok_or("Escopo de identidade ausente.")?;
    if let Some(set)=&revision.body.target_set{reject_ambiguous_excluded_roots(set)?;}
    let expected=revision.body.target_set.as_ref().map(|set|set.entries.iter().filter(|entry|matches!(entry.kind.as_str(),"code"|"files")&&entry.methods.iter().any(|method|matches!(method.as_str(),"local.inventory.v1"|"source.rules.v1"|"source.capture.v1"|"local.text-remediation.v1"))).map(crate::scope_targets::target_ref).collect::<Vec<_>>()).unwrap_or_default();
    if expected!=refs{return Err("Referências do preflight não correspondem às raízes locais revisadas.".into());}Ok(())
}

fn reject_ambiguous_excluded_roots(set:&crate::scope_targets::ScopeTargetSet)->Result<(),String>{
    if set.excluded_project_targets.iter().any(|entry|matches!(entry.kind.as_str(),"code"|"files")){return Err("scope_excluded_local_identity_unproven: uma raiz local excluída pode ser alias de uma selecionada. Nenhum metadado ou conteúdo foi aberto; revise/derive o escopo usando exclusões relativas na raiz selecionada. Operações web não precisam desse preflight.".into());}Ok(())
}
