//! Exact, ephemeral reviews for report and recovery outputs. Preparation reads
//! selected local records but creates no output directory or file.
use super::*;
use super::decode;
use crate::{data_policy::*,data_projection::*,data_host::JournalResolver};

#[derive(Clone,Deserialize,Serialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub(super) struct ReportSelection {
    assessment_id:String,format:String,evidence_ids:Vec<String>,derived_subjects:Vec<DataSubject>,
    categories:Vec<DataCategory>,context_target_refs:Vec<crate::scope_targets::ScopeTargetRef>,package_sensitivity:Sensitivity,
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct BackupSelection{project_ids:Vec<String>,package_sensitivity:Sensitivity,acknowledge_recovery_purpose:bool}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct ReportCommit{data_review:DataReviewRef,acknowledge_export:bool,#[serde(default)]acknowledge_sensitive:bool}
#[derive(Deserialize)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
struct BackupCommit{data_review:DataReviewRef,acknowledge_recovery_backup:bool,#[serde(default)]acknowledge_sensitive:bool}

#[derive(Clone)]
pub(super) enum OutputReview{
    Report{reference:DataReviewRef,id:String,created_at:String,input:ReportSelection,spec:DataUseSpec},
    Backup{reference:DataReviewRef,id:String,project_ids:Vec<String>,package_sensitivity:Sensitivity,spec:DataUseSpec,inventory:Vec<data::DatabaseInventory>},
}
impl OutputReview{fn reference(&self)->&DataReviewRef{match self{Self::Report{reference,..}|Self::Backup{reference,..}=>reference}}}

struct ReportPacket{spec:DataUseSpec,files:Vec<(String,Vec<u8>)>,manifest:Value,markdown:String,omissions:Vec<Value>}
fn bytes_digest(name:&str,bytes:&[u8])->ArtifactDigest{ArtifactDigest{name:name.into(),sha256:format!("{:x}",Sha256::digest(bytes)),bytes:bytes.len() as u64}}
fn review_expired(reference:&DataReviewRef)->bool{chrono::DateTime::parse_from_rfc3339(&reference.expires_at).map_or(true,|v|v<=chrono::Utc::now())}
fn fail(code:&str)->String{format!("{code}: revise o pacote antes de gravar uma nova cópia.")}
fn redact_tree(value:Value,r:&dyn DataRedactor)->Value{match value{
    Value::String(s)=>json!(r.redact(&s).text),Value::Array(a)=>Value::Array(a.into_iter().map(|v|redact_tree(v,r)).collect()),
    Value::Object(o)=>Value::Object(o.into_iter().map(|(k,v)|(k,redact_tree(v,r))).collect()),other=>other,
}}

fn add_subject(store:&Store,state:&Snapshot,subject:&DataSubject,spec:&mut DataUseSpec,redactor:&dyn DataRedactor)->Result<Value,String>{
    let resolved=resolve_subject(store,state,subject,&JournalResolver{store})?;
    if spec.operation!=DataOperation::RecoveryBackup{require_known_origin(&resolved)?;}
    let classification=classification_state(state,&resolved)?;
    if classification.secret_material{return Err(fail("data_secret_material"));}
    if !spec.subjects.iter().any(|v|v.subject==*subject){spec.subjects.push(SubjectVersion{subject:subject.clone(),source_fingerprint:resolved.fingerprint.clone(),classification_ref:classification.classification_ref});}
    for origin in &resolved.origins{if !spec.selection.origins.contains(origin){spec.selection.origins.push(origin.clone());}}
    for contributor in &resolved.contributors{
        // Record every transitive subject version rather than accepting a class
        // inherited from a stale renderer projection.
        if !spec.subjects.iter().any(|v|v.subject==contributor.subject){let _=add_subject(store,state,&contributor.subject,spec,redactor)?;}
    }
    // A remediation's full local record is available only through a reviewed
    // local window. Reports retain the established whitelist: source contents,
    // predicate bytes and rollback material are not report fields.
    let value=if spec.operation==DataOperation::ReportExport{
        if let DataSubject::Derived{assessment_id,producer:DerivedProducer::Remediation,object_id}=subject{
            crate::remediation::report_metadata(&store.root.join("remediation.sqlite3"),state,assessment_id)?.into_iter().find(|v|v["id"]==*object_id).ok_or_else(||fail("data_source_unknown"))?
        }else{resolved.value}
    }else{resolved.value};
    Ok(redact_tree(value,redactor))
}

fn report_packet(store:&Store,input:&ReportSelection,id:&str,created_at:&str,redactor:&dyn DataRedactor)->Result<ReportPacket,String>{
    if matches!(redactor.mode(),RedactionMode::MetadataOnly){return Err(fail("data_redactor_unavailable"));}
    if input.format!="markdown_bundle"||input.evidence_ids.len()>100||input.derived_subjects.len()>100||input.categories.len()>7||input.context_target_refs.len()>8{return Err(fail("data_limit"));}
    core::valid_id(&input.assessment_id)?;
    let state=store.snapshot()?;
    let assessment=state.assessments.iter().find(|a|a.id==input.assessment_id).ok_or("Avaliação não encontrada.")?;
    let scope_ref=crate::scope::current_ref(&state,&assessment.id)?.ok_or_else(||fail("data_source_unknown"))?;
    let combined=crate::scope_targets::is_combined(&state,&assessment.id);
    if !input.context_target_refs.is_empty()||!combined{crate::scope_targets::validate_selection(crate::scope_targets::current_body(&state,&assessment.id)?,&input.context_target_refs,None)?;}
    let all_refs=if combined{crate::scope_targets::current_body(&state,&assessment.id)?.target_set.as_ref().map(|set|set.entries.len()).unwrap_or(0)}else{0};
    let mut spec=DataUseSpec::new(DataOperation::ReportExport,DataDestination::ReportExport,vec![assessment.project_id.clone()],Some(assessment.id.clone()));
    spec.scope_ref=Some(scope_ref.clone());
    spec.selection=DataSelection{categories:input.categories.clone(),origins:vec![],evidence_ids:input.evidence_ids.clone(),derived_subjects:input.derived_subjects.clone(),package_sensitivity:input.package_sensitivity};
    let mut omissions=vec![];let mut sections=vec![];let mut files=vec![];let mut evidence_records=vec![];
    let mut unique=HashSet::new();
    for category in &input.categories{
        if !unique.insert(format!("{category:?}")){return Err(fail("data_invalid_request"));}
        if matches!(category,DataCategory::HumanText|DataCategory::ExecutionSummary|DataCategory::ActivityMetadata){
            if combined&&input.context_target_refs.len()!=all_refs&&*category!=DataCategory::HumanText{
                omissions.push(json!({"category":category,"reasonCode":"unselected_source_in_aggregate"}));continue;
            }
            let subject=DataSubject::AssessmentMetadata{assessment_id:assessment.id.clone(),category:*category};
            let value=add_subject(store,&state,&subject,&mut spec,redactor)?;
            sections.push(json!({"subject":subject,"representation":"redacted_derivative","value":value}));
        }
    }
    if input.categories.contains(&DataCategory::TargetMetadata){
        if combined{
            for reference in &input.context_target_refs{
                crate::scope_targets::resolve_current_target(&state,&assessment.id,Some(reference))?;
                let subject=DataSubject::ScopeSource{assessment_id:assessment.id.clone(),scope_ref:scope_ref.clone(),target_ref:Some(reference.clone())};
                sections.push(json!({"subject":subject,"representation":"redacted_derivative","value":add_subject(store,&state,&subject,&mut spec,redactor)?}));
            }
        }else{
            if !input.context_target_refs.is_empty(){return Err(fail("data_source_unknown"));}
            let subject=DataSubject::ScopeSource{assessment_id:assessment.id.clone(),scope_ref:scope_ref.clone(),target_ref:None};
            sections.push(json!({"subject":subject,"representation":"redacted_derivative","value":add_subject(store,&state,&subject,&mut spec,redactor)?}));
        }
    }
    if !input.evidence_ids.is_empty()&&!input.categories.contains(&DataCategory::EvidenceExcerpt)&&!input.categories.contains(&DataCategory::SourceExcerpt){return Err(fail("data_invalid_request"));}
    let mut selected=HashSet::new();
    for evidence_id in &input.evidence_ids{
        if !selected.insert(evidence_id){return Err(fail("data_invalid_request"));}
        let evidence=assessment.evidence.iter().find(|e|e.id==*evidence_id).ok_or("Evidência fora desta avaliação.")?;
        if core::hash(&evidence.content)!=evidence.sha256{return Err("Integridade da evidência não confirmada.".into());}
        let subject=DataSubject::Evidence{assessment_id:assessment.id.clone(),evidence_id:evidence.id.clone()};
        let _=add_subject(store,&state,&subject,&mut spec,redactor)?;
        let origin=resolve_subject(store,&state,&subject,&JournalResolver{store})?;
        if origin.categories.iter().any(|category|!input.categories.contains(category)){
            return Err("data_category_required: a evidência selecionada contém uma captura de fontes; selecione e revise a categoria correspondente antes de exportar.".into());
        }
        let content=redactor.redact(&evidence.content).text;
        let name=format!("evidence-{}.txt",evidence.id);
        evidence_records.push(json!({"id":evidence.id,"name":redactor.redact(&evidence.name).text,"rawSha256":evidence.sha256,"rawBytes":evidence.content.len(),"exportedSha256":core::hash(&content),"exportedBytes":content.len(),"file":name,"representation":"redacted_derivative","originRefs":origin.origins,"targetRefs":origin.origins.iter().filter_map(|r|r.target_ref.clone()).collect::<Vec<_>>(),"sourceVerified":origin.origin_verified}));
        files.push((name,content.into_bytes()));
    }
    for evidence in &assessment.evidence{if !selected.contains(&evidence.id){omissions.push(json!({"subjectId":evidence.id,"reasonCode":"not_selected"}));}}
    for subject in &input.derived_subjects{
        if subject.assessment_id()!=assessment.id||!matches!(subject,DataSubject::Derived{..}){return Err(fail("data_source_unknown"));}
        let resolved=resolve_subject(store,&state,subject,&JournalResolver{store})?;
        if resolved.categories.iter().any(|c|!input.categories.contains(c)){return Err("data_category_required: o histórico selecionado inclui outras categorias de dados. Revise as categorias do pacote completo antes de exportar.".into());}
        sections.push(json!({"subject":subject,"representation":"redacted_derivative","value":add_subject(store,&state,subject,&mut spec,redactor)?}));
    }
    // A contributor cannot add a source that the human left unselected.
    if combined&&spec.selection.origins.iter().any(|origin|origin.target_ref.as_ref().is_none_or(|r|!input.context_target_refs.contains(r))){return Err(fail("data_source_unknown"));}
    let mut manifest=json!({"schema":"openkoda.reviewed-report.v1","reportId":id,"createdAt":created_at,"appVersion":env!("CARGO_PKG_VERSION"),"assessmentId":assessment.id,"projectId":assessment.project_id,"partial":true,"selection":spec.selection,"sections":sections,"evidence":evidence_records,"omissions":omissions,"limitations":["Pacote dos registros selecionados; omissões e limites não equivalem a ausência de vulnerabilidades.","Representações redigidas são derivados; os hashes do original identificam a evidência local preservada.","A revisão de sensibilidade não confirma alegações nem amplia autorização operacional.","Associação declarada pelo usuário não confirma a origem factual nem a veracidade do conteúdo."]});
    manifest["assessment"]=json!({"id":assessment.id,"status":assessment.status,"targetRole":"presentation_anchor_only"});manifest["project"]=json!({"id":assessment.project_id});
    let mut findings=vec![];let mut validations=vec![];let mut remediations=vec![];
    for section in &sections{
        let subject:DataSubject=serde_json::from_value(section["subject"].clone()).map_err(|_|fail("data_invalid_request"))?;let value=&section["value"];
        match subject{
            DataSubject::AssessmentMetadata{category:DataCategory::HumanText,..}=>{for key in ["title","objective","summary"]{manifest["assessment"][key]=value[key].clone();}manifest["project"]["name"]=value["projectName"].clone();},
            DataSubject::AssessmentMetadata{category:DataCategory::ExecutionSummary,..}=>{manifest["executions"]=value["executions"].clone();manifest["coverage"]=value["coverage"].clone();},
            DataSubject::AssessmentMetadata{category:DataCategory::ActivityMetadata,..}=>manifest["activity"]=value["events"].clone(),
            DataSubject::Derived{producer:DerivedProducer::Finding,..}=>{let resolved=resolve_subject(store,&state,&subject,&JournalResolver{store})?;let mut value=value.clone();value["targetRefs"]=json!(resolved.origins.iter().filter_map(|r|r.target_ref.clone()).collect::<Vec<_>>());value["sourceVerified"]=json!(resolved.origin_verified);findings.push(value);},
            DataSubject::Derived{producer:DerivedProducer::Validation,..}=>validations.push(value.clone()),
            DataSubject::Derived{producer:DerivedProducer::Remediation,..}=>remediations.push(value.clone()),
            _=>{},
        }
    }
    let mut selected_state=state.clone();selected_state.findings.retain(|f|findings.iter().any(|v|v["id"]==f.id));
    if input.categories.contains(&DataCategory::TargetMetadata)&&input.categories.contains(&DataCategory::ExecutionSummary){
        manifest["httpValidations"]=redact_tree(json!(crate::http_validation::report_rows(&selected_state,&assessment.id)?),redactor);
    }else{
        manifest["httpValidations"]=json!([]);
        if !selected_state.findings.is_empty(){omissions.push(json!({"category":"execution_summary","reasonCode":"http_observation_categories_not_selected"}));manifest["omissions"]=json!(omissions);}
    }
    manifest["findings"]=json!(findings);manifest["validations"]=json!(validations);manifest["remediations"]=json!(remediations);
    if input.categories.contains(&DataCategory::TargetMetadata){
        if !combined||input.context_target_refs.len()==all_refs{
            if input.categories.contains(&DataCategory::HumanText){manifest["scope"]=redact_tree(crate::scope::report_metadata(&state,&assessment.id)?,redactor);}
            manifest["assessment"]["target"]=json!(redactor.redact(&assessment.target).text);
        }
        if input.categories.contains(&DataCategory::ExecutionSummary){manifest["targetCoverage"]=redact_tree(json!(crate::scope::target_coverage(&state,&assessment.id).into_iter().filter(|r|r.get("targetRef").is_some_and(|v|input.context_target_refs.iter().any(|reference|json!(reference)==*v))).collect::<Vec<_>>()),redactor);}
    }
    let mut markdown=markdown_report(&manifest);
    if let Some(findings)=manifest["findings"].as_array(){for finding in findings{
        if let Some(references)=finding["targetRefs"].as_array(){markdown.push_str(&format!("\nAchado {} · Entradas de origem: {}.\n",markdown_text(finding["id"].as_str().unwrap_or_default()),references.iter().filter_map(|r|r["entryId"].as_str()).map(|id|format!("`{id}`")).collect::<Vec<_>>().join(", ")));}
        if finding["sourceVerified"]==false{markdown.push_str("Associação declarada pelo usuário; não confirma a origem factual nem a veracidade do conteúdo.\n");}
    }}
    if combined{markdown.push_str("\nO alvo do cabeçalho é apenas a referência de apresentação.\n");if let Some(targets)=manifest["targetCoverage"].as_array(){for target in targets{if let Some(methods)=target["methods"].as_array(){for method in methods{markdown.push_str(&format!("- {}: {}\n",markdown_text(method["method"].as_str().unwrap_or_default()),markdown_text(method["state"].as_str().unwrap_or_default())));}}}}}
    markdown.push_str(&format!("\n## Omissões e limites\n\n{} registros omitidos. Consulte o manifest para seleção e motivos exatos.\n",omissions.len()));
    files.push(("manifest.json".into(),serde_json::to_vec_pretty(&manifest).map_err(|_|fail("data_invalid_request"))?));files.push(("report.md".into(),markdown.as_bytes().to_vec()));
    let checksums=files.iter().map(|(name,bytes)|bytes_digest(name,bytes)).collect::<Vec<_>>();
    files.push(("checksums.json".into(),serde_json::to_vec_pretty(&checksums).map_err(|_|fail("data_invalid_request"))?));
    if files.iter().map(|(_,b)|b.len()).sum::<usize>()>8*1024*1024{return Err(fail("data_limit"));}
    spec.artifacts=files.iter().map(|(name,bytes)|bytes_digest(name,bytes)).collect();
    spec.destination_context=json!({"format":input.format,"reportId":id,"redactorVersion":"openkoda.redactor.v1"});
    let inputs=serde_json::to_string(&spec.subjects).map_err(|_|fail("data_invalid_request"))?;
    let outputs=serde_json::to_string(&spec.artifacts).map_err(|_|fail("data_invalid_request"))?;
    spec.requests=vec![RequestFingerprint{index:0,input_sha256:core::hash(&inputs),output_sha256:core::hash(&outputs),input_bytes:inputs.len() as u64,output_bytes:spec.artifacts.iter().map(|a|a.bytes).sum()}];
    Ok(ReportPacket{spec,files,manifest,markdown,omissions})
}

impl RecoveryService{
    fn remember_review(&self,pending:OutputReview)->Result<(),String>{
        let mut reviews=self.reviews.lock().map_err(|_|fail("data_limit"))?;
        reviews.retain(|_,r|!review_expired(r.reference()));
        if reviews.len()>=32{return Err(fail("data_limit"));}
        reviews.insert(pending.reference().id.clone(),pending);Ok(())
    }
    fn remembered(&self,reference:&DataReviewRef)->Result<OutputReview,String>{
        let reviews=self.reviews.lock().map_err(|_|fail("data_limit"))?;
        let value=reviews.get(&reference.id).ok_or_else(||fail("data_review_required"))?;
        if value.reference()!=reference{return Err(fail("data_review_changed"));}
        if review_expired(reference){return Err(fail("data_review_expired"));}Ok(value.clone())
    }
    pub(crate) fn handle_reviewed(&self,action:&str,payload:Value,store:&Store,policy:&DataPolicyService,redactor:&dyn DataRedactor)->Result<Value,String>{
        if payload.to_string().len()>64*1024{return Err(fail("data_limit"));}
        let _operation=self.operation.try_lock().map_err(|_|"Uma exportação ou recuperação já está em andamento.")?;
        match action{
            "report.prepare"=>{
                let input:ReportSelection=decode(payload)?;let id=model::id();let created_at=model::now();
                let packet=report_packet(store,&input,&id,&created_at,redactor)?;
                let decision=DataPolicyService::check_use(&store.snapshot()?,&packet.spec)?;
                // Denial must not become a second route for unrestricted local
                // content. Only an allowed selection receives the exact packet.
                if !decision.allowed{return Ok(json!({"dataReview":null,"reportId":id,"manifestPreview":null,"markdownPreview":"","artifacts":[],"omissions":[],"canExport":false,"requiresSensitiveConfirmation":decision.effective_sensitivity==Sensitivity::Sensitive,"reasonCodes":decision.reason_codes,"decision":decision}));}
                let review=policy.prepare_use(store,packet.spec.clone())?;
                if review.decision.allowed{self.remember_review(OutputReview::Report{reference:review.review_ref.clone(),id:id.clone(),created_at,input,spec:packet.spec.clone()})?;}
                Ok(json!({"dataReview":review.review_ref,"reportId":id,"manifestPreview":packet.manifest,"markdownPreview":packet.markdown,"artifacts":packet.spec.artifacts,"omissions":packet.omissions,"canExport":review.decision.allowed,"requiresSensitiveConfirmation":review.requires_sensitive_confirmation,"reasonCodes":review.decision.reason_codes,"decision":review.decision}))
            },
            "report.commit"=>{
                let input:ReportCommit=decode(payload)?;if !input.acknowledge_export{return Err(fail("data_review_required"));}
                let OutputReview::Report{reference,id,created_at,input:selection,spec:expected}=self.remembered(&input.data_review)? else{return Err(fail("data_review_changed"));};
                let packet=report_packet(store,&selection,&id,&created_at,redactor)?;
                if packet.spec!=expected{return Err(fail("data_review_changed"));}
                let admission=policy.with_admission(store,&reference,&packet.spec,None,EffectLink{kind:"report".into(),id:id.clone()},input.acknowledge_sensitive,|admission|Ok(admission.clone()))?;
                let admitted=match admission{AdmissionResult::AlreadyUsed(receipt)=>return Ok(json!({"alreadyUsed":true,"receiptRef":receipt,"reportId":id})),AdmissionResult::Admitted(admitted)=>admitted};
                policy.before_dispatch(store,&admitted.receipt_ref,&packet.spec,None,||Ok(()))?;
                let result=(||{
                    let base=self.directory("exports")?;let staging=base.join(format!("report-{id}.incomplete"));fs::create_dir(&staging).map_err(|_|"Não foi possível criar o destino exclusivo de exportação.")?;
                    for (name,bytes) in &packet.files{write_new(&staging.join(name),bytes)?;}
                    let folder=base.join(format!("report-{id}"));if folder.exists(){return Err("Destino já existe; nenhuma cópia foi substituída.".into());}fs::rename(&staging,&folder).map_err(|_|"Exportação incompleta preservada para inspeção.")?;
                    Ok(json!({"receiptRef":admitted.receipt_ref,"reportId":id,"id":id,"path":folder.join("report.md"),"manifestPath":folder.join("manifest.json"),"directory":folder,"partial":true,"completed":true,"reviewRequired":true}))
                })();
                match result{Ok(value)=>{policy.finish(store,&admitted.receipt_ref,DataUseOutcome::Completed,packet.spec.artifacts)?;Ok(value)},Err(error)=>{let _=policy.finish(store,&admitted.receipt_ref,DataUseOutcome::Failed,vec![]);Err(error)}}
            },
            "backup.prepare"=>self.prepare_backup_review(decode(payload)?,store,policy,redactor),
            "backup.commit"=>self.commit_backup_review(decode(payload)?,store,policy,redactor),
            _=>Err(fail("data_invalid_request")),
        }
    }
}

struct BackupPacket{spec:DataUseSpec,databases:Vec<data::LogicalDatabase>,classes:Vec<Value>}
fn backup_exclusions()->Vec<&'static str>{vec![
    "Credenciais e confirmação da conexão com o provedor; valores nunca entram na cópia.",
    "Orçamento e identidade monetária da instalação permanecem na base global e não são restaurados por cópia de perfil.",
    "Sistema de arquivos e processos da distribuição Kali, fontes externas e relatórios já exportados não fazem parte desta cópia.",
    "Políticas restauradas ficam inativas até revisão prospectiva; recibos históricos permanecem preservados.",
]}
fn backup_packet(service:&RecoveryService,store:&Store,id:&str,project_ids:&[String],package_sensitivity:Sensitivity,redactor:&dyn DataRedactor)->Result<BackupPacket,String>{
    if matches!(redactor.mode(),RedactionMode::MetadataOnly){return Err(fail("data_redactor_unavailable"));}
    let state=store.snapshot()?;
    store.work_read(crate::work_supervisor::ensure_quiescent_locked)?;
    if state.assessments.iter().any(|a|core::active(&a.status)){return Err("Reconcilie ou encerre avaliações ativas antes do backup.".into());}
    let mut expected=state.projects.iter().map(|p|p.id.clone()).collect::<Vec<_>>();expected.sort();
    let mut selected=project_ids.to_vec();selected.sort();
    if expected!=selected{return Err("O backup de recuperação inclui todos os projetos; revise novamente o inventário completo.".into());}
    if service.root.join("environment.sqlite3").exists()&&service.root.join("environment-history.sqlite3").exists(){return Err("Ambiente ativo e histórico têm destinos conflitantes; os originais foram preservados.".into());}
    let mut spec=DataUseSpec::new(DataOperation::RecoveryBackup,DataDestination::RecoveryBackup,selected.clone(),None);
    spec.selection.categories=vec![DataCategory::HumanText,DataCategory::TargetMetadata,DataCategory::ExecutionSummary,DataCategory::EvidenceExcerpt,DataCategory::SourceExcerpt,DataCategory::FindingText,DataCategory::ActivityMetadata];
    spec.selection.package_sensitivity=package_sensitivity;
    let resolver=JournalResolver{store};
    let mut classes=vec![];
    for project in &state.projects{
        let mut sensitivities=vec![];
        for assessment in state.assessments.iter().filter(|a|a.project_id==project.id){
            let mut subjects=vec![];
            for category in [DataCategory::HumanText,DataCategory::ExecutionSummary,DataCategory::ActivityMetadata]{subjects.push(DataSubject::AssessmentMetadata{assessment_id:assessment.id.clone(),category});}
            if let Some(scope_ref)=crate::scope::current_ref(&state,&assessment.id)?{
                if crate::scope_targets::is_combined(&state,&assessment.id){
                    let body=crate::scope_targets::current_body(&state,&assessment.id)?;
                    if let Some(targets)=&body.target_set{
                        for entry in &targets.entries{
                            // The resolver computes/validates the current entry
                            // digest; no label is used as source authority.
                            let reference=crate::scope_targets::entry_ref(entry);
                            subjects.push(DataSubject::ScopeSource{assessment_id:assessment.id.clone(),scope_ref:scope_ref.clone(),target_ref:Some(reference)});
                        }
                    }
                }else{subjects.push(DataSubject::ScopeSource{assessment_id:assessment.id.clone(),scope_ref,target_ref:None});}
            }
            for evidence in &assessment.evidence{subjects.push(DataSubject::Evidence{assessment_id:assessment.id.clone(),evidence_id:evidence.id.clone()});}
            for finding in state.findings.iter().filter(|f|f.assessment_id==assessment.id){subjects.push(DataSubject::Derived{assessment_id:assessment.id.clone(),producer:DerivedProducer::Finding,object_id:finding.id.clone()});}
            for validation in state.validations.iter().filter(|v|v.assessment_id==assessment.id){subjects.push(DataSubject::Derived{assessment_id:assessment.id.clone(),producer:DerivedProducer::Validation,object_id:validation.id.clone()});}
            subjects.extend(resolver.list(&assessment.id)?);
            for subject in subjects{
                let resolved=resolve_subject(store,&state,&subject,&resolver)?;
                let classification=classification_state(&state,&resolved)?;
                if !sensitivities.contains(&classification.sensitivity){sensitivities.push(classification.sensitivity);}
                if classification.sensitivity==Sensitivity::Unclassified{spec.selection.package_sensitivity=Sensitivity::Unclassified;}
                else if spec.selection.package_sensitivity!=Sensitivity::Unclassified{spec.selection.package_sensitivity=spec.selection.package_sensitivity.max(classification.sensitivity);}
                let _=add_subject(store,&state,&subject,&mut spec,redactor)?;
            }
        }
        sensitivities.sort();classes.push(json!({"projectId":project.id,"sensitivities":sensitivities,"reasonCodes":if sensitivities.contains(&Sensitivity::Unclassified){vec!["data_classification_required"]}else{vec![]}}));
    }
    let mut databases=vec![];let mut total=0u64;
    for name in DATABASES{
        let path=service.root.join(name);if !path.exists(){continue;}
        let database=data::prepare_database_with_managed(&path,name,&|text|redactor.redact(text).secret_material,&|text|redactor.contains_managed_secret(text))?;
        total=total.checked_add(database.inventory.logical_bytes).ok_or_else(||fail("data_limit"))?;
        if total>256*1024*1024{return Err(fail("data_limit"));}
        databases.push(database);
    }
    if !databases.iter().any(|db|db.inventory.name=="openkoda.sqlite3"){return Err("Banco principal não encontrado; nenhuma cópia foi gravada.".into());}
    let inventory=databases.iter().map(|db|db.inventory.clone()).collect::<Vec<_>>();
    spec.destination_context=json!({"backupId":id,"profileSnapshotDigest":core::hash(&serde_json::to_string(&state).map_err(|_|fail("data_invalid_request"))?),"projectIds":selected,"databaseSchemaDigests":inventory.iter().map(|i|json!({"name":i.name,"sha256":i.logical_sha256,"userVersion":i.user_version})).collect::<Vec<_>>()});
    spec.artifacts=inventory.iter().map(|i|ArtifactDigest{name:i.name.clone(),sha256:i.logical_sha256.clone(),bytes:i.logical_bytes}).collect();
    let inputs=serde_json::to_string(&spec.subjects).map_err(|_|fail("data_invalid_request"))?;
    let outputs=serde_json::to_string(&inventory).map_err(|_|fail("data_invalid_request"))?;
    spec.requests=vec![RequestFingerprint{index:0,input_sha256:core::hash(&inputs),output_sha256:core::hash(&outputs),input_bytes:inputs.len() as u64,output_bytes:total}];
    spec.human_text.push(HumanTextDecision{field:"backup_metadata".into(),sensitivity:package_sensitivity,input_sha256:core::hash(&outputs),projected_sha256:core::hash(&outputs),bytes:total});
    Ok(BackupPacket{spec,databases,classes})
}

impl RecoveryService{
    fn prepare_backup_review(&self,input:BackupSelection,store:&Store,policy:&DataPolicyService,redactor:&dyn DataRedactor)->Result<Value,String>{
        if !input.acknowledge_recovery_purpose{return Err("Confirme que esta cópia é para recuperação local e pode preservar dados sensíveis.".into());}
        let id=model::id();let packet=backup_packet(self,store,&id,&input.project_ids,input.package_sensitivity,redactor)?;
        let inventory=packet.databases.iter().map(|db|db.inventory.clone()).collect::<Vec<_>>();
        let decision=DataPolicyService::check_use(&store.snapshot()?,&packet.spec)?;
        if !decision.allowed{return Ok(json!({"dataReview":null,"backupId":id,"projectIds":input.project_ids,"inventory":inventory,"classes":packet.classes,"exclusions":backup_exclusions(),"canBackup":false,"requiresSensitiveConfirmation":decision.effective_sensitivity==Sensitivity::Sensitive,"reasonCodes":decision.reason_codes,"decision":decision}));}
        let review=policy.prepare_use(store,packet.spec.clone())?;
        if review.decision.allowed{self.remember_review(OutputReview::Backup{reference:review.review_ref.clone(),id:id.clone(),project_ids:input.project_ids.clone(),package_sensitivity:input.package_sensitivity,spec:packet.spec,inventory:inventory.clone()})?;}
        Ok(json!({"dataReview":review.review_ref,"backupId":id,"projectIds":input.project_ids,"inventory":inventory,"classes":packet.classes,"exclusions":backup_exclusions(),"canBackup":review.decision.allowed,"requiresSensitiveConfirmation":review.requires_sensitive_confirmation,"reasonCodes":review.decision.reason_codes,"decision":review.decision}))
    }
    fn commit_backup_review(&self,input:BackupCommit,store:&Store,policy:&DataPolicyService,redactor:&dyn DataRedactor)->Result<Value,String>{
        if !input.acknowledge_recovery_backup{return Err(fail("data_review_required"));}
        // A receipt is durable; rebuilding the package would include that very
        // receipt and change its logical snapshot. Replay has no new effect,
        // including after the ephemeral preparation has expired or restarted.
        if let Some(receipt)=store.snapshot()?.data_use_receipts.iter().find(|r|r.review_ref.id==input.data_review.id&&r.parent_receipt_ref.is_none()){
            if receipt.review_ref!=input.data_review||receipt.operation!=DataOperation::RecoveryBackup||receipt.destination!=DataDestination::RecoveryBackup||receipt.effect.kind!="backup"{return Err(fail("data_review_changed"));}
            return Ok(json!({"alreadyUsed":true,"receiptRef":receipt_ref(receipt),"backupId":receipt.effect.id}));
        }
        let OutputReview::Backup{reference,id,project_ids,package_sensitivity,spec:expected,inventory}=self.remembered(&input.data_review)? else{return Err(fail("data_review_changed"));};
        let packet=backup_packet(self,store,&id,&project_ids,package_sensitivity,redactor)?;
        if packet.spec!=expected||packet.databases.iter().map(|db|db.inventory.clone()).collect::<Vec<_>>()!=inventory{return Err(fail("data_review_changed"));}
        let admission=policy.with_admission(store,&reference,&packet.spec,None,EffectLink{kind:"backup".into(),id:id.clone()},input.acknowledge_sensitive,|a|Ok(a.clone()))?;
        let admitted=match admission{AdmissionResult::AlreadyUsed(receipt)=>return Ok(json!({"alreadyUsed":true,"receiptRef":receipt,"backupId":id})),AdmissionResult::Admitted(admitted)=>admitted};
        policy.before_dispatch(store,&admitted.receipt_ref,&packet.spec,None,||Ok(()))?;
        let result=(||{
            let base=self.directory("backups")?;let staging=base.join(format!("{id}.incomplete"));fs::create_dir(&staging).map_err(|_|"Não foi possível criar o destino exclusivo de recuperação.")?;
            let mut files=vec![];
            for database in &packet.databases{
                let target=staging.join(&database.inventory.name);database.write_new(&target)?;
                files.push(BackupFile{name:database.inventory.name.clone(),bytes:fs::metadata(&target).map_err(|_|"Cópia incompleta preservada.")?.len(),sha256:hash_file(&target)?,user_version:verify_database(&target)?});
            }
            validate_core_database(&staging.join("openkoda.sqlite3"))?;
            let manifest=BackupManifest{format:1,id:id.clone(),created_at:model::now(),app_version:env!("CARGO_PKG_VERSION").into(),consistency:"Reconstrução lógica de cada banco em memória a partir de registros ativos revisados. Exclui vault, confirmação de conexão e páginas SQLite liberadas. Snapshot do domínio anterior ao recibo deste backup; não inclui o sistema WSL.".into(),files,exclusions:backup_exclusions().into_iter().map(str::to_owned).collect()};
            write_new(&staging.join("manifest.json"),&serde_json::to_vec_pretty(&manifest).map_err(|_|fail("data_invalid_request"))?)?;
            let destination=base.join(&id);if destination.exists(){return Err("Destino já existe; nenhuma cópia foi substituída.".into());}fs::rename(&staging,&destination).map_err(|_|"Backup incompleto preservado para inspeção.")?;
            let (verified,_)=self.verify(&id)?;
            Ok(json!({"id":id,"backupId":id,"receiptRef":admitted.receipt_ref,"path":destination,"verified":true,"manifest":verified,"inventory":inventory}))
        })();
        match result{Ok(value)=>{let results=value["manifest"]["files"].as_array().into_iter().flatten().map(|f|ArtifactDigest{name:f["name"].as_str().unwrap_or_default().into(),sha256:f["sha256"].as_str().unwrap_or_default().into(),bytes:f["bytes"].as_u64().unwrap_or_default()}).collect();policy.finish(store,&admitted.receipt_ref,DataUseOutcome::Completed,results)?;Ok(value)},Err(error)=>{let _=policy.finish(store,&admitted.receipt_ref,DataUseOutcome::Failed,vec![]);Err(error)}}
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    struct Redactor;
    impl DataRedactor for Redactor{
        fn mode(&self)->RedactionMode{RedactionMode::NoCredentialConfigured}
        fn redact(&self,text:&str)->RedactedText{let projected=crate::ai::redact(text,None);RedactedText{changed:projected!=text,secret_material:projected.contains("[REDACTED"),text:projected}}
    }
    struct Fixture{_root:tempfile::TempDir,store:Store,recovery:RecoveryService,policy:DataPolicyService,pid:String,aid:String}
    #[test]
    fn reviewed_workflow_backup_restores_sealed_bytes_with_paused_heads_and_no_authority(){
        let(_dir,store,aid,manifest)=crate::capture::fixture::build(b"recoverable bounded observation",crate::capture::CaptureState::Sealed);
        let recovery=RecoveryService::new(store.root.clone()).unwrap();
        fs::write(store.root.join("ai-budget.sqlite3"),b"installation ledger unchanged").unwrap();
        let revisions_before=store.work_read(|conn,_|{let mut q=conn.prepare("SELECT data FROM work_plan_revisions ORDER BY id").map_err(|e|e.to_string())?;let rows=q.query_map([],|r|r.get::<_,String>(0)).map_err(|e|e.to_string())?.collect::<Result<Vec<_>,_>>().map_err(|e|e.to_string())?;Ok(rows)}).unwrap();
        let backup=crate::recovery::test_support::backup(&recovery,&store).unwrap();
        let restored=recovery.restore(backup["id"].as_str().unwrap()).unwrap();let root=PathBuf::from(restored["path"].as_str().unwrap());
        assert!(!root.join("ai-budget.sqlite3").exists());let copy=Store::open(root).unwrap();
        copy.work_read(|conn,s|{
            let captured=crate::capture::load_locked(conn,&manifest.id)?;assert_eq!(captured,manifest);assert_eq!(crate::capture::preserved_locked(conn,&captured)?,b"recoverable bounded observation");
            let head=crate::work_store::head_for_assessment(conn,&aid)?.unwrap();assert_eq!(head.control,crate::workflow::PlanControl::Paused);
            assert_eq!(conn.query_row("SELECT COUNT(*) FROM work_leases",[],|r|r.get::<_,i64>(0)).unwrap(),0);
            assert!(s.data_policies.iter().all(|p|!p.active));assert!(!s.settings.has_api_key);
            let mut q=conn.prepare("SELECT data FROM work_plan_revisions ORDER BY id").unwrap();assert_eq!(q.query_map([],|r|r.get::<_,String>(0)).unwrap().collect::<Result<Vec<_>,_>>().unwrap(),revisions_before);
            crate::work_supervisor::ensure_quiescent_locked(conn,s)
        }).unwrap();
        let supervisor=crate::work_supervisor::WorkSupervisor::default();assert!(supervisor.next_native(&copy).unwrap().is_none());
        assert_eq!(fs::read(store.root.join("ai-budget.sqlite3")).unwrap(),b"installation ledger unchanged");
        store.work_read(|conn,_|{assert_eq!(crate::capture::load_locked(conn,&manifest.id)?,manifest);Ok(())}).unwrap();
    }
    #[test]
    fn logical_backup_refuses_managed_secret_split_between_capture_chunks(){
        let secret="SYNTHETIC-MANAGED-CAPTURE-SECRET-ONLY-FOR-TEST";
        let mut bytes=vec![b'x';crate::capture::MAX_CHUNK_BYTES-17];bytes.extend_from_slice(secret.as_bytes());bytes.extend_from_slice(b" trailing data");
        let(_dir,store,_aid,manifest)=crate::capture::fixture::build(&bytes,crate::capture::CaptureState::Sealed);
        let source=store.root.join("openkoda.sqlite3");
        let result=data::prepare_database_with_managed(&source,"openkoda.sqlite3",&|_|false,&|text|text.contains(secret));
        assert!(result.is_err(),"a secret split across rows must never enter a recovery copy");
        assert!(!store.root.join("backups").exists());
        store.work_read(|conn,_|{assert_eq!(crate::capture::preserved_locked(conn,&manifest)?,bytes);Ok(())}).unwrap();
    }
    impl Fixture{
        fn new()->Self{
            let root=tempfile::tempdir().unwrap();let store=Store::open(root.path().into()).unwrap();let recovery=RecoveryService::new(root.path().into()).unwrap();let policy=DataPolicyService::new();
            let project=store.handle(core::Request{action:"project.create".into(),payload:json!({"name":"Reviewed recovery fixture","kind":"web","target":"https://fixture.invalid","description":"Local synthetic records"})}).unwrap();
            let pid=project["id"].as_str().unwrap().to_owned();let assessment=store.handle(core::Request{action:"assessment.create".into(),payload:json!({"projectId":pid,"title":"Report fixture","kind":"web","target":"https://fixture.invalid","objective":"Selected synthetic observations only","scope":crate::scope::test_creation(&store,&pid,&[])})}).unwrap();let aid=assessment["id"].as_str().unwrap().to_owned();
            Self{_root:root,store,recovery,policy,pid,aid}
        }
        fn authorize(&self){crate::data_policy::test_support::authorize(&self.store,&self.policy,&self.aid,&Redactor,&JournalResolver{store:&self.store},None).unwrap();}
        fn report(&self)->Value{json!({"assessmentId":self.aid,"format":"markdown_bundle","evidenceIds":[],"derivedSubjects":[],"categories":["human_text","target_metadata"],"contextTargetRefs":[],"packageSensitivity":"public"})}
        fn call(&self,action:&str,payload:Value)->Result<Value,String>{self.recovery.handle_reviewed(action,payload,&self.store,&self.policy,&Redactor)}
    }
    #[test]
    fn old_output_routes_and_unreviewed_packages_create_no_files(){
        let f=Fixture::new();
        for (action,payload) in [("backup.create",json!({})),("report.bundle",json!({"id":f.aid}))]{assert!(f.recovery.handle(action,payload,&f.store).unwrap_err().contains("data_review_required"));}
        assert!(f.store.handle(core::Request{action:"report.export".into(),payload:json!({"id":f.aid})}).unwrap_err().contains("data_review_required"));
        let report=f.call("report.prepare",f.report()).unwrap();assert_eq!(report["canExport"],false);assert!(report["dataReview"].is_null());
        assert!(!f.store.root.join("exports").exists());assert!(!f.store.root.join("backups").exists());assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());
    }
    #[test]
    fn denied_report_preview_cannot_disclose_full_content_without_a_data_grant(){
        for explicit_denial in [false,true]{
            let f=Fixture::new();let marker="DENIED-REPORT-TAIL-CANARY";
            f.store.mutate(|s|{s.assessments[0].summary=format!("{}\n{marker}","Synthetic human summary ".repeat(300));Ok(())}).unwrap();
            if explicit_denial{
                let mut body=default_body();body.local_raw_view.enabled=false;
                let current=f.policy.handle("data.policy.status",json!({"projectId":f.pid}),&f.store,&Redactor,&JournalResolver{store:&f.store},None).unwrap();
                let preview=f.policy.handle("data.policy.prepare",json!({"projectId":f.pid,"expectedStateDigest":current["stateDigest"],"body":body}),&f.store,&Redactor,&JournalResolver{store:&f.store},None).unwrap();
                f.policy.handle("data.policy.commit",json!({"review":preview["review"],"idempotencyKey":model::id(),"acknowledgePolicy":true}),&f.store,&Redactor,&JournalResolver{store:&f.store},None).unwrap();
            }
            let mut input=f.report();input["categories"]=json!(["human_text"]);
            let response=f.call("report.prepare",input.clone()).unwrap();assert_eq!(response["canExport"],false);assert!(response["dataReview"].is_null());
            assert!(!response.to_string().contains(marker),"A denied report returned full content");
            assert!(response["manifestPreview"].is_null());assert_eq!(response["markdownPreview"],"");assert_eq!(response["artifacts"],json!([]));
            assert!(!f.store.root.join("exports").exists());assert!(f.store.snapshot().unwrap().data_use_receipts.is_empty());
            if !explicit_denial{f.authorize();let permitted=f.call("report.prepare",input).unwrap();assert_eq!(permitted["canExport"],true);assert!(permitted["manifestPreview"].to_string().contains(marker));}
        }
    }
    #[test]
    fn report_writes_only_exact_reviewed_artifacts_and_replay_has_no_effect(){
        let f=Fixture::new();f.authorize();let preview=f.call("report.prepare",f.report()).unwrap();assert_eq!(preview["canExport"],true);assert!(!f.store.root.join("exports").exists());
        let payload=json!({"dataReview":preview["dataReview"],"acknowledgeExport":true});let result=f.call("report.commit",payload.clone()).unwrap();assert_eq!(result["completed"],true);assert_eq!(result["partial"],true);
        let directory=PathBuf::from(result["directory"].as_str().unwrap());
        for artifact in preview["artifacts"].as_array().unwrap(){let path=directory.join(artifact["name"].as_str().unwrap());assert_eq!(hash_file(&path).unwrap(),artifact["sha256"].as_str().unwrap());assert_eq!(fs::metadata(path).unwrap().len(),artifact["bytes"].as_u64().unwrap());}
        assert_eq!(fs::read_to_string(directory.join("report.md")).unwrap(),preview["markdownPreview"]);
        let repeated=f.call("report.commit",payload).unwrap();assert_eq!(repeated["alreadyUsed"],true);assert_eq!(fs::read_dir(f.store.root.join("exports")).unwrap().count(),1);
    }
    #[test]
    fn reclassification_invalidates_report_before_any_output(){
        let f=Fixture::new();f.authorize();let preview=f.call("report.prepare",f.report()).unwrap();
        let subject=DataSubject::AssessmentMetadata{assessment_id:f.aid.clone(),category:DataCategory::HumanText};let state=f.store.snapshot().unwrap();let resolved=resolve_subject(&f.store,&state,&subject,&JournalResolver{store:&f.store}).unwrap();
        let classification=f.policy.handle("data.classification.prepare",json!({"subject":subject,"expectedSourceFingerprint":resolved.fingerprint,"sensitivity":"internal","reason":"Fixture changed decision"}),&f.store,&Redactor,&JournalResolver{store:&f.store},None).unwrap();
        f.policy.handle("data.classification.commit",json!({"review":classification["review"],"idempotencyKey":model::id(),"acknowledgeClassification":true}),&f.store,&Redactor,&JournalResolver{store:&f.store},None).unwrap();
        assert!(f.call("report.commit",json!({"dataReview":preview["dataReview"],"acknowledgeExport":true})).is_err());assert!(!f.store.root.join("exports").exists());
    }
    #[test]
    fn reviewed_backup_restores_new_copy_with_inactive_policies(){
        let f=Fixture::new();f.authorize();let preview=f.call("backup.prepare",json!({"projectIds":[f.pid],"packageSensitivity":"internal","acknowledgeRecoveryPurpose":true})).unwrap();assert_eq!(preview["canBackup"],true);assert!(!f.store.root.join("backups").exists());
        let backup=f.call("backup.commit",json!({"dataReview":preview["dataReview"],"acknowledgeRecoveryBackup":true})).unwrap();assert_eq!(backup["verified"],true);
        let live=f.store.snapshot().unwrap();let restored=f.recovery.restore(backup["id"].as_str().unwrap()).unwrap();let restored_store=Store::open(PathBuf::from(restored["path"].as_str().unwrap())).unwrap();
        assert!(restored_store.snapshot().unwrap().data_policies.iter().all(|p|!p.active));assert!(f.store.snapshot().unwrap().data_policies.iter().all(|p|p.active));assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),serde_json::to_value(live).unwrap());
        assert!(!PathBuf::from(backup["path"].as_str().unwrap()).join("ai-budget.sqlite3").exists());
    }
    #[test]
    fn backup_replay_recovers_durable_receipt_without_rebuilding_or_writing(){
        let f=Fixture::new();f.authorize();
        let preview=f.call("backup.prepare",json!({"projectIds":[f.pid],"packageSensitivity":"internal","acknowledgeRecoveryPurpose":true})).unwrap();
        assert_eq!(preview["canBackup"],true);
        let request=json!({"dataReview":preview["dataReview"],"acknowledgeRecoveryBackup":true});
        let first=f.call("backup.commit",request.clone()).unwrap();
        let before=serde_json::to_value(f.store.snapshot().unwrap()).unwrap();
        let receipt_count=f.store.snapshot().unwrap().data_use_receipts.len();
        for restarted in [false,true]{
            let other=RecoveryService::new(f.store.root.clone()).unwrap();
            let replay=if restarted{other.handle_reviewed("backup.commit",request.clone(),&f.store,&DataPolicyService::new(),&Redactor)}else{f.call("backup.commit",request.clone())}.unwrap();
            assert_eq!(replay["alreadyUsed"],true);assert_eq!(replay["receiptRef"],first["receiptRef"]);assert_eq!(replay["backupId"],first["backupId"]);
            assert_eq!(f.store.snapshot().unwrap().data_use_receipts.len(),receipt_count);
            assert_eq!(serde_json::to_value(f.store.snapshot().unwrap()).unwrap(),before);
            assert_eq!(fs::read_dir(f.store.root.join("backups")).unwrap().count(),1);
        }
        let mut forged=request;forged["dataReview"]["digest"]=json!("0".repeat(64));
        assert!(f.call("backup.commit",forged).unwrap_err().contains("data_review_changed"));
    }
    #[test]
    fn classified_evidence_without_origin_requires_explicit_association_for_report(){
        let f=Fixture::new();let eid=model::id();let content="Synthetic legacy evidence without an origin";
        f.store.mutate(|s|{s.assessments[0].evidence.push(model::Evidence{id:eid.clone(),name:"legacy.txt".into(),content:content.into(),sha256:core::hash(content),created_at:model::now()});Ok(())}).unwrap();
        let subject=DataSubject::Evidence{assessment_id:f.aid.clone(),evidence_id:eid.clone()};
        let classify=|origins:Vec<DataOriginRef>|{
            let state=f.store.snapshot().unwrap();let resolved=resolve_subject(&f.store,&state,&subject,&JournalResolver{store:&f.store}).unwrap();
            let review=f.policy.handle("data.classification.prepare",json!({"subject":subject,"expectedSourceFingerprint":resolved.fingerprint,"sensitivity":"public","reason":"Synthetic origin review","associationRefs":origins}),&f.store,&Redactor,&JournalResolver{store:&f.store},None).unwrap();
            f.policy.handle("data.classification.commit",json!({"review":review["review"],"idempotencyKey":model::id(),"acknowledgeClassification":true}),&f.store,&Redactor,&JournalResolver{store:&f.store},None).unwrap();
        };
        classify(vec![]);f.authorize();
        let mut input=f.report();input["evidenceIds"]=json!([eid]);input["categories"]=json!(["human_text","target_metadata","evidence_excerpt"]);
        assert!(f.call("report.prepare",input.clone()).unwrap_err().contains("data_source_unknown"));assert!(!f.store.root.join("exports").exists());
        let origin=DataOriginRef{scope_ref:crate::scope::current_ref(&f.store.snapshot().unwrap(),&f.aid).unwrap().unwrap(),target_ref:None};classify(vec![origin.clone()]);
        let preview=f.call("report.prepare",input).unwrap();assert_eq!(preview["canExport"],true);assert_eq!(preview["manifestPreview"]["evidence"][0]["sourceVerified"],false);assert_eq!(preview["manifestPreview"]["evidence"][0]["originRefs"],json!([origin]));
    }
    #[test]
    fn source_capture_cannot_be_exported_as_generic_evidence_category(){
        let source=tempfile::tempdir().unwrap();fs::write(source.path().join("sample.py"),"print('synthetic selected source')\n").unwrap();
        let f=crate::executor::combined_fixture::Fixture::new(&[("code",source.path().to_str().unwrap(),&["source.rules.v1","source.capture.v1"])],&[],json!([]));
        let plan=f.plan(0,"source.rules.v1");let ai=crate::ai::AiService::new(f.store.root.clone()).unwrap();
        let input=json!({"assessmentId":f.aid,"planId":plan["id"],"paths":["sample.py"]});
        let preview=crate::source_context::handle("evidence.sourcePreview",input.clone(),&f.store,&ai).unwrap();let mut capture=input;capture["digest"]=preview["digest"].clone();capture["acknowledgeLocalCapture"]=json!(true);
        let evidence=crate::source_context::handle("evidence.sourceCapture",capture,&f.store,&ai).unwrap();
        let policy=DataPolicyService::new();let redactor=ai.data_redactor();crate::data_policy::test_support::authorize(&f.store,&policy,&f.aid,&redactor,&JournalResolver{store:&f.store},None).unwrap();
        let service=RecoveryService::new(f.store.root.clone()).unwrap();
        let input=json!({"assessmentId":f.aid,"format":"markdown_bundle","evidenceIds":[evidence["id"]],"derivedSubjects":[],"categories":["human_text","target_metadata","evidence_excerpt"],"contextTargetRefs":f.refs,"packageSensitivity":"public"});
        let result=service.handle_reviewed("report.prepare",input,&f.store,&policy,&redactor);
        assert!(result.as_ref().is_err_and(|error|error.contains("data_category_required")),"source excerpt accepted under evidence-only selection: {}",result.is_ok());
        assert!(!f.store.root.join("exports").exists());
    }
}
