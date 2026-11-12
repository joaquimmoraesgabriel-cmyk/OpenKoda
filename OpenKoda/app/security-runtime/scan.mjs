import { lstat, mkdir, open, readdir, realpath } from 'node:fs/promises';
import { dirname, isAbsolute, join, relative, resolve, sep } from 'node:path';

export class ScanInputError extends Error {}
const invalid=()=>{throw new ScanInputError('invalid_request');};
const bounded=(s,max)=>typeof s==='string' && s.length>0 && Buffer.byteLength(s)<=max && !s.includes('\0');
const targetPart=/^(?![. ]+$)[^<>:"|?*\\\x00-\x1f]+$/;

export function validateRequest(request) {
  const fields=['operation','repository','outputDir','provider','model','apiKey','prompt','maxCostUsd','target'];
  if (!request || typeof request!=='object' || Array.isArray(request) || Object.keys(request).some(k=>!fields.includes(k))) invalid();
  if (request.operation!=='scan' || !['openai','openrouter'].includes(request.provider)) invalid();
  if (![request.repository,request.outputDir].every(s=>bounded(s,4096) && isAbsolute(s))) invalid();
  if (!bounded(request.model,192) || !/^[a-zA-Z0-9][a-zA-Z0-9._:/-]*$/.test(request.model)) invalid();
  if (!bounded(request.apiKey,4096) || !/^[\x21-\x7e]+$/.test(request.apiKey)) invalid();
  if (typeof request.prompt!=='string' || Buffer.byteLength(request.prompt)>8192 || request.prompt.includes('\0')) invalid();
  if (!Number.isFinite(request.maxCostUsd) || request.maxCostUsd<=0 || request.maxCostUsd>10000) invalid();
  if (request.target!==undefined){
    if (!Array.isArray(request.target) || !request.target.length || request.target.length>64 || new Set(request.target).size!==request.target.length) invalid();
    for (const item of request.target){
      if (!bounded(item,512) || item.split('/').some(p=>!targetPart.test(p) || /[ .]$/.test(p))) invalid();
    }
  }
  return request;
}

function inside(parent,path) {
  const rel=relative(parent,path);
  return rel==='' || !isAbsolute(rel) && rel!=='..' && !rel.startsWith('..'+sep);
}

export async function preparePaths(req) {
  const repository=await realpath(req.repository);
  if (!(await lstat(repository)).isDirectory()) invalid();
  for (const item of req.target??[]){
    const selected=await realpath(join(repository,item));
    if (!inside(repository,selected)) invalid();
  }
  const requested=resolve(req.outputDir);
  const physical=join(await realpath(dirname(requested)),relative(dirname(requested),requested));
  if (inside(repository,physical)||inside(physical,repository))invalid();
  try { await mkdir(physical,{mode:0o700}); }
  catch(error){ if(error.code!=='EEXIST')throw error; }
  const stat=await lstat(physical);
  if (!stat.isDirectory() || stat.isSymbolicLink() || (await readdir(physical)).length) throw new ScanInputError('output_not_empty');
  const runDir=await realpath(physical);
  const configDir=join(runDir,'config');
  await mkdir(configDir,{mode:0o700});
  return {repository,runDir,configDir,stateDir:join(runDir,'state'),artifacts:join(runDir,'artifacts')};
}

export function sdkConfig(req,pythonPath) {
  const codexOverrides={model:req.model,model_provider:req.provider};
  if(req.provider==='openrouter'){
    codexOverrides.model_providers={openrouter:{name:'OpenRouter',base_url:'https://openrouter.ai/api/v1',wire_api:'responses',env_key:'OPENROUTER_API_KEY'}};
  }
  return {pythonPath,codexOverrides};
}

const reportLimit=256*1024;
function redact(text,key){
  return text.split(key).join('[redacted]').replace(/\bsk-(?:proj-|or-v1-)?[A-Za-z0-9_-]{16,}/g,'[redacted]').replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/g,' ');
}
function utf8Prefix(value,bytes){
  const buf=Buffer.from(value);
  return buf.length<=bytes?value:new TextDecoder().decode(buf.subarray(0,bytes),{stream:true});
}

async function readReport(path,dir,key) {
  if(resolve(path)!==join(dir,'report.md') || (await realpath(path))!==join(await realpath(dir),'report.md')) throw new Error('invalid_artifacts');
  const link=await lstat(path);
  if (!link.isFile() || link.isSymbolicLink() || link.nlink!==1) throw new Error('invalid_artifacts');
  const file=await open(path,'r');
  try{
    const stat=await file.stat();
    if (!stat.isFile() || stat.ino!==link.ino || stat.dev!==link.dev) throw new Error('invalid_artifacts');
    const buf=Buffer.alloc(Math.min(stat.size,reportLimit+4096));
    let n=0;
    while(n<buf.length){
      const {bytesRead}=await file.read(buf,n,buf.length-n,n);
      if (!bytesRead)break;
      n+=bytesRead;
    }
    const safe=redact(new TextDecoder().decode(buf.subarray(0,n),{stream:n<stat.size}),key);
    return {report:utf8Prefix(safe,reportLimit),truncated:n<stat.size || Buffer.byteLength(safe)>reportLimit};
  }finally{await file.close();}
}

export function costSummary(cost) {
  return Number.isFinite(cost?.estimatedUsd) && cost.estimatedUsd>=0?{estimatedUsd:cost.estimatedUsd}:null;
}
export async function resultSummary(result,paths,key) {
  const scan=result.manifest?.scan;
  const raw=result.findings?.findings;
  if (scan?.status!=='completed' || !bounded(scan.id,192) || result.findings?.scanId!==scan.id || !Array.isArray(raw)) throw new Error('invalid_artifacts');
  const {report,truncated}=await readReport(result.reportPath,paths.artifacts,key);
  const safe=(value,bytes)=>utf8Prefix(redact(typeof value==='string'?value:'',key),bytes);
  const findings=raw.slice(0,100).map(f=>{
    const at=f.locations?.[0];
    return {id:bounded(f.findingId,192)?safe(f.findingId,192):null,title:safe(f.title,384),severity:safe(f.severity?.level,24),description:safe(f.summary,2048),location:at?safe(`${at.path}:${at.startLine}`,560):null};
  });
  const included=scan.scope?.includePaths??[];
  const excluded=[...new Set([...(scan.scope?.excludePaths??[]),...(result.review?.excludedPaths??[])])];
  const coverage={scope:included.slice(0,64).map(s=>safe(s,512)),excludedPaths:excluded.slice(0,128).map(s=>safe(s,512)),additionalExclusions:Math.max(0,excluded.length-128),findingsTotal:raw.length,reportTruncated:truncated,
    summary:safe(scan.scope?.summary,4096),limitations:Array.isArray(scan.scope?.limitations)?scan.scope.limitations.slice(0,128).map(s=>safe(s,2048)):[]};
  return {type:'result',status:'completed',scanId:safe(scan.id,192),report,findings,coverage,cost:costSummary(result.cost)};
}

// Only known SDK status codes and constant text cross the host boundary.
// Raw SDK errors can contain provider bodies, argv, environment or source.
export function failureSummary(err) {
  const words=[err,err?.cause,err?.cause?.cause].map(e=>String(e?.name||'')+' '+String(e?.message||'')).join(' ');
  const reasons=[
    [/invalid_request/,'invalid_request','Solicitação de análise inválida.'],
    [/output_not_empty/,'output_not_empty','Selecione uma nova pasta de resultados.'],
    [/invalid_artifacts/,'invalid_artifacts','O SDK retornou artefatos incompletos ou inconsistentes.'],
    [/pricing|no price|unpriced|verifiable usage|cost.*(?:unavailable|not available)/i,'pricing_unavailable','O SDK não conseguiu verificar o custo deste modelo.'],
    [/ScanCostLimitExceededError|cost limit|budget exceeded/i,'cost_limit','O SDK atingiu o limite de custo estimado.'],
    [/\b429\b|rate.limit|quota.exceeded/i,'provider_quota','O provedor informou limite de uso ou de requisições.'],
    [/\b401\b|invalid.api.key|authenticationrequired|authentication failed/i,'authentication','O provedor recusou a credencial.'],
    [/\b403\b|forbidden|entitlement|trusted access/i,'provider_access','O provedor recusou acesso ao modelo ou à análise.'],
    [/sandbox/i,'sandbox','O SDK não conseguiu preparar ou executar o isolamento local.'],
    [/model.*not found|unsupported model|model_not_found/i,'model_unavailable','O modelo selecionado está indisponível.'],
    [/PluginPythonUnavailable|python interpreter/i,'python_unavailable','O SDK não conseguiu usar o Python configurado.'],
    [/PluginBootstrap|bootstrap plugin|plugin installation/i,'plugin_bootstrap','Falha ao preparar o plugin oficial do SDK.'],
    [/OutputDirectory|OutputInsideProtectedRoot|protected root/i,'output_unavailable','O SDK recusou o destino dos artefatos.'],
    [/IncompleteScan|ScanInterrupted/i,'incomplete_scan','A execução terminou antes de produzir um relatório completo.'],
    [/ENOTFOUND|ECONNRESET|network error|connection refused|timed out/i,'connection','Falha de conexão durante a execução do SDK.'],
    [/ENOENT|EACCES|EPERM/i,'local_access','O motor não conseguiu acessar um recurso local necessário.'],
  ];
  const [,code,message]=reasons.find(([pattern])=>pattern.test(words))??[null,'sdk_failure','O SDK encerrou a análise com erro.'];
  return {type:'error',code,message};
}
