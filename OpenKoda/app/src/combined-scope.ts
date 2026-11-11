import type { Kind } from './types';
import { validScopeCatalog } from './scope';
import type { ScopeBody, ScopeCatalogEntry, ScopeMethod, ScopeRef, ScopeStatus } from './scope';

export interface ScopeTargetRef {entryId:string;entryDigest:string}
export interface ProjectTarget {id:string;label:string;kind:Kind;target:string;createdAt:string;inactive?:boolean}
export interface CatalogTarget extends ProjectTarget {methodCatalog:ScopeCatalogEntry[]}
export interface TargetCatalog {schema:'openkoda.project-targets.v1';projectId:string;projectCatalogDigest:string;targets:CatalogTarget[];canChange:boolean;limits:string[];catalogVersion?:string}
export interface ScopeTarget {entryId:string;projectTargetId:string;kind:Kind;target:string;label:string;methods:ScopeMethod[]}
export interface ScopeTargetSet {
 schema:'openkoda.scope-target-set.v1';primaryEntryId:string;entries:ScopeTarget[];commonMethods:ScopeMethod[];
 excludedProjectTargets:{projectTargetId:string;kind:Kind;target:string;label:string;reason:string}[];
 localExclusions:{entryId:string;relativePath:string;match:'exact'|'subtree';reason:string}[];
 connectionExclusions:{address:string;ports?:number[];reason:string}[];
}
export interface CombinedSelection {
 primaryProjectTargetId:string;entries:{projectTargetId:string;methods:ScopeMethod[]}[];commonMethods:ScopeMethod[];
 excludedProjectTargets:{projectTargetId:string;reason:string}[];
 localExclusions:{projectTargetId:string;relativePath:string;match:'exact'|'subtree';reason:string}[];
 connectionExclusions:{address:string;ports?:number[];reason:string}[];
}
export interface TargetOption {targetRef:ScopeTargetRef;entry:ScopeTarget;workGates:{method:ScopeMethod;allowedByScope:boolean;reasonCodes:string[]}[]}
export interface TargetCoverage {targetRef:ScopeTargetRef;label:string;kind:Kind;target:string;methods:{method:ScopeMethod;state:string;applicableExecutor:boolean;recordNotice:string|null;planIds:string[];executionIds:string[]}[];limits:string[]}
export interface ScopeOmission {kind:'local'|'connection';value:string;ruleDigest:string;reason:string}
export interface CombinedPreview {sourceAssessmentId?:string;expectedStateDigest?:string;canCreate?:boolean;assessmentId:string;proposal:{title?:string;body:ScopeBody};reviewDigest:string;canCommit:boolean;reasonCodes:string[]}
export function emptyCombinedSelection():CombinedSelection{return {primaryProjectTargetId:'',entries:[],commonMethods:[],excludedProjectTargets:[],localExclusions:[],connectionExclusions:[]};}
export function selectionFromBody(body?:ScopeBody):CombinedSelection {
 const set=body?.targetSet;if(!set)return emptyCombinedSelection();
 return {primaryProjectTargetId:set.entries.find(e=>e.entryId===set.primaryEntryId)?.projectTargetId??'',entries:set.entries.map(e=>({projectTargetId:e.projectTargetId,methods:[...e.methods]})),commonMethods:[...set.commonMethods],excludedProjectTargets:set.excludedProjectTargets.map(e=>({projectTargetId:e.projectTargetId,reason:e.reason})),localExclusions:set.localExclusions.map(e=>({projectTargetId:set.entries.find(t=>t.entryId===e.entryId)?.projectTargetId??'',relativePath:e.relativePath,match:e.match,reason:e.reason})),connectionExclusions:set.connectionExclusions.map(e=>({...e,ports:e.ports?[...e.ports]:undefined}))};
}
export function sameTargetRef(a?:ScopeTargetRef,b?:ScopeTargetRef){return !!a&&!!b&&a.entryId===b.entryId&&a.entryDigest===b.entryDigest;}
export function currentScopeRef(status?:ScopeStatus):ScopeRef|undefined{return status?.revision?{assessmentId:status.assessmentId,revisionId:status.revision.id,digest:status.revision.digest}:undefined;}
export function targetAllows(option:TargetOption|undefined,method:ScopeMethod){return option?.workGates.some(g=>g.method===method&&g.allowedByScope)===true;}
export function validTargetCatalog(value:unknown,projectId:string):value is TargetCatalog {
 if(!value||typeof value!=='object')return false;const v=value as TargetCatalog;
 return v.schema==='openkoda.project-targets.v1'&&v.projectId===projectId&&typeof v.projectCatalogDigest==='string'&&!!v.projectCatalogDigest&&Array.isArray(v.targets)&&v.targets.length<=64&&new Set(v.targets.map(t=>t.id)).size===v.targets.length&&v.targets.every(t=>typeof t.id==='string'&&typeof t.label==='string'&&typeof t.target==='string'&&['code','files','web','network'].includes(t.kind)&&validScopeCatalog(t.methodCatalog))&&typeof v.canChange==='boolean';
}
export function combinedSelectionValid(s:CombinedSelection,catalog:TargetCatalog){
 const included=new Set(s.entries.map(e=>e.projectTargetId));
 return s.entries.length>0&&s.entries.length<=8&&included.size===s.entries.length&&included.has(s.primaryProjectTargetId)&&s.entries.every(e=>{const t=catalog.targets.find(t=>t.id===e.projectTargetId&&!t.inactive);return !!t&&e.methods.length>0&&new Set(e.methods).size===e.methods.length&&e.methods.every(m=>m!=='kali.runtime-diagnostics.v1'&&t.methodCatalog.some(c=>c.id===m&&c.allowed))&&(!e.methods.includes('source.capture.v1')||e.methods.includes('source.rules.v1'));})&&s.commonMethods.every(m=>m==='kali.runtime-diagnostics.v1')&&s.commonMethods.length<=1&&s.excludedProjectTargets.length<=32&&new Set(s.excludedProjectTargets.map(e=>e.projectTargetId)).size===s.excludedProjectTargets.length&&s.localExclusions.length<=128&&s.connectionExclusions.length<=32&&s.excludedProjectTargets.every(e=>!included.has(e.projectTargetId)&&catalog.targets.some(t=>t.id===e.projectTargetId)&&!!e.reason.trim())&&s.localExclusions.every(e=>included.has(e.projectTargetId)&&catalog.targets.some(t=>t.id===e.projectTargetId&&['code','files'].includes(t.kind))&&s.localExclusions.filter(r=>r.projectTargetId===e.projectTargetId).length<=32&&!!e.relativePath.trim()&&!!e.reason.trim())&&s.connectionExclusions.every(e=>!!e.address.trim()&&!!e.reason.trim()&&(!e.ports||e.ports.length>0&&e.ports.length<=16&&new Set(e.ports).size===e.ports.length&&e.ports.every(p=>Number.isInteger(p)&&p>=1&&p<=65535)));
}
