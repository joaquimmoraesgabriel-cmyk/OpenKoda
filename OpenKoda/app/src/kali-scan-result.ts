export const kaliRuleLabels={'private-key-marker':'Marcador de chave privada','dynamic-evaluation':'Avaliação dinâmica textual','credential-identifier':'Identificador de credencial'};
type RuleId=keyof typeof kaliRuleLabels;
export interface KaliScanResult {
  operation:'scan_bytes';adapter:'kali.rg.signals.v1';rulesVersion:string;inputSha256:string;bytes:number;
  environmentId:string;sessionId:string;assessmentId:string;tool:{name:'ripgrep';version:string;binarySha256:string};
  candidates:{ruleId:RuleId;line:number;label:string;status:'candidate'}[];counts:Record<RuleId,number>;
  observedSignals:number;omittedCandidates:number;countsComplete:boolean;partial:boolean;
  termination:'completed'|'canceled'|'timed_out'|'output_limited'|'parser_error';effects:string;limitations:string;
}
function object(value:unknown):value is Record<string,unknown>{return !!value&&typeof value==='object'&&!Array.isArray(value);}
function count(value:unknown){return typeof value==='number'&&Number.isSafeInteger(value)&&value>=0;}
export function readKaliScanResult(value:unknown):KaliScanResult|null{
  if(!object(value)||value.operation!=='scan_bytes'||value.adapter!=='kali.rg.signals.v1'||!['rulesVersion','inputSha256','environmentId','sessionId','assessmentId','effects','limitations'].every(key=>typeof value[key]==='string'))return null;
  if(!object(value.tool)||value.tool.name!=='ripgrep'||typeof value.tool.version!=='string'||typeof value.tool.binarySha256!=='string')return null;
  if(!count(value.bytes)||!count(value.observedSignals)||!count(value.omittedCandidates)||typeof value.countsComplete!=='boolean'||typeof value.partial!=='boolean'||!['completed','canceled','timed_out','output_limited','parser_error'].includes(String(value.termination)))return null;
  if(!object(value.counts))return null;const counts=value.counts;if(!Object.keys(kaliRuleLabels).every(key=>count(counts[key])))return null;
  if(!Array.isArray(value.candidates)||value.candidates.length>16||!value.candidates.every(candidate=>object(candidate)&&typeof candidate.ruleId==='string'&&Object.hasOwn(kaliRuleLabels,candidate.ruleId)&&typeof candidate.label==='string'&&candidate.status==='candidate'&&count(candidate.line)&&Number(candidate.line)>0))return null;
  return value as unknown as KaliScanResult;
}
