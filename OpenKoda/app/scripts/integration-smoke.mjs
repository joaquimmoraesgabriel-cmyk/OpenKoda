// Real native integration with a generated, explicitly selected lab fixture.
// --ai permits one minimal provider request. --investigation permits exactly
// two independent reviews of the synthetic fixture. --proposal permits one
// constrained operation suggestion. --source captures only a selected synthetic
// source fixture locally; sending it still requires an explicit AI flag.
import {chromium,expect} from '@playwright/test';
import {spawn} from 'node:child_process';
import {createServer} from 'node:net';
import {mkdir,writeFile} from 'node:fs/promises';
import {resolve,dirname,join} from 'node:path';
import {fileURLToPath} from 'node:url';
import {randomUUID,createHash} from 'node:crypto';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'..');
const directory=join(root,'.local',`integration-smoke-${randomUUID()}`);
const fixture=join(directory,'fixture');await mkdir(fixture,{recursive:true});
await writeFile(join(fixture,'lab.txt'),'OpenKoda integration fixture. No secrets or user project content.\n');
const net=createServer();await new Promise((ok,fail)=>{net.once('error',fail);net.listen(0,'127.0.0.1',ok);});
const port=net.address().port;await new Promise(ok=>net.close(ok));
const output={passed:false,records:[],directory};let child,browser,page,project,assessment;
async function gate(id,fn){const record={id,passed:false};output.records.push(record);record.detail=await fn();record.passed=true;console.log(JSON.stringify({gate:id,passed:true}));}
const hash=text=>createHash('sha256').update(text).digest('hex');
try {
 const env={...process.env,WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`};delete env.OPENKODA_DATA_DIR;
 child=spawn(join(root,'src-tauri/target/debug/openkoda.exe'),[],{cwd:root,windowsHide:true,stdio:'ignore',env});child.on('error',()=>{});
 await expect.poll(async()=>{try{return(await fetch(`http://127.0.0.1:${port}/json/version`,{signal:AbortSignal.timeout(1000)})).ok;}catch{return false;}},{timeout:45000}).toBe(true);
 browser=await chromium.connectOverCDP(`http://127.0.0.1:${port}`);const context=browser.contexts()[0];
 await expect.poll(()=>context.pages().length,{timeout:15000}).toBeGreaterThan(0);page=context.pages()[0];
 await page.waitForFunction(()=>typeof window.__TAURI_INTERNALS__?.invoke==='function');
 await expect(page.locator('main')).toHaveAttribute('aria-busy','false',{timeout:60000});
 const request=(action,payload={})=>page.evaluate(({action,payload})=>window.__TAURI_INTERNALS__.invoke('request',{request:{action,payload}}),{action,payload});
 await gate('E2-real-managed-environment',async()=>{const environment=await request('environment.diagnose');expect(environment.status).toBe('ready');expect(environment.details.uid).toBeGreaterThan(0);expect(environment.details.interopEnabled).toBe(false);expect(environment.details.windowsMounts).toEqual([]);return environment;});
 let inventory,sourceCapture;
 await gate('E3-native-lab-observation',async()=>{
  project=await request('project.create',{name:'Verificação de integração OpenKoda',kind:'code',target:fixture,description:'Fixture sintética criada pelo teste; arquivada após verificação.'});
  const scopeOptions=await request('assessment.scopeOptions',{projectId:project.id});
  const methods=['local.inventory.v1','kali.runtime-diagnostics.v1','kali.hash-evidence.v1'];
  if(process.argv.includes('--scan'))methods.push('kali.rg.signals.v1');
  if(process.argv.includes('--source')||process.argv.includes('--proposal'))methods.push('source.rules.v1');
  if(process.argv.includes('--source'))methods.push('source.capture.v1');
  if(['--ai','--investigation','--proposal'].some(flag=>process.argv.includes(flag)))methods.push('ai.remote-review.v1');
  assessment=await request('assessment.create',{projectId:project.id,title:'Integrações reais em laboratório',kind:'code',target:fixture,objective:'Verificar diagnóstico e integridade de bytes sintéticos. Não realizar exploração ou interpretar isto como auditoria de segurança.',
   scope:{methods,window:{startsAt:null,endsAt:null},projectDigest:scopeOptions.projectDigest,catalogVersion:scopeOptions.catalogVersion,acknowledgeScope:true}});
  output.scope=await request('assessment.scopeStatus',{assessmentId:assessment.id});
  expect(output.scope.phase).toBe('draft');expect(output.scope.revision.body.methods.slice().sort()).toEqual(methods.slice().sort());
  const plan=await request('assessment.plan',{id:assessment.id});expect(plan.inputFiles).toHaveLength(1);
  await request('plan.approve',{id:plan.id,digest:plan.digest});await request('plan.run',{id:plan.id,digest:plan.digest});
  const state=await request('snapshot');assessment=state.assessments.find(a=>a.id===assessment.id);expect(assessment.status).toBe('partial');inventory=assessment.evidence[0];expect(inventory.sha256).toBe(hash(inventory.content));return {assessmentId:assessment.id,evidenceId:inventory.id};
 });
 await gate('E3-real-kali-mcp',async()=>{
  const diagnostic=await request('kali.mcp.diagnose',{assessmentId:assessment.id});expect(diagnostic.run.status).toBe('completed');expect(diagnostic.cleanupConfirmed).toBe(true);expect(diagnostic.result.uid).toBeGreaterThan(0);
  const hashed=await request('kali.mcp.hashEvidence',{assessmentId:assessment.id,evidenceId:inventory.id});expect(hashed.run.status).toBe('completed');expect(hashed.cleanupConfirmed).toBe(true);expect(hashed.result.sha256).toBe(hash(inventory.content));
  const status=await request('kali.mcp.status');expect(status.hasUnresolved).toBe(false);return {diagnostic,hashed};
 });
 if(process.argv.includes('--scan'))await gate('E3-real-kali-ripgrep',async()=>{
  const input=await request('evidence.import',{assessmentId:assessment.id,name:'Synthetic parser input',sourceDescription:'Synthetic fixture made only to verify the fixed offline parser.',content:"const api_key = 'SYNTHETIC-PARSER-SENTINEL';\neval(data);\n-----BEGIN PRIVATE KEY-----\n",acknowledgeUnverifiedSource:true});
  const state=await request('snapshot');const a=state.assessments.find(item=>item.id===assessment.id);const evidence=a.evidence.find(item=>item.id===input.id);
  expect(evidence).toBeTruthy();
  let refused=false;try{await request('kali.mcp.scanEvidence',{assessmentId:assessment.id,evidenceId:evidence.id,acknowledgeLocalTransfer:false});}catch{refused=true;}expect(refused).toBe(true);
  const before=await request('kali.mcp.status');
  const parsed=await request('kali.mcp.scanEvidence',{assessmentId:assessment.id,evidenceId:evidence.id,acknowledgeLocalTransfer:true});
  expect(parsed.run.status).toBe('completed');expect(parsed.cleanupConfirmed).toBe(true);expect(parsed.result.termination).toBe('completed');
  expect(parsed.result.adapter).toBe('kali.rg.signals.v1');expect(parsed.result.inputSha256).toBe(hash(evidence.content));expect(parsed.run.sourceEvidenceId).toBe(evidence.id);
  expect(parsed.result.candidates.length).toBeGreaterThan(0);expect(parsed.result.tool.name).toBe('ripgrep');expect(parsed.result.tool.binarySha256).toMatch(/^[a-f0-9]{64}$/);
  expect(JSON.stringify(parsed)).not.toContain('SYNTHETIC-PARSER-SENTINEL');
  const after=await request('kali.mcp.status');expect(after.hasUnresolved).toBe(false);expect(after.recent.length).toBe(before.recent.length+1);
  expect((await request('snapshot')).findings.filter(item=>item.assessmentId===assessment.id)).toHaveLength(0);
  return parsed;
 });
 if(process.argv.includes('--source'))await gate('E4-reviewed-synthetic-source',async()=>{
  await writeFile(join(fixture,'omitted.txt'),'UNSELECTED-SOURCE-SENTINEL');
  const plan=await request('assessment.sourcePlan',{id:assessment.id});
  const selection={assessmentId:assessment.id,planId:plan.id,paths:['lab.txt']};
  const preview=await request('evidence.sourcePreview',selection);
  expect(preview.paths).toEqual(['lab.txt']);expect(preview.sourceFilesIncluded).toBe(true);expect(preview.sentToProvider).toBe(false);
  expect(JSON.stringify(preview)).not.toContain('UNSELECTED-SOURCE-SENTINEL');
  sourceCapture=await request('evidence.sourceCapture',{...selection,digest:preview.digest,acknowledgeLocalCapture:true});
  expect(sourceCapture.sha256).toBe(hash(sourceCapture.content));
  const repeat=await request('evidence.sourceCapture',{...selection,digest:preview.digest,acknowledgeLocalCapture:true});expect(repeat.id).toBe(sourceCapture.id);
  await request('plan.revoke',{id:plan.id,digest:plan.digest});
  const context=await request('ai.context',{assessmentId:assessment.id,evidenceIds:[sourceCapture.id]});
  expect(context.sourceFilesIncluded).toBe(true);expect(JSON.stringify(context)).not.toContain('UNSELECTED-SOURCE-SENTINEL');
  return {planId:plan.id,evidenceId:sourceCapture.id,evidenceSha256:sourceCapture.sha256,selectedPaths:preview.paths,sourceBytes:preview.totalSourceBytes,providerCalled:false};
 });
 const selectedAiEvidence=()=>[sourceCapture?.id??inventory.id];
 if(process.argv.includes('--ai'))await gate('E4-real-consented-lab-context',async()=>{
  const status=await request('ai.status');expect(status.verified).toBe(true);
  const preview=await request('ai.context',{assessmentId:assessment.id,evidenceIds:selectedAiEvidence()});
  await writeFile(join(directory,'reviewed-synthetic-context.json'),JSON.stringify(preview,null,2));
  expect(preview.bytes).toBeLessThan(16000);
  const reply=await request('ai.ask',{assessmentId:assessment.id,evidenceIds:selectedAiEvidence(),contextDigest:preview.digest,message:'Este é somente um teste de integração com uma fixture sintética. Responda em uma frase curta confirmando que inventário, hashes e leitura de um arquivo não comprovam ausência de vulnerabilidades. Não sugira operações ou ferramentas.',allowRemote:true});
  expect(reply.text.length).toBeGreaterThan(0);
  const history=await request('ai.history',{assessmentId:assessment.id});expect(history.messages).toHaveLength(2);
  return {model:status.model,contextSha256:preview.digest,contextBytes:preview.bytes,usage:reply.usage,historyMessages:history.messages.length};
 });
 if(process.argv.includes('--investigation'))await gate('E4-real-independent-investigators',async()=>{
  const status=await request('ai.status');expect(status.verified).toBe(true);
  const preview=await request('ai.context',{assessmentId:assessment.id,evidenceIds:selectedAiEvidence()});
  expect(preview.bytes).toBeLessThan(16000);
  const plan=await request('ai.investigation.plan',{assessmentId:assessment.id,evidenceIds:selectedAiEvidence(),contextDigest:preview.digest,
   message:'Teste de integração com fixture sintética, sem alvo externo. Em no máximo duas frases, explique o limite de concluir segurança a partir de inventário e hashes. Cite o ID da evidência. Não proponha execução ou vulnerabilidade.',
   perspectives:['evidence','counterevidence'],maxOutputTokensPerRequest:2048,maxTotalTokens:20000});
  expect(plan.status).toBe('prepared');expect(plan.budget.maxRequests).toBe(2);expect(plan.contextDigest).toBe(preview.digest);expect(plan.model).toBe(status.model);
  await writeFile(join(directory,'reviewed-synthetic-investigation.json'),JSON.stringify(plan,null,2));
  const job=await request('ai.investigation.run',{id:plan.id,digest:plan.digest,allowRemote:true});
  expect(job.status).toBe('completed');expect(job.results).toHaveLength(2);expect(job.results.every(result=>result.status==='completed'&&result.text.length>0)).toBe(true);
  expect(job.results.map(result=>result.perspective)).toEqual(['evidence','counterevidence']);expect(job.toolsExecuted).toBe(false);expect(job.usageUnknown).toBe(false);
  expect((await request('ai.investigation.list',{assessmentId:assessment.id})).items.find(item=>item.id===plan.id).status).toBe('completed');
  return {id:job.id,model:job.model,contextDigest:job.contextDigest,requests:job.results.map(result=>({id:result.requestId,perspective:result.perspective,usage:result.usage,status:result.status})),knownTokens:job.knownTokens,toolsExecuted:false};
 });
 if(process.argv.includes('--proposal'))await gate('E4-real-constrained-operation-proposal',async()=>{
  const status=await request('ai.status');expect(status.verified).toBe(true);
  const preview=await request('ai.context',{assessmentId:assessment.id,evidenceIds:selectedAiEvidence()});
  expect(preview.bytes).toBeLessThan(16000);
  const before=await request('snapshot');
  const proposal=await request('ai.operation.propose',{assessmentId:assessment.id,evidenceIds:selectedAiEvidence(),contextDigest:preview.digest,
   message:'Teste de integração com fixture sintética. Sugira a única operação permitida para verificar sinais lexicais locais. Use justificativa de uma frase, sem comandos nem parâmetros.',
   allowedAdapters:['source.rules.v1'],allowRemote:true});
  expect(proposal.adapter).toBe('source.rules.v1');expect(proposal.assessmentId).toBe(assessment.id);expect(proposal.contextDigest).toBe(preview.digest);
  expect(proposal.toolsExecuted).toBe(false);expect(proposal.requiresHumanReview).toBe(true);expect(proposal.availabilityVerified).toBe(false);
  expect(proposal.rationale.length).toBeGreaterThan(0);
  const after=await request('snapshot');
  expect(after.assessments).toEqual(before.assessments);expect(after.findings).toEqual(before.findings);
  const history=await request('ai.history',{assessmentId:assessment.id});
  const item=history.requests.find(item=>item.id===proposal.requestId);
  expect(item.operationProposal.status).toBe('accepted');expect(item.operationProposal.proposal.id).toBe(proposal.id);
  return {id:proposal.id,requestId:proposal.requestId,adapter:proposal.adapter,model:proposal.model,contextDigest:proposal.contextDigest,usage:proposal.usage,toolsExecuted:false,domainUnchanged:true};
 });
 await gate('E5-integrated-report',async()=>{const report=await request('report.bundle',{id:assessment.id,includeEvidence:true});expect(report.partial).toBe(true);return report;});
 await request('project.archive',{id:project.id});
 output.archivedProjectId=project.id;output.assessmentId=assessment.id;
 await page.reload();await expect(page.locator('main')).toHaveAttribute('aria-busy','false',{timeout:60000});
 await page.screenshot({path:join(directory,'integration-complete.png'),fullPage:true});output.passed=true;
}catch(error){output.error=String(error);process.exitCode=1;}
finally{await browser?.close().catch(()=>{});if(child&&child.exitCode===null)child.kill();await writeFile(join(directory,'result.json'),JSON.stringify(output,null,2));console.log(JSON.stringify({passed:output.passed,error:output.error,directory},null,2));}
