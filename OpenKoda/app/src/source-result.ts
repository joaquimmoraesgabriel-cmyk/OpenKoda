export interface SourceCandidate {id:string;ruleId:string;ruleVersion:string;status:'candidate';title:string;observation:string;path:string;line:number;fileSha256:string}
const countKeys=['plannedFiles','plannedBytes','verifiedFiles','scannedFiles','binaryFiles','tooLargeFiles','partialFiles','readErrors','excludedDirectories','unvisitedFiles','candidates','omittedCandidates','omittedFileResults'] as const;
type CountKey=typeof countKeys[number];
export interface SourceResult {adapter:'source.rules.v1';rulesVersion:string;manifestSha256:string;termination:'completed'|'canceled'|'timed_out'|'read_error';canceled:boolean;timedOut:boolean;partial:boolean;counts:Record<CountKey,number>;candidates:SourceCandidate[];fileResults:unknown[];limits:string[]}
function object(value:unknown):value is Record<string,unknown>{return !!value&&typeof value==='object'&&!Array.isArray(value);}
function integer(value:unknown){return typeof value==='number'&&Number.isSafeInteger(value)&&value>=0;}
export function parseSourceResult(content:string):SourceResult|null{
  try{return readSourceResult(JSON.parse(content));}catch{return null;}
}
export function readSourceResult(value:unknown):SourceResult|null{
    if(!object(value)||value.adapter!=='source.rules.v1'||typeof value.rulesVersion!=='string'||typeof value.manifestSha256!=='string'||!['completed','canceled','timed_out','read_error'].includes(String(value.termination)))return null;
    if(!['canceled','timedOut','partial'].every(key=>typeof value[key]==='boolean')||!object(value.counts))return null;
    const counts=value.counts;
    if(!countKeys.every(key=>integer(counts[key]))||!Array.isArray(value.candidates)||!Array.isArray(value.fileResults)||!Array.isArray(value.limits)||!value.limits.every(limit=>typeof limit==='string'))return null;
    if(!value.candidates.every(candidate=>object(candidate)&&candidate.status==='candidate'&&['id','ruleId','ruleVersion','title','observation','path','fileSha256'].every(key=>typeof candidate[key]==='string')&&integer(candidate.line)&&Number(candidate.line)>0))return null;
    return value as unknown as SourceResult;
}
