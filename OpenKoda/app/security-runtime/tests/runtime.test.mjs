import test from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { mkdtemp, mkdir, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { homedir, tmpdir } from 'node:os';
import { basename, dirname, join } from 'node:path';
import { root, sdkVersion } from '../platform.mjs';
import { failureSummary, preparePaths, resultSummary, sdkConfig, validateRequest } from '../scan.mjs';

const key='test-credential-that-must-not-appear';
async function fixture(){
  const dir=await mkdtemp(join(tmpdir(),'openkoda-security-test-'));
  const repository=join(dir,'repository'),outputDir=join(dir,'output');
  await mkdir(join(repository,'src'),{recursive:true});await writeFile(join(repository,'src','demo.js'),'export const a = 1;');
  return {operation:'scan',repository,outputDir,target:['src'],provider:'openai',apiKey:key,model:'gpt-6.1-sol',prompt:'Review the fixture.',maxCostUsd:1};
}
function cli(args,lines){
  return new Promise((resolve,reject)=>{
    const child=spawn(process.execPath,[join(root,'cli.mjs'),...args],{cwd:root,windowsHide:true,stdio:'pipe'});
    let stdout='',stderr='';
    const timeout=setTimeout(()=>{child.kill();reject(new Error('runtime test timeout'));},90000);
    child.once('error',error=>{clearTimeout(timeout);reject(error);});
    child.stdout.on('data',s=>{stdout+=s;});
    child.stderr.on('data',s=>{stderr+=s;});
    child.once('close',code=>{clearTimeout(timeout);try{resolve({code,stderr,stdout,frames:stdout.trim().split('\n').map(s=>JSON.parse(s))});}catch(e){reject(e);}});
    for(const line of lines)child.stdin.write(line+'\n');
    if(args.length)child.stdin.end();
  });
}

test('only scoped paths and supported options reach a provider',async()=>{
  const good=await fixture();
  assert.equal(validateRequest(good),good);
  for(const overrides of [{provider:'custom'},{target:['../outside']},{target:['src:*']},{target:[]},{target:['src//private']},{target:['.']},{target:['src/*']},{apiKey:'a\nb'},{maxCostUsd:0},{maxCostUsd:Infinity},{mock:true},{codexOverrides:{}}]){
    assert.throws(()=>validateRequest({...good,...overrides}));
  }
  const config=sdkConfig({...good,provider:'openrouter'},'python');
  assert.equal(config.codexOverrides.model_providers.openrouter.env_key,'OPENROUTER_API_KEY');
  assert.equal(config.codexOverrides.model_providers.openrouter.base_url,'https://openrouter.ai/api/v1');
  assert.ok(!JSON.stringify(config).includes(key));
});
test('rejects existing nonempty output or output overlapping the repository',async()=>{
  const req=await fixture();
  await assert.rejects(preparePaths({...req,outputDir:join(req.repository,'results')}));
  await mkdir(req.outputDir);
  await writeFile(join(req.outputDir,'keep'),'previous output');
  await assert.rejects(preparePaths(req));
  assert.equal(await readFile(join(req.outputDir,'keep'),'utf8'),'previous output');
});
test('bounded projection validates the official scan ID, redacts the credential and keeps counts',async()=>{
  const req=await fixture(),paths=await preparePaths(req);
  await mkdir(paths.artifacts);const reportPath=join(paths.artifacts,'report.md');
  await writeFile(reportPath,`# ${key}\n`+'Área de teste '.repeat(34000));
  const finding={title:'Synthetic '+key,severity:{level:'low'},summary:'Example.',locations:[{path:'src/demo.js',startLine:1}]};
  const result={manifest:{scan:{id:'fixture-id',status:'completed',scope:{includePaths:['src'],excludePaths:['vendor'],summary:'Partial source review '+key,limitations:['Unreviewed code '+key,'x'.repeat(9000)]}}},review:{excludedPaths:['vendor','dist']},findings:{scanId:'fixture-id',findings:Array(102).fill(finding)},cost:{estimatedUsd:0.25},reportPath};
  const frame=await resultSummary(result,paths,key);
  assert.equal(frame.coverage.findingsTotal,102);assert.equal(frame.findings.length,100);
  assert.deepEqual(frame.coverage.excludedPaths,['vendor','dist']);
  assert.equal(frame.coverage.summary,'Partial source review [redacted]');
  assert.equal(frame.coverage.limitations[0],'Unreviewed code [redacted]');
  assert.ok(Buffer.byteLength(frame.coverage.limitations[1])<=2048);
  assert.equal(frame.coverage.reportTruncated,true);assert.ok(Buffer.byteLength(frame.report)<=256*1024);
  assert.ok(frame.report.includes('[redacted]'));assert.ok(!JSON.stringify(frame).includes(key));
  assert.equal(frame.findings[0].location,'src/demo.js:1');
  assert.deepEqual(frame.cost,{estimatedUsd:0.25});
  await assert.rejects(resultSummary({...result,findings:{scanId:'other',findings:[]}},paths,key));
  await assert.rejects(resultSummary({...result,reportPath:join(paths.runDir,'config','secret')},paths,key));
});
test('maps provider failures without returning provider bodies',()=>{
  const failure=failureSummary(new Error('401 invalid api key '+key));
  assert.equal(failure.code,'authentication');assert.ok(!JSON.stringify(failure).includes(key));
  assert.equal(failureSummary(new Error('A scan cost limit is not available for the configured model')).code,'pricing_unavailable');
});
test('official SDK accepts a bounded local path scan in offline preflight',async t=>{
  const req=await fixture();
  if(process.platform==='win32'){
    // The SDK rejects writable ancestors of some user TEMP folders. Test with a
    // new private output under the user's trusted home, retaining the temp source.
    const home=await realpath(homedir());
    const privateDir=await mkdtemp(join(home,'.openkoda-security-tests-'));
    req.outputDir=join(privateDir,'output');
    t.after(async()=>{
      const actual=await realpath(privateDir);
      assert.equal(dirname(actual),home);
      assert.ok(basename(actual).startsWith('.openkoda-security-tests-'));
      assert.equal(actual,privateDir);
      await rm(actual,{recursive:true});
    });
  }
  const paths=await preparePaths(req);
  const originalHome=process.env.CODEX_HOME,originalState=process.env.CODEX_SECURITY_STATE_DIR;
  process.env.CODEX_HOME=paths.configDir;
  process.env.CODEX_SECURITY_STATE_DIR=paths.stateDir;
  process.env.OPENAI_API_KEY=req.apiKey; // Offline preflight checks presence only.
  let client;
  try{
    const {CodexSecurity}=await import('@openai/codex-security');
    client=new CodexSecurity({codexOverrides:{model:req.model,model_provider:req.provider}});
    const checked=await client.preflight(paths.repository,{target:req.target,maxCostUsd:req.maxCostUsd,outputDir:paths.artifacts,mode:'standard',auth:'api-key'});
    assert.equal(await realpath(checked.repository),paths.repository);
    assert.equal(checked.maxCostUsd,1);
    assert.equal(checked.mode,'standard');
  }finally{
    await client?.close();
    delete process.env.OPENAI_API_KEY;
    if(originalHome===undefined)delete process.env.CODEX_HOME;else process.env.CODEX_HOME=originalHome;
    if(originalState===undefined)delete process.env.CODEX_SECURITY_STATE_DIR;else process.env.CODEX_SECURITY_STATE_DIR=originalState;
  }
});
test('installed check reports the real SDK and local prerequisites without a key',async()=>{
  const run=await cli(['--check'],[]);
  assert.equal(run.stderr,'');assert.equal(run.frames.length,1);
  assert.equal(run.frames[0].type,'check');assert.equal(run.frames[0].sdkVersion,sdkVersion);
  assert.equal(run.frames[0].available,true,JSON.stringify(run.frames[0]));
  assert.equal(run.code,0);
});
test('rejects an oversized frame before starting SDK and produces one safe terminal event',async()=>{
  const run=await cli([],['x'.repeat(65537)]);
  assert.equal(run.frames.length,1);assert.equal(run.frames[0].code,'invalid_request');assert.equal(run.code,1);assert.equal(run.stderr,'');
});
test('cancel before preflight completes returns a cancellation without provider work',async()=>{
  const req=await fixture();
  const run=await cli([],[JSON.stringify(req),JSON.stringify({operation:'cancel'})]);
  assert.equal(run.frames.length,1);assert.equal(run.frames[0].status,'cancelled');assert.equal(run.code,1);
  assert.equal(run.frames[0].cost,null);assert.equal(run.stderr,'');assert.ok(!run.stdout.includes(key));
  await assert.rejects(realpath(req.outputDir),{code:'ENOENT'});
});
