import test from 'node:test';
import assert from 'node:assert/strict';
import { randomUUID } from 'node:crypto';
import { dirname } from 'node:path';
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
