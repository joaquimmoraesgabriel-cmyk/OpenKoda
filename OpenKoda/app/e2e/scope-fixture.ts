import type { Page } from '@playwright/test';
import type { ScopeMethod, ScopeRef, ScopeStatus } from '../src/scope';

declare global { interface Window { __scopeFixture:(state:any,methods:ScopeMethod[])=>{ref:ScopeRef;status:(id:unknown)=>ScopeStatus;statuses:Record<string,ScopeStatus>}; } }

/** Explicit, synthetic human-reviewed scope for pre-existing IPC contracts.
 * This never grants methods from the invoked action and is not native enforcement evidence. */
export async function installScopeFixture(page:Page){
  await page.addInitScript(()=>{
    window.__scopeFixture=(state,methods)=>{
      const labels:Record<string,string>={'local.inventory.v1':'Inventário local','source.rules.v1':'Análise local de fontes','source.capture.v1':'Captura seletiva de fontes','http.headers.v1':'Observação HTTP HEAD','network.tcp-connect.v1':'Observação TCP','kali.runtime-diagnostics.v1':'Diagnóstico Kali','kali.hash-evidence.v1':'Hash de evidência no Kali','kali.rg.signals.v1':'Sinais em evidência no Kali','ai.remote-review.v1':'Revisão remota com IA','local.text-remediation.v1':'Correção textual local'};
      const statuses:Record<string,any>={};
      state.scopeRevisions=[];state.assessmentScopes=[];state.scopeReceipts=[];
      for(const a of state.assessments){
        const local=['code','files'].includes(a.kind);const allowed=(method:string)=>method.startsWith('kali.')||method==='ai.remote-review.v1'||(local?['local.inventory.v1','source.rules.v1','source.capture.v1','local.text-remediation.v1'].includes(method):method===(a.kind==='web'?'http.headers.v1':'network.tcp-connect.v1'));
        if(methods.some(method=>!allowed(method)))throw new Error('Fixture selected incompatible scope methods');
        const catalog=Object.entries(labels).map(([id,label])=>({id,label,allowed:allowed(id),reasonCodes:allowed(id)?[]:['scope_method_incompatible'],effects:'Efeito sintético para revisão do contrato.',windowEnforcement:id.startsWith('kali.')||id==='ai.remote-review.v1'?'admission_only':'local_deadline'}));
        const revision={id:'revision-'+a.id,assessmentId:a.id,number:1,previousRevisionId:null,body:{objective:a.objective,kind:a.kind,target:a.target,methods:[...methods],window:{startsAt:null,endsAt:null}},createdAt:a.createdAt,origin:'human_review',review:{author:'local-user',reviewedAt:a.createdAt,acknowledgeScope:true},digest:'scope-digest-'+a.id};
        const frozen=a.status==='draft'?null:{revisionId:revision.id,digest:revision.digest,recordedAt:a.createdAt,triggerAction:'fixture.reviewed-work',reason:'first_work'};
        const archived=state.projects.find((p:any)=>p.id===a.projectId)?.archived===true;
        a.scopeId='scope-'+a.id;state.scopeRevisions.push(revision);state.assessmentScopes.push({id:a.scopeId,assessmentId:a.id,generation:1,currentRevisionId:revision.id,frozen,revocation:null,origin:null});
        statuses[a.id]={targetOptions:[],commonWorkGates:[],targetCoverage:[],schema:'openkoda.scope-status.v1',assessmentId:a.id,projectId:a.projectId,stateDigest:'state-'+a.id,phase:frozen?'frozen':'draft',revision,legacySummary:null,frozen,revocation:null,origin:null,windowStatus:'unbounded',evaluatedAt:a.createdAt,methodCatalog:catalog,actions:{canSave:!frozen&&!archived,canAdopt:false,canDerive:!archived,canRevoke:!!frozen&&!archived,canPrepareWork:!archived,canReviewStoredEvidence:!archived,canImportEvidence:!archived,reasonCodes:archived?['project_archived']:[]},freezeReasons:frozen?['fixture_work']:[],workGates:catalog.map(item=>({method:item.id,allowedByScope:!archived&&methods.includes(item.id as ScopeMethod),reasonCodes:archived?['project_archived']:methods.includes(item.id as ScopeMethod)?[]:['scope_method_not_selected']})),limits:['Fixture de contrato: não comprova autorização nativa.']};
      }
      const status=(id:unknown)=>{if(typeof id!=='string'||!statuses[id])throw new Error('Unknown assessment scope in fixture');return structuredClone(statuses[id]);};
      const refFor=(id:string)=>({assessmentId:id,revisionId:statuses[id].revision.id,digest:statuses[id].revision.digest});
      for(const plan of state.plans??[])plan.scopeRef=refFor(plan.assessmentId);
      return {ref:refFor(state.assessments[0].id),status,statuses};
    };
  });
}
