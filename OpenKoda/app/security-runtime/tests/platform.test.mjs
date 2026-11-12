import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { basename, dirname, join, resolve } from 'node:path';
import { tmpdir } from 'node:os';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import childProcess from 'node:child_process';
import { syncBuiltinESMExports } from 'node:module';
import { promisify } from 'node:util';
import { inspectPlatform, probeBinaryVersion } from '../platform.mjs';

test('platform reports bounded version diagnostics without process output or paths',async()=>{
  const {check}=await inspectPlatform();
  assert.ok(check.diagnostics?.codex);
  for(const component of ['python','git','codex']){
    const result=check.diagnostics[component];
    assert.ok(result.attempts.length>0 && result.attempts.length<=8);
    for(const attempt of result.attempts){
      assert.deepEqual(Object.keys(attempt).sort(),['candidate','category','childKilled','code','elapsedMs','exitCode','signal','spawnElapsedMs','stderrBytes','stdoutBytes']);
      assert.ok(Number.isInteger(attempt.elapsedMs) && attempt.elapsedMs>=0);
    }
  }
});

test('version failures retain only safe error metadata, never child output or arguments',async()=>{
  const secret='synthetic-provider-body-that-must-stay-private';
  const cases=[
    {script:`process.stderr.write('${secret}');process.exit(9)`,category:'exit',code:9},
    {script:`process.stdout.write('${secret}')`,category:'output_format',code:null},
    {script:`process.stdout.write('${secret}'.repeat(1000))`,category:'output_limit',code:'ERR_CHILD_PROCESS_STDIO_MAXBUFFER'},
  ];
  for(const entry of cases){
    const result=await probeBinaryVersion([process.execPath],['-e',entry.script],/^codex-cli ([0-9.]+)$/);
    assert.equal(result.binary,null);
    assert.equal(result.diagnostic.attempts[0].category,entry.category);
    assert.equal(result.diagnostic.attempts[0].code,entry.code);
    const encoded=JSON.stringify(result.diagnostic);
    assert.ok(!encoded.includes(secret));
    assert.ok(!encoded.includes(process.execPath));
    assert.ok(!encoded.includes('"stderr":') && !encoded.includes('"stdout":'));
    assert.ok(Number.isInteger(result.diagnostic.attempts[0].stdoutBytes));
    assert.ok(Number.isInteger(result.diagnostic.attempts[0].stderrBytes));
  }
});

test('candidate failures remain explicit and diagnostic output is bounded without implicit retry',async()=>{
  const missing=process.execPath+'.openkoda-missing-'+randomUUID();
  const absent=await probeBinaryVersion([missing],['--version'],/^codex-cli ([0-9.]+)$/);
  assert.equal(absent.binary,null);
  assert.deepEqual(absent.diagnostic.attempts.map(a=>[a.candidate,a.category,a.code]),[[1,'missing','ENOENT']]);
  const directory=await probeBinaryVersion([dirname(process.execPath)],[],/^unused$/);
  assert.equal(directory.diagnostic.attempts[0].category,'not_file');
  const found=await probeBinaryVersion([...Array(9).fill(missing),process.execPath],['-e','console.log("codex-cli 0.167.1")'],/^codex-cli ([0-9.]+)$/);
  assert.equal(found.binary.version,'0.167.1');
  assert.equal(found.diagnostic.attempts.length,8);
  assert.equal(found.diagnostic.additionalAttempts,2);
});

test('a real probe timeout is identified without extending the production ten-second budget',async()=>{
  const result=await probeBinaryVersion([process.execPath],['-e','setInterval(()=>{},1000)'],/^unused$/,100);
  assert.equal(result.binary,null);
  assert.equal(result.diagnostic.attempts[0].category,'timeout');
  assert.ok(result.diagnostic.attempts[0].elapsedMs>=100);
  await assert.rejects(probeBinaryVersion([],[],/^unused$/,10001),/invalid_probe_timeout/);
});

test('a killed child cannot confirm a version even if execFile resolves exit zero',async()=>{
  let calls=0;
  const synthetic=()=>{throw new Error('Only the promisified process adapter is used');};
  synthetic[promisify.custom]=()=>{
    calls++;
    const pending=Promise.resolve({stdout:'codex-cli 0.167.1\n',stderr:''});
    pending.child={killed:true,exitCode:0,signalCode:null};
    return pending;
  };
  const original=childProcess.execFile;
  childProcess.execFile=synthetic;syncBuiltinESMExports();
  try{
    const {probeBinaryVersion:probe}=await import('../platform.mjs?killed-response-fixture');
    const result=await probe([process.execPath],['--version'],/^codex-cli ([0-9.]+)$/);
    assert.equal(calls,1);
    assert.equal(result.binary,null);
    assert.equal(result.diagnostic.attempts[0].category,'terminated');
    assert.equal(result.diagnostic.attempts[0].childKilled,true);
    assert.equal(result.diagnostic.attempts[0].exitCode,0);
  }finally{
    childProcess.execFile=original;syncBuiltinESMExports();
  }
});

test('SDK loading finishes before any version probe starts',async()=>{
  const folder=await mkdtemp(join(tmpdir(),'openkoda-platform-order-'));
  try{
    const sdkSource="globalThis.__sdkOrder.events.push({type:'sdk-evaluate',active:globalThis.__sdkOrder.active});const until=Date.now()+25;while(Date.now()<until){};globalThis.__sdkOrder.loaded=true;export class CodexSecurity{}";
    await writeFile(join(folder,'loader.mjs'),`
export async function resolve(specifier,context,nextResolve){
  if(specifier==='@openai/codex-security'&&context.parentURL?.includes('sdk-load-order-fixture'))return {url:'openkoda-order-fixture:sdk',shortCircuit:true};
  return nextResolve(specifier,context);
}
export async function load(url,context,nextLoad){
  if(url==='openkoda-order-fixture:sdk')return {format:'module',source:${JSON.stringify(sdkSource)},shortCircuit:true};
  return nextLoad(url,context);
}`);
    const platform=new URL('../platform.mjs?sdk-load-order-fixture',import.meta.url).href;
    await writeFile(join(folder,'fixture.mjs'),`
import childProcess from 'node:child_process';
import fs from 'node:fs/promises';
import {register,syncBuiltinESMExports} from 'node:module';
import {promisify} from 'node:util';
globalThis.__sdkOrder={active:0,loaded:false,events:[]};
const originalRead=fs.readFile;
fs.readFile=async(path,...args)=>{
  if(String(path).replaceAll('\\\\','/').endsWith('/@openai/codex-security/package.json')){
    await new Promise(done=>setTimeout(done,50));return JSON.stringify({name:'@openai/codex-security',version:'0.10.0'});
  }
  return originalRead(path,...args);
};
fs.access=async()=>{};fs.realpath=async path=>String(path);fs.lstat=async()=>({isFile:()=>true});fs.readdir=async()=>[];
const synthetic=()=>{throw new Error('Unexpected callback adapter');};
synthetic[promisify.custom]=(exe)=>{
  const state=globalThis.__sdkOrder;state.active++;state.events.push({type:'probe-start',sdkLoaded:state.loaded});
  const stdout=exe.includes('python')?'3.14.7\\n':/git(?:\\.exe)?$/.test(exe)?'git version 2.55.0\\n':'codex-cli 0.167.1\\n';
  const pending=new Promise(done=>{
    const finish=()=>{if(!state.loaded){setTimeout(finish,5);return;}state.active--;done({stdout,stderr:''});};
    setTimeout(finish,5);
  });
  pending.child={killed:false,exitCode:0,signalCode:null};return pending;
};
childProcess.execFile=synthetic;syncBuiltinESMExports();register(new URL('./loader.mjs',import.meta.url));
const {inspectPlatform}=await import(${JSON.stringify(platform)});
const {check}=await inspectPlatform();
const events=globalThis.__sdkOrder.events;
console.log(JSON.stringify({available:check.available,sdkEvaluatedWithActiveProbes:events.find(event=>event.type==='sdk-evaluate')?.active,probesStartedBeforeSdk:events.filter(event=>event.type==='probe-start'&&!event.sdkLoaded).length,probes:events.filter(event=>event.type==='probe-start').length}));
`);
    const {stdout}=await promisify(childProcess.execFile)(process.execPath,[join(folder,'fixture.mjs')],{
      windowsHide:true,timeout:5000,maxBuffer:8192,
      env:{SystemRoot:process.env.SystemRoot??'C:\\Windows',TEMP:tmpdir(),TMP:tmpdir()},
    });
    const result=JSON.parse(stdout);
    assert.equal(result.available,true);
    assert.equal(result.probes,3);
    assert.equal(result.sdkEvaluatedWithActiveProbes,0,'SDK evaluation must not overlap live version probes');
    assert.equal(result.probesStartedBeforeSdk,0,'version probes must wait for SDK loading');
  }finally{
    assert.equal(dirname(folder),resolve(tmpdir()));
    assert.ok(basename(folder).startsWith('openkoda-platform-order-'));
    await rm(folder,{recursive:true,force:true});
  }
});
