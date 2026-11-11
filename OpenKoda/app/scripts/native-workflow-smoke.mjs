// OpenKoda 0.1.11 native IPC fixture. All application data and HTTP peers are owned
// by this run. The exact coordinator-reviewed custom-protocol binary is required.
// No provider, credential, WSL, personal profile, or existing target is used.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer as tcpServer } from 'node:net';
import { createServer as httpServer } from 'node:http';
import { mkdir, readFile, writeFile, realpath, lstat } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { reviewedAssessment } from './scope-fixture.mjs';
import { authorizeData, reviewedBackup } from './data-fixture.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--expected-sha256' || !/^[a-f0-9]{64}$/.test(args[1])) throw new Error('Use --expected-sha256 <coordinator-reviewed 0.1.11 custom-protocol debug hash>.');
if (process.platform !== 'win32') throw new Error('This fixture requires native Windows WebView2.');
const hash = value => createHash('sha256').update(value).digest('hex');
const binary = join(root, 'src-tauri', 'target', 'debug', 'openkoda.exe');
const binaryBytes = await readFile(binary);
if (await realpath(binary) !== binary || hash(binaryBytes) !== args[1]) throw new Error('Debug binary differs from the reviewed artifact.');
const legacyRelease = join(root, 'releases', '0.1.10-local-20261111');
const legacyManifest = JSON.parse((await readFile(join(legacyRelease, 'release.json'), 'utf8')).replace(/^\uFEFF/, ''));
const legacyBinary = join(legacyRelease, 'openkoda-debug.exe');
if (legacyManifest.version !== '0.1.10' || legacyManifest.schema !== 8 || await realpath(legacyBinary) !== legacyBinary || hash(await readFile(legacyBinary)) !== legacyManifest.nativeTestBinarySha256) throw new Error('Preserved 0.1.10 artifact differs from its release manifest.');
const directory = join(root, '.local', `native-workflow-${randomUUID()}`);
const dataDirectory = join(directory, 'data');
await mkdir(directory, { recursive: true });
if (await realpath(directory) !== directory) throw new Error('Fixture directory must not redirect.');
const result = { passed: false, directory, artifact: { version: '0.1.11', binary, sha256: args[1], bytes: binaryBytes.length, scriptSha256: hash(await readFile(fileURLToPath(import.meta.url))) }, records: [], limits: [
  'Mutations use public native IPC. Read-only SQLite is restricted to this run and checks actual journals, capture bytes and hashes; it injects no authority.',
  'Only the owned loopback HTTP fixture and WebView2 CDP endpoint are contacted. No credential, provider, WSL, personal TXT or pre-existing profile is accessed.',
  'The fixture releases held responses explicitly. Cancellation confirms local completion; it does not undo a HEAD already received by the peer.',
  'Restart means restarting this owned process, not a power-loss test. Restore is an inactive copy, never activation of a profile.',
  'Artifacts are retained. Cleanup closes only this run\'s process, browser connection and listener; no recursive deletion.',
  'This validates native host behavior and persistence; screenshots are evidence, not a claim of full visual or accessibility validation.',
] };
const ipc = [], diagnostics = [], errors = [], traffic = [], held = new Map();
let child, browser, page, boot = 0, failure, assessmentId, projectId, firstEnqueue, firstPlanCommit, firstCapture, firstCursor, staleRead, legacyProject, legacyRawHash;

function owned(value, base = directory) {
  const ordinary = value.startsWith('\\\\?\\') && /^[a-z]:\\/i.test(value.slice(4)) ? value.slice(4) : value;
  const path = resolve(ordinary), part = relative(base, path);
  if (!part || isAbsolute(part) || part === '..' || part.startsWith(`..${sep}`)) throw new Error('Artifact escaped the owned fixture.');
  return path;
}
async function plainOwned(value, base = directory) {
  const path = owned(value, base);
  if (await realpath(path) !== path || (await lstat(path)).isSymbolicLink()) throw new Error('Owned artifact redirects.');
  return path;
}
async function audit(base = dataDirectory) {
  const db = new DatabaseSync(await plainOwned(join(base, 'openkoda.sqlite3')), { readOnly: true });
  let core;
  try {
    const raw = db.prepare('SELECT json FROM app_state WHERE id=1').get().json;
    const manifests = db.prepare('SELECT data FROM capture_manifests ORDER BY id').all().map(row => JSON.parse(row.data));
    const captures = manifests.map(manifest => {
      const chunks = db.prepare('SELECT chunk_index,offset,sha256,bytes FROM capture_chunks WHERE capture_id=? ORDER BY chunk_index').all(manifest.id);
      let offset = 0;
      for (const [index, chunk] of chunks.entries()) { expect(chunk.chunk_index).toBe(index); expect(chunk.offset).toBe(offset); expect(hash(chunk.bytes)).toBe(chunk.sha256); offset += chunk.bytes.length; }
      const bytes = Buffer.concat(chunks.map(chunk => Buffer.from(chunk.bytes)));
      expect(bytes.length).toBe(manifest.preservedBytes);
      if (manifest.state !== 'open' && manifest.state !== 'purged') expect(hash(bytes)).toBe(manifest.preservedSha256);
      return { manifest, sha256: hash(bytes), bytes: bytes.length };
    });
    core = { schema: db.prepare('PRAGMA user_version').get().user_version, rawSha256: hash(raw), state: JSON.parse(raw), captures,
      heads: db.prepare('SELECT * FROM work_plan_heads ORDER BY assessment_id').all(),
      revisions: db.prepare('SELECT data FROM work_plan_revisions ORDER BY plan_id,number').all().map(row => JSON.parse(row.data)),
      attempts: db.prepare('SELECT data FROM work_attempts ORDER BY job_id,number').all().map(row => JSON.parse(row.data)),
      leaseCount: db.prepare('SELECT count(*) AS n FROM work_leases').get().n,
      commandCount: db.prepare('SELECT count(*) AS n FROM work_commands').get().n };
  } finally { db.close(); }
  const ai = new DatabaseSync(await plainOwned(join(base, 'assistant.sqlite3')), { readOnly: true });
  try { core.requestCount = ai.prepare('SELECT count(*) AS n FROM requests').get().n; core.vaultCount = ai.prepare('SELECT count(*) AS n FROM vault').get().n; }
  finally { ai.close(); }
  return core;
}
async function assertNoForeignApplications() {
  const command = "$entries=@(Get-Process -Name 'openkoda','openkoda-debug' -ErrorAction SilentlyContinue | Select-Object Id,Path); ConvertTo-Json -InputObject $entries -Compress; exit 0";
  const output = await new Promise((ok, bad) => {
    const probe = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], signal: AbortSignal.timeout(15000) });
    let out = '', err = ''; probe.on('error', bad); probe.stdout.on('data', bytes => { out += bytes; if (out.length > 32768) probe.kill(); }); probe.stderr.on('data', bytes => { err += bytes; });
    probe.once('exit', (code, signal) => code === 0 && !signal ? ok(out) : bad(new Error(`Application inventory failed: ${code ?? signal}; ${err}`)));
  });
  const parsed = output.trim() ? JSON.parse(output) : [], entries = Array.isArray(parsed) ? parsed : [parsed];
  if (entries.length) throw new Error('A pre-existing OpenKoda process is running; it has been preserved. Close it explicitly before this fixture.');
}
// CloseMainWindow posts the same WM_CLOSE as the native title-bar X. Limit it to this exact owned PID and image.
async function requestOwnedWindowClose() {
  expect(child.exitCode).toBeNull(); expect(child.signalCode).toBeNull();
  const command = [
    "$ErrorActionPreference='Stop'",
    "$app=Get-Process -Id ([int]$env:OPENKODA_FIXTURE_PID)",
    "if($app.Path -ne $env:OPENKODA_FIXTURE_IMAGE -or $app.MainWindowHandle -eq 0){throw 'Unexpected process or window.'}",
    "if(-not $app.CloseMainWindow()){throw 'WM_CLOSE request rejected.'}",
  ].join(';');
  await new Promise((ok,bad)=>{
    const proc=spawn('powershell.exe',['-NoProfile','-NonInteractive','-Command',command],{
      windowsHide:true,stdio:['ignore','ignore','pipe'],signal:AbortSignal.timeout(8000),
      env:{...process.env,OPENKODA_FIXTURE_PID:String(child.pid),OPENKODA_FIXTURE_IMAGE:binary},
    });
    let err='';proc.stderr.on('data',chunk=>{err+=chunk;});
    proc.once('error',bad);proc.once('exit',(code,signal)=>code===0&&!signal?ok():bad(new Error('Owned WM_CLOSE failed: '+err)));
  });
}
async function checkBusyWindowClose(reviewQueue) {
  await requestOwnedWindowClose();
  const dialog=page.getByRole('dialog',{name:'Há uma operação em andamento'});
  await expect(dialog).toBeVisible({timeout:5000});
  await expect(dialog.getByRole('button',{name:'Manter aberto'})).toBeFocused();
  await dialog.getByRole('button',{name:reviewQueue?'Revisar plano e fila':'Manter aberto'}).click();
  await expect(dialog).toHaveCount(0);
  if(reviewQueue)await expect(page.getByRole('button',{name:'Plano e fila',exact:true})).toHaveAttribute('aria-current','page');
  expect(child.exitCode).toBeNull();expect(child.signalCode).toBeNull();
}
async function unusedPort() {
  const server = tcpServer(); await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  const port = server.address().port; await new Promise((ok, bad) => server.close(error => error ? bad(error) : ok())); return port;
}
async function openApp(executable = binary, expectedHash = args[1]) {
  await assertNoForeignApplications(); expect(hash(await readFile(executable))).toBe(expectedHash);
  boot++; const port = await unusedPort();
  child = spawn(executable, [], { cwd: dirname(executable), windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, OPENKODA_DATA_DIR: dataDirectory, WEBVIEW2_USER_DATA_FOLDER: join(directory, `webview-${boot}`), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}` } });
  result.ownedPids ??= []; result.ownedPids.push(child.pid); let startupError;
  child.on('error', error => { startupError = error; }); child.stdout.on('data', bytes => diagnostics.push(`boot ${boot} stdout: ${bytes}`)); child.stderr.on('data', bytes => diagnostics.push(`boot ${boot} stderr: ${bytes}`));
  await expect.poll(async () => { if (startupError) throw startupError; if (child.exitCode !== null || child.signalCode !== null) throw new Error('Owned app exited during startup.'); try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; } }, { timeout: 45000 }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`); const context = browser.contexts()[0];
  await expect.poll(() => context.pages().length, { timeout: 15000 }).toBeGreaterThan(0); page = context.pages()[0]; page.setDefaultTimeout(15000);
  page.on('console', message => diagnostics.push(`console ${message.type()}: ${message.text()}`)); page.on('pageerror', error => errors.push(error.message));
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function'); expect(new URL(page.url()).hostname).toBe('tauri.localhost');
  await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
}
async function closeApp() {
  try { if (browser) { await browser.contexts()[0]?.tracing.stop({ path: join(directory, `trace-${boot}.zip`) }).catch(error => diagnostics.push(String(error))); await browser.close(); } }
  finally { browser = undefined; page = undefined; if (child && child.exitCode === null && child.signalCode === null) { child.kill(); await expect.poll(() => child.exitCode !== null || child.signalCode !== null, { timeout: 10000 }).toBe(true); } }
}
async function request(action, payload = {}) {
  const entry = { action, at: new Date().toISOString() }; ipc.push(entry);
  try { const value = await page.evaluate(async ({ action, payload }) => { let timer; try { return await Promise.race([window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }), new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('WORKFLOW_IPC_DEADLINE')), 30000); })]); } finally { clearTimeout(timer); } }, { action, payload }); entry.status = 'fulfilled'; return value; }
  catch (error) { entry.status = 'rejected'; entry.error = String(error); throw error; }
}
async function denied(action, payload, code) {
  let error; const hits = traffic.length;
  try { await request(action, payload); } catch (caught) { error = String(caught); }
  expect(error, `${action} must reject explicitly`).toBeTruthy(); expect(error).not.toMatch(/WORKFLOW_IPC_DEADLINE|Target closed|Page crashed/);
  if (code) expect(error).toContain(code); expect(traffic.length).toBe(hits); return error;
}
async function gate(id, task) {
  const record = { id, status: 'running', startedAt: new Date().toISOString() }; result.records.push(record);
  try { record.detail = await task(); record.status = 'passed'; } catch (error) { record.status = 'failed'; record.error = String(error); throw error; } finally { record.endedAt = new Date().toISOString(); }
}
const plan = async () => (await request('work.plan.get', { assessmentId, revisionId: null })).plan;
const jobRef = job => ({ id: job.id, generation: job.generation });
const native = (key, dependsOnKeys = []) => ({ key, existingNodeId: null, purpose: 'Observe only the owned loopback fixture.', expectedObservation: 'One bounded HTTP HEAD observation.', dependsOnKeys, intent: { kind: 'native', adapter: 'http.headers.v1', targetRef: null, ports: null } });
async function newPlan(nodes, current = null) {
  const preview = await request('work.plan.prepare', { assessmentId, expectedHead: current?.head ?? null, expectedGeneration: current?.generation ?? 0, goal: 'Review each bounded synthetic operation.', nodes, limits: { maxAttemptsTotal: 10, maxCaptureBytesTotal: 4 * 1024 * 1024 } });
  const payload = { review: preview.review, idempotencyKey: randomUUID(), acknowledgePlan: true }; const value = await request('work.plan.commit', payload); return { preview, payload, value };
}
async function prepare(index) { const p = await plan(); const payload = { job: jobRef(p.jobs[index]), revision: p.head, idempotencyKey: randomUUID() }; return { payload, result: await request('work.operation.prepare', payload), revision: p.head }; }
async function enqueue(prepared) { const payload = { kind: 'native', job: jobRef(prepared.result.value.job), revision: prepared.revision, compiled: prepared.result.value.compiled, idempotencyKey: randomUUID(), acknowledgeExecution: true }; return { payload, result: await request('work.operation.enqueue', payload) }; }
async function awaitJob(id, state) { await expect.poll(async () => (await plan()).jobs.find(job => job.id === id)?.state, { timeout: 45000 }).toBe(state); return (await plan()).jobs.find(job => job.id === id); }
async function readCapture(cursorId = null) {
  const preview = await request('capture.access.prepare', { assessmentId, captureId: firstCapture.id, expectedManifestDigest: firstCapture.digest, cursorId, maxBytes: 64 });
  expect(preview.canRead).toBe(true); expect(preview).not.toHaveProperty('text');
  const payload = { review: preview.review, acknowledgeLocalRead: true, acknowledgeSensitive: preview.classification.sensitivity === 'sensitive' };
  const value = await request('capture.access.read', payload); expect(Buffer.byteLength(value.text)).toBe(value.returnedBytes); expect(value.rangeEnd - value.rangeStart).toBe(value.returnedBytes);
  return { preview, payload, value };
}
const server = httpServer((req, res) => {
  const number = traffic.length + 1; traffic.push({ number, method: req.method, url: req.url, authorization: Boolean(req.headers.authorization), cookie: Boolean(req.headers.cookie) });
  if (req.method !== 'HEAD' || req.url !== '/fixture' || req.headers.authorization || req.headers.cookie) { res.writeHead(400); res.end(); return; }
  const finish = () => { if (!res.destroyed) { res.writeHead(200, { 'X-Content-Type-Options': 'nosniff', 'Content-Security-Policy': "default-src 'none'; frame-ancestors 'none'", 'Referrer-Policy': 'no-referrer', 'Content-Length': '0', Connection: 'close' }); res.end(); } held.delete(number); };
  if (number === 2 || number === 3) held.set(number, finish); else finish();
});
await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
const target = `http://127.0.0.1:${server.address().port}/fixture`;
let hostCheckpoint;
let previousEnvironmentState;
async function environmentDatabase(version) {
  const db = new DatabaseSync(await plainOwned(join(dataDirectory, 'environment.sqlite3')), { readOnly: true });
  try {
    expect(db.prepare('PRAGMA user_version').get().user_version).toBe(version);
    if (version === 2) expect(db.prepare('SELECT count(*) AS count FROM environment_removals').get().count).toBe(0);
    return hash(db.prepare('SELECT data FROM environment_state WHERE id=1').get().data);
  } finally { db.close(); }
}

try {
  await openApp(legacyBinary, legacyManifest.nativeTestBinarySha256);
  await gate('preserved-previous-release-creates-fixture', async () => {
    expect((await request('snapshot')).projects).toEqual([]);
    legacyProject = await request('project.create', { name: 'Preserved 0.1.10 project', kind: 'web', target, description: 'Synthetic pre-upgrade metadata created through the preserved native IPC.' });
    const db = new DatabaseSync(await plainOwned(join(dataDirectory, 'openkoda.sqlite3')), { readOnly: true });
    try {
      expect(db.prepare('PRAGMA user_version').get().user_version).toBe(8);
      const raw = db.prepare('SELECT json FROM app_state WHERE id=1').get().json; legacyRawHash = hash(raw);
      // IPC adds projection metadata to its response, not to the stored project.
      const stored = JSON.parse(raw).projects.find(project => project.id === legacyProject.id);
      expect(stored).toBeTruthy(); expect(stored.name).toBe(legacyProject.name); legacyProject = stored;
    }
    finally { db.close(); }
    expect(traffic).toHaveLength(0);
    previousEnvironmentState = await environmentDatabase(2);
    return { version: '0.1.10', sha256: legacyManifest.nativeTestBinarySha256, schema: 8, environmentSchema: 2, projectId: legacyProject.id, coreSha256: legacyRawHash };
  });
  await closeApp();
  await openApp();
  await gate('new-release-preserves-exact-project-history', async () => {
    const migrated = await audit(); expect(migrated.schema).toBe(8); expect(migrated.rawSha256).toBe(legacyRawHash);
    expect(migrated.state.projects).toEqual([legacyProject]); expect(migrated.attempts).toEqual([]); expect(migrated.captures).toEqual([]);
    expect(migrated.requestCount).toBe(0); expect(migrated.vaultCount).toBe(0); expect(traffic).toHaveLength(0);
    expect(await environmentDatabase(2)).toBe(previousEnvironmentState);
    return { schema: [8, 8], environmentSchema: [2, 2], exactEnvironmentPreserved: true, exactJsonPreserved: true, existingProjectUnchanged: true, noAutomaticDispatch: true };
  });
  await gate('windows-preparation-and-retirement-start-without-authority', async () => {
    expect(await request('environment.host.resume')).toBeNull();
    const host = await request('environment.host.check');
    expect(typeof host.startedAt).toBe('string');
    expect(Number.isFinite(Date.parse(host.checkedAt))).toBe(true);
    expect(typeof host.step).toBe('string');
    hostCheckpoint = host.startedAt;
    expect((await request('environment.removal.status')).record).toBeNull();
    return { step: host.step, checkpointCreated: true, removalRecorded: false };
  });
  await gate('native-dag-pure-plan-and-single-first-dispatch', async () => {
    expect((await request('snapshot')).projects).toEqual([legacyProject]);
    const project = await request('project.create', { name: 'Owned workflow fixture', kind: 'web', target, description: 'Loopback listener owned by native-workflow-smoke.mjs.' }); projectId = project.id;
    assessmentId = (await reviewedAssessment(request, { projectId, title: 'Native workflow 0.1.11', kind: 'web', target, objective: 'Validate reviewed native DAG without provider or Linux.' }, ['http.headers.v1'])).id;
    firstPlanCommit = await newPlan([native('a'), { key: 'b', existingNodeId: null, purpose: 'Review the observation identity.', expectedObservation: 'An explicit human decision.', dependsOnKeys: ['a'], intent: { kind: 'human_review', inputs: [{ kind: 'dependency_output', nodeKey: 'a', outputRole: 'evidence' }] } }, native('c', ['b'])]);
    expect(traffic).toHaveLength(0); expect((await audit()).state.executions).toEqual([]); expect((await plan()).jobs[1].canPrepare).toBe(false);
    const a = await prepare(0); expect(traffic).toHaveLength(0);
    const same = await prepare(0); expect(same.result.value.compiled).toEqual(a.result.value.compiled);
    for (const action of ['plan.approve', 'plan.run']) await denied(action, { id: a.result.value.compiled.planId, digest: a.result.value.compiled.digest }, 'work_owned_operation');
    firstEnqueue = await enqueue(a); await awaitJob(a.result.value.job.id, 'completed'); expect(traffic).toHaveLength(1);
    expect((await request('work.operation.enqueue', firstEnqueue.payload)).replayed).toBe(true); expect(traffic).toHaveLength(1);
    const state = await audit(); expect(state.schema).toBe(8); expect(state.state.executions).toHaveLength(1); expect(state.leaseCount).toBe(0);
    const current = await plan(); expect(current.jobs[1].state).toBe('awaiting_preparation'); expect(current.jobs[2].state).toBe('waiting_dependencies');
    const snapshot = await request('snapshot'); expect(snapshot.workPlanBindings).toContainEqual(expect.objectContaining({ executionPlanId: a.result.value.compiled.planId, jobId: a.result.value.job.id }));
    expect(snapshot.assessments.find(item => item.id === assessmentId).evidence[0]).not.toHaveProperty('content');
    return { assessmentId, planId: current.id, firstJobId: current.jobs[0].id, firstExecutionId: state.state.executions[0].id, requests: traffic.length, repeatedExecution: false };
  });
  await gate('human-review-does-not-dispatch-pause-does-not-claim-cancel', async () => {
    const human = await prepare(1); expect(human.result.value.kind).toBe('human_review'); expect(human.result.value.canRecord).toBe(true); expect(traffic).toHaveLength(1);
    const input = { job: jobRef(human.result.value.job), revision: human.revision, review: human.result.value.review, inputDigest: human.result.value.inputDigest, decision: 'continue', note: 'Reviewed the owned synthetic observation, without validating a finding.', idempotencyKey: randomUUID(), acknowledgeReview: true };
    const recorded = await request('work.review.commit', input); expect((await request('work.review.commit', input)).replayed).toBe(true); expect(recorded.replayed).toBe(false); expect(traffic).toHaveLength(1);
    const c = await prepare(2); const queued = await enqueue(c); await expect.poll(() => held.has(2), { timeout: 15000 }).toBe(true); await awaitJob(c.result.value.job.id, 'running');
    await checkBusyWindowClose(false);
    expect((await plan()).jobs[2].state).toBe('running');
    const active = await plan(); await request('work.pause', { planId: active.id, expectedGeneration: active.generation, idempotencyKey: randomUUID(), acknowledgePause: true });
    const paused = await plan(); expect(paused.control).toBe('paused'); expect(paused.jobs[2].state).toBe('running'); held.get(2)(); await awaitJob(c.result.value.job.id, 'completed');
    expect((await plan()).control).toBe('paused'); expect((await plan()).outcome).toBe('completed'); expect(traffic).toHaveLength(2);
    const resume = await request('work.resume.prepare', { planId: paused.id, expectedGeneration: (await plan()).generation }); await request('work.resume.commit', { review: resume.review, idempotencyKey: randomUUID(), acknowledgeResume: true });
    expect((await request('work.operation.enqueue', queued.payload)).replayed).toBe(true); expect(traffic).toHaveLength(2);
    const state = await audit(); expect(state.state.findings.filter(item => item.assessmentId === assessmentId)).toEqual([]); expect(state.state.executions).toHaveLength(2);
    return { humanReviewRecorded: true, secondRequest: 2, pausePreservedRunning: true, resumeDidNotRepeat: true };
  });
  await gate('new-revision-running-cancel-preserves-history-without-retry', async () => {
    const previous = await plan(); const historical = previous.revision;
    await newPlan([native('cancel')], previous); const p = await prepare(0), q = await enqueue(p);
    await expect.poll(() => held.has(3), { timeout: 15000 }).toBe(true); const running = await awaitJob(p.result.value.job.id, 'running');
    await checkBusyWindowClose(true);
    expect((await plan()).jobs[0].state).toBe('running');
    const input = { job: jobRef(running), idempotencyKey: randomUUID(), acknowledgeCancel: true };
    const canceled = await request('work.cancel', input); expect(canceled.value.terminationConfirmed).toBe(false); expect(canceled.value.job.state).toBe('cancel_requested');
    held.get(3)(); await awaitJob(running.id, 'canceled'); expect((await request('work.cancel', input)).replayed).toBe(true); expect((await request('work.operation.enqueue', q.payload)).replayed).toBe(true); expect(traffic).toHaveLength(3);
    const state = await audit(); expect(state.state.executions).toHaveLength(3); expect(state.leaseCount).toBe(0); expect(state.revisions[0]).toEqual(historical);
    expect(state.captures).toHaveLength(3); expect(state.captures.every(item => ['sealed', 'interrupted'].includes(item.manifest.state))).toBe(true);
    return { canceledAttemptId: (await plan()).attempts.at(-1).id, receivedHeadNotUndone: true, canonicalRevisionPreserved: true, requests: traffic.length };
  });
  await gate('capture-metadata-reviewed-cursor-and-consumed-read', async () => {
    await authorizeData(request, assessmentId); await request('data.session.select', { assessmentId });
    const listed = await request('capture.list', { assessmentId, attemptId: null }); expect(listed.captures).toHaveLength(3);
    firstCapture = listed.captures.find(item => item.manifest.state === 'sealed' && item.manifest.preservedBytes > 128).manifest;
    for (const item of listed.captures) { expect(item).not.toHaveProperty('text'); expect(item).not.toHaveProperty('bytes'); expect(item.manifest).not.toHaveProperty('content'); }
    const first = await readCapture(); expect(first.value.rangeStart).toBe(0); expect(first.value.hasMore).toBe(true); expect(first.value.cursor.offset).toBe(first.value.rangeEnd); firstCursor = first.value.cursor;
    const replay = await request('capture.access.read', first.payload); expect(replay.alreadyUsed).toBe(true); expect(replay).not.toHaveProperty('text'); expect(replay.receiptRef).toEqual(first.value.receiptRef);
    let text = first.value.text, cursor = firstCursor;
    for (let count = 0; count < 128 && cursor; count++) { const next = await readCapture(cursor.id); expect(next.value.rangeStart).toBe(cursor.offset); text += next.value.text; cursor = next.value.cursor; if (!next.value.hasMore) expect(cursor).toBeNull(); }
    expect(cursor).toBeNull(); expect(Buffer.byteLength(text)).toBe(first.preview.totalBytes); expect(hash(text)).toBe(first.preview.projectionSha256);
    // The reviewed projection can normalize line endings or redact content.
    // Its digest is distinct from the original bytes verified by audit().
    expect(first.preview.rawSha256).toBe(firstCapture.preservedSha256); expect(first.preview.rawBytes).toBe(firstCapture.preservedBytes);
    const records = text.trim().split('\n').map(line => JSON.parse(line));
    expect(records.filter(record => record.kind === 'http_status')).toEqual([expect.objectContaining({ method: 'HEAD', status: 200 })]);
    expect(records).toContainEqual({ encoding: 'utf8', kind: 'http_header', name: 'x-content-type-options', valueText: 'nosniff' });
    staleRead = await request('capture.access.prepare', { assessmentId, captureId: firstCapture.id, expectedManifestDigest: firstCapture.digest, cursorId: firstCursor.id, maxBytes: 64 });
    expect(traffic).toHaveLength(3); return { captureId: firstCapture.id, preservedSha256: firstCapture.preservedSha256, bytes: firstCapture.preservedBytes, consumedReviewHasNoText: true, cursorIsPositionOnly: true };
  });
  await gate('event-reload-and-restart-never-restore-execution-grants', async () => {
    const before = await audit(), current = await plan(); let afterSequence = 0; const sequences = [];
    for (let pageNumber = 0; pageNumber < 128; pageNumber++) { const events = await request('work.events', { assessmentId, streamId: current.id, afterSequence, limit: 3 }); expect(events.gap).toBe(false); for (const event of events.events) { expect(event).not.toHaveProperty('text'); expect(event).not.toHaveProperty('payload'); sequences.push(event.sequence); } afterSequence = events.next.afterSequence; if (!events.hasMore) break; }
    expect(sequences).toEqual(Array.from({ length: afterSequence }, (_, index) => index + 1)); expect(afterSequence).toBe(current.eventCursor.afterSequence);
    await page.reload(); await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 }); expect((await plan()).eventCursor.afterSequence).toBe(afterSequence); expect(traffic).toHaveLength(3);
    await closeApp(); await openApp(); const recovered = await plan(); expect(recovered.control).toBe('paused'); expect(recovered.eventCursor.afterSequence).toBeGreaterThan(afterSequence);
    expect((await request('work.operation.enqueue', firstEnqueue.payload)).replayed).toBe(true); expect((await request('work.plan.commit', firstPlanCommit.payload)).replayed).toBe(true);
    await denied('capture.access.read', { review: staleRead.review, acknowledgeLocalRead: true }); await request('data.session.select', { assessmentId });
    const renewed = await readCapture(firstCursor.id); expect(renewed.value.rangeStart).toBe(firstCursor.offset);
    const after = await audit(); expect(after.state.executions).toEqual(before.state.executions); expect(after.captures).toEqual(before.captures); expect(after.requestCount).toBe(0); expect(after.vaultCount).toBe(0); expect(traffic).toHaveLength(3);
    await page.screenshot({ path: join(directory, 'native-after-restart.png'), fullPage: true });
    return { priorEventSequence: afterSequence, recoveredEventSequence: recovered.eventCursor.afterSequence, control: recovered.control, staleReadRejected: true, cursorReReviewed: true, requests: traffic.length };
  });
  await gate('windows-preparation-checkpoint-survives-restart', async () => {
    const host = await request('environment.host.resume');
    expect(host.startedAt).toBe(hostCheckpoint);
    expect(Number.isFinite(Date.parse(host.checkedAt))).toBe(true);
    return { sameCheckpoint: true, step: host.step, rebootObserved: host.rebootObserved };
  });
  await gate('local-closure-public-router-never-invents-an-unknown-ai-effect', async () => {
    const before = await audit();
    const list = await request('work.aiLocalClosure.list', {});
    expect(list.items).toEqual([]); expect(list.candidates).toEqual([]);
    await denied('work.aiLocalClosure.prepare', { target: { kind: 'ai_request', requestId: randomUUID() } });
    const nativeAttemptId = before.attempts[0].view.id; expect(typeof nativeAttemptId).toBe('string');
    await denied('work.aiLocalClosure.prepare', { target: { kind: 'attempt', attemptId: nativeAttemptId } });
    await denied('work.aiLocalClosure.commit', {
      review: { id: randomUUID(), digest: hash('synthetic missing review') }, idempotencyKey: randomUUID(),
      note: 'Synthetic refusal check; no unknown AI effect exists in this profile.',
      acknowledgeRemoteUnknown: true, acknowledgeNoReplay: true, acknowledgeQueueInvalidation: true,
    });
    const after = await audit();
    expect(after).toEqual(before); expect(traffic).toHaveLength(3);
    return { emptyCandidates: true, nonexistentAndNativeEffectsRejected: true, fabricatedReviewRejected: true, unchangedStorage: true };
  });
  await gate('comparison-reads-only-selected-project-assessments', async () => {
    const second = await reviewedAssessment(request, {
      projectId, title:'Comparison fixture', kind:'web', target, objective:'Compare two local records without dispatch.',
    }, ['http.headers.v1']);
    const ids = [assessmentId, second.id];
    const prior = await audit();
    const previousTraffic = traffic.length;
    expect(await request('kali.mcp.assessmentEnvironments', {projectId,assessmentIds:ids})).toEqual([]);
    await denied('kali.mcp.assessmentEnvironments',{projectId:legacyProject.id,assessmentIds:ids});
    await page.reload();
    await expect(page.locator('#main-content')).toHaveAttribute('aria-busy','false');
    await page.getByRole('button',{name:'Projetos',exact:true}).click();
    await page.getByRole('button',{name:/Owned workflow fixture/}).click();
    await page.getByRole('navigation',{name:'Seções'}).getByRole('button',{name:'Avaliações'}).click();
    await page.getByText('Comparar avaliações',{exact:true}).click();
    await page.getByLabel('Avaliação A',{exact:true}).selectOption(ids[0]);
    await page.getByLabel('Avaliação B',{exact:true}).selectOption(ids[1]);
    await page.getByRole('button',{name:'Comparar registros'}).click();
    const details=page.getByRole('region',{name:'Resultado da comparação'});
    await expect(details.getByRole('region',{name:'Métodos da revisão de escopo'})).toContainText('http.headers.v1');
    await expect(details.getByRole('region',{name:'Identidades de ambiente no histórico do Kali'})).toContainText('Dados insuficientes');
    await expect(details).not.toContainText('O histórico de ambientes do Kali está indisponível.');
    const after=await audit();
    expect(after.rawSha256).toBe(prior.rawSha256);
    expect(after.attempts.length).toBe(prior.attempts.length);
    expect(after.requestCount).toBe(prior.requestCount);
    expect(traffic).toHaveLength(previousTraffic);
    return {distinctAssessments:2,projectBoundaryVerified:true,coreUnchanged:true,noAutomaticDispatch:true};
  });
  await gate('reviewed-backup-restore-keeps-capture-bytes-and-pauses-work', async () => {
    const before = await audit(); const backup = await reviewedBackup(request, { projectIds: [legacyProject.id, projectId] });
    expect((await request('backup.verify', { id: backup.backupId })).verified).toBe(true);
    const restored = await request('backup.restore', { id: backup.backupId }); expect(restored.activated).toBe(false);
    const copy = await audit(await plainOwned(restored.path, dataDirectory)); expect(copy.schema).toBe(8); expect(copy.captures).toEqual(before.captures); expect(copy.revisions).toEqual(before.revisions); expect(copy.state.executions).toEqual(before.state.executions);
    expect(copy.heads.every(head => head.control === 'paused')).toBe(true); expect(copy.state.dataPolicies.every(policy => !policy.active)).toBe(true); expect(copy.leaseCount).toBe(0); expect(copy.requestCount).toBe(0); expect(copy.vaultCount).toBe(0); expect(traffic).toHaveLength(3);
    return { backupId: backup.backupId, restoredPath: restored.path, activated: false, exactCaptureHashes: copy.captures.map(item => item.sha256), policiesInactive: true, workPaused: true };
  });
  await gate('native-window-close-keeps-active-worker-and-exits-when-idle', async()=>{
    // The two active WM_CLOSE paths above ran after enqueue had returned. Both effects completed through normal controls.
    await requestOwnedWindowClose();
    await expect.poll(()=>child.exitCode!==null||child.signalCode!==null,{timeout:15000}).toBe(true);
    expect(child.exitCode).toBe(0);expect(child.signalCode).toBeNull();
    return {activeCloseMethods:['keep-open','review-plan-and-queue'],idleClose:'WM_CLOSE',idleExitCode:child.exitCode};
  });
  expect(errors).toEqual([]); expect(traffic.every(item => item.method === 'HEAD' && item.url === '/fixture' && !item.authorization && !item.cookie)).toBe(true); result.passed = true;
} catch (error) { failure = error; result.error = String(error); result.stack = error?.stack; }
finally {
  if (failure && page) await page.screenshot({ path: join(directory, 'failure.png'), fullPage: true }).catch(error => diagnostics.push(String(error)));
  for (const finish of [...held.values()]) finish();
  await closeApp().catch(error => { failure ??= error; result.passed = false; result.cleanupError = String(error); });
  server.closeAllConnections(); await new Promise(ok => server.close(ok));
  result.ownedProcessClosed = !child || child.exitCode !== null || child.signalCode !== null; if (!result.ownedProcessClosed) result.passed = false;
  try { const final = await audit(); result.finalStorage = { schema: final.schema, coreSha256: final.rawSha256, attempts: final.attempts.length, executions: final.state.executions.length, captures: final.captures, leaseCount: final.leaseCount, providerRequests: final.requestCount, credentialRows: final.vaultCount }; if (result.passed) { expect(final.requestCount).toBe(0); expect(final.vaultCount).toBe(0); expect(final.leaseCount).toBe(0); } }
  catch (error) { diagnostics.push(String(error)); if (result.passed) { failure ??= error; result.passed = false; result.persistenceError = String(error); } }
  result.generatedAt = new Date().toISOString(); result.traffic = traffic; result.pageErrors = errors;
  await writeFile(join(directory, 'ipc-log.json'), JSON.stringify(ipc, null, 2)); await writeFile(join(directory, 'diagnostics.log'), diagnostics.join('\n')); await writeFile(join(directory, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ passed: result.passed, directory, gates: result.records.map(({ id, status }) => ({ id, status })), error: result.error, ownedProcessClosed: result.ownedProcessClosed }, null, 2));
}
if (failure || !result.passed) process.exitCode = 1;
