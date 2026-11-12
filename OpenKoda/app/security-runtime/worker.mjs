import { dirname, delimiter } from 'node:path';
import { inspectPlatform } from './platform.mjs';
import { validateRequest, preparePaths, sdkConfig, costSummary, resultSummary, failureSummary } from './scan.mjs';

const controller=new AbortController();
let started=false;
function send(event){
  if (process.connected)process.send(event,error=>{if(error)controller.abort();});
}
process.on('disconnect',()=>controller.abort());
const phases={
  preflight:'Verificando os requisitos da análise.',
  threat_model:'Identificando riscos e limites do projeto.',
  discovery:'Analisando o código selecionado.',
  validation:'Verificando os achados candidatos.',
  attack_path:'Verificando os caminhos de exploração.',
  reporting:'Preparando o relatório.',
};

async function execute(input){
  let client,latestCost=null,scanId=null;
  try{
    const checkOnly=input.operation==='check';
    if(!checkOnly)validateRequest(input);
    const {check,python,git,codex}=await inspectPlatform();
    if(checkOnly)return check;
    if(!check.available)return {type:'error',code:'runtime_unavailable',message:'Componentes indisponíveis: '+check.missing.join(', ')+'.'};
    controller.signal.throwIfAborted();
    const paths=await preparePaths(input);
    process.chdir(paths.runDir);
    process.env.CODEX_HOME=paths.configDir;
    process.env.CODEX_SECURITY_STATE_DIR=paths.stateDir;
    process.env.CODEX_CLI_PATH=codex.path;
    process.env.PATH=[dirname(git.path),dirname(python.path),process.env.PATH].join(delimiter);
    process.env[input.provider==='openrouter'?'OPENROUTER_API_KEY':'OPENAI_API_KEY']=input.apiKey;
    const {CodexSecurity}=await import('@openai/codex-security');
    client=new CodexSecurity(sdkConfig(input,python.path));
    let lastPhase='';
    let lastProgress=0;
    const result=await client.run(paths.repository,{
      target:input.target,mode:'standard',auth:'api-key',scanPrompt:input.prompt || undefined,
      maxCostUsd:input.maxCostUsd,maxTimeMinutes:15,outputDir:paths.artifacts,
      skipFindingMatching:true,signal:controller.signal,
      onLifecycle:event=>{if(event.mock===true)controller.abort(); else if(typeof event.scanId==='string' && event.scanId.length<=192)scanId=event.scanId;},
      onCost:event=>{latestCost=costSummary(event);},
      onProgress:event=>{
        if(!Object.hasOwn(phases,event.phase))return;
        const now=Date.now();
        if(lastPhase===event.phase && now-lastProgress<15000)return;
        lastPhase=event.phase;lastProgress=now;
        send({type:'progress',stage:event.phase,message:phases[event.phase]});
      },
    });
    controller.signal.throwIfAborted();
    return await resultSummary(result,paths,input.apiKey);
  }catch(error){
    if(controller.signal.aborted)return {type:'result',status:'cancelled',scanId,report:'',findings:[],coverage:{scope:input.target??[],excludedPaths:[],additionalExclusions:0,findingsTotal:0,reportTruncated:false},cost:latestCost};
    return failureSummary(error);
  }finally{
    try{await client?.close();}finally{delete process.env.OPENAI_API_KEY;delete process.env.OPENROUTER_API_KEY;}
  }
}
if(!process.send)process.exitCode=2;
else process.on('message',input=>{
  if(input?.operation==='cancel'){controller.abort();return;}
  if(started)return;
  started=true;
  execute(input).catch(failureSummary).then(event=>{
    if(process.connected)process.send(event,()=>process.disconnect());
  });
});
