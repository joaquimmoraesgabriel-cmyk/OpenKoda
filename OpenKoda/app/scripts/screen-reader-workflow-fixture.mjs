// Native 0.1.6 screen-reader preparation. UI input belongs to the coordinator.
// Only public IPC creates records. SQLite is opened read-only for preservation.
import assert from 'node:assert/strict';
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { mkdir, readFile, writeFile, realpath, lstat } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { reviewedAssessment } from './scope-fixture.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
assert.ok(args.length === 2 && args[0] === '--expected-sha256' && /^[a-f0-9]{64}$/.test(args[1]), 'Use --expected-sha256 <reviewed 0.1.6 custom-protocol debug SHA-256>.');
assert.equal(process.platform, 'win32', 'Native Windows WebView2 is required.');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const expectedSha256 = args[1];
function confined(path, base = root) {
  const absolute = resolve(base, path), part = relative(base, absolute);
  assert.ok(part && !isAbsolute(part) && part !== '..' && !part.startsWith(`..${sep}`), 'Path escaped the owned workspace.');
  return absolute;
}
async function plain(path) {
  assert.equal(await realpath(path), path, 'Artifact path must not redirect.');
  assert.equal((await lstat(path)).isSymbolicLink(), false);
  return path;
}
const executable = await plain(confined('src-tauri/target/debug/openkoda.exe'));
const binaryBytes = await readFile(executable);
assert.equal(hash(binaryBytes), expectedSha256, 'Debug artifact differs from the coordinator-reviewed binary.');
assert.equal(JSON.parse(await readFile(join(root, 'src-tauri/tauri.conf.json'), 'utf8')).version, '0.1.6');
await plain(join(root, '.local'));
const directory = confined(join('.local', `screen-reader-workflow-${randomUUID()}`));
await mkdir(directory);
await plain(directory);
const dataDirectory = join(directory, 'data');
const finishPath = join(directory, 'finish.json');
const stopPath = join(directory, 'stop.json');
const receipt = {
  passed: false, directory,
  artifact: { version: '0.1.6', path: executable, sha256: expectedSha256, bytes: binaryBytes.length, scriptSha256: hash(await readFile(fileURLToPath(import.meta.url))) },
  observations: [],
  limits: [
    'Synthetic empty profile only. Public IPC prepares project, scope, imported text and work plans; no SQLite injection, authority fabrication or debug IPC hooks.',
    'No enqueue, execution, human-review commit, credential, provider, Kali command, WSL invocation, backup or real target is requested by this harness.',
    'The numeric loopback target is a socket reserved by this harness, with no HTTP service. Any connection fails preservation. Native preparation resolves only the numeric address.',
    'Startup may inspect Windows/WSL registration metadata; this harness neither invokes WSL nor owns a distro.',
    'The coordinator supplies all walkthrough UI input and observes real NVDA. CDP only prepares public IPC records and reads DOM diagnostics; it is not assistive-technology evidence.',
    'Native review is already prepared before baseline. Reopen it within 15 minutes; recompilation after expiry intentionally fails the strict core-preservation check.',
    'Human preparation can renew its five-minute review and append preparation commands/events. Those bounded metadata changes are audited separately from preserved core state.',
    'Capture list is legitimately empty. CaptureViewer is not rendered without an attempt with capture IDs. Capture read/pagination and actual unknown-AI closure require separate coverage.',
    'No confirmation with persistent effects may be submitted in the walkthrough. Closing a review, selecting transient values and reading metadata are allowed.',
    'An observed subset with NVDA and browser automation does not establish WCAG conformance. Profiles and all evidence are retained.',
  ],
};
const ipc = [], diagnostics = [], pageErrors = [], targetConnections = [];
let child, browser, page, observedExit = false, signalRequested = false, targetListener, baseline, assessmentId;
const scenarioIds = ['queue-navigation', 'native-review', 'human-review', 'control-review', 'empty-closure'];
const allowedActions = new Set(['snapshot', 'ai.status', 'project.create', 'assessment.scopeOptions', 'assessment.create', 'assessment.scopeStatus', 'evidence.import', 'work.plan.prepare', 'work.plan.commit', 'work.plan.get', 'work.queue', 'work.events', 'work.operation.prepare', 'capture.list', 'work.aiLocalClosure.list']);

async function powershell(command, extraEnv = {}) {
  return new Promise((ok, bad) => {
    const probe = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: { ...process.env, ...extraEnv }, signal: AbortSignal.timeout(15000) });
    let out = '', err = '';
    probe.on('error', bad);
    probe.stdout.on('data', bytes => { out += bytes; if (out.length > 32768) probe.kill(); });
    probe.stderr.on('data', bytes => { err += bytes; if (err.length > 32768) probe.kill(); });
    probe.once('exit', (code, signal) => code === 0 && !signal ? ok(out) : bad(new Error(`Metadata probe failed: ${code ?? signal}; ${err}`)));
  });
}
async function assertNoForeignApplication() {
  const output = await powershell("$entries=@(Get-Process -Name 'openkoda','openkoda-debug' -ErrorAction SilentlyContinue | Select-Object Id,Path); ConvertTo-Json -InputObject $entries -Compress; exit 0");
  const parsed = output.trim() ? JSON.parse(output) : [], entries = Array.isArray(parsed) ? parsed : [parsed];
  assert.deepEqual(entries, [], 'Pre-existing OpenKoda process preserved. Close it explicitly before running this fixture.');
}
async function listen(server) {
  await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  return server.address().port;
}
async function request(action, payload = {}) {
  assert.ok(allowedActions.has(action), `Harness action is not allowed: ${action}`);
  const entry = { action, at: new Date().toISOString() }; ipc.push(entry);
  try {
    const result = await page.evaluate(async ({ action, payload }) => {
      let timer;
      try { return await Promise.race([window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }), new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('WORKFLOW_IPC_DEADLINE')), 30000); })]); }
      finally { clearTimeout(timer); }
    }, { action, payload });
    entry.status = 'fulfilled'; return result;
  } catch (error) { entry.status = 'rejected'; entry.error = String(error); throw error; }
}
async function storageAudit() {
  const core = new DatabaseSync(await plain(confined('openkoda.sqlite3', dataDirectory)), { readOnly: true });
  let audit;
  try {
    const raw = core.prepare('SELECT json FROM app_state WHERE id=1').get().json;
    audit = {
      schema: core.prepare('PRAGMA user_version').get().user_version,
      coreSha256: hash(raw), state: JSON.parse(raw),
      heads: core.prepare('SELECT * FROM work_plan_heads ORDER BY plan_id').all(),
      revisions: core.prepare('SELECT data FROM work_plan_revisions ORDER BY id').all(),
      jobs: core.prepare('SELECT data FROM work_jobs ORDER BY id').all().map(row => JSON.parse(row.data)),
      commands: core.prepare('SELECT id,action,result FROM work_commands ORDER BY id').all(),
      events: core.prepare('SELECT data FROM work_events ORDER BY stream_id,sequence').all().map(row => JSON.parse(row.data)),
      attempts: core.prepare('SELECT count(*) AS n FROM work_attempts').get().n,
      leases: core.prepare('SELECT count(*) AS n FROM work_leases').get().n,
      captures: core.prepare('SELECT count(*) AS n FROM capture_manifests').get().n,
      closures: core.prepare('SELECT count(*) AS n FROM work_ai_dispositions').get().n,
    };
  } finally { core.close(); }
  const ai = new DatabaseSync(await plain(confined('assistant.sqlite3', dataDirectory)), { readOnly: true });
  try { audit.providerRequests = ai.prepare('SELECT count(*) AS n FROM requests').get().n; audit.vaultEntries = ai.prepare('SELECT count(*) AS n FROM vault').get().n; }
  finally { ai.close(); }
  return audit;
}
function assertNoEffects(audit) {
  assert.equal(audit.schema, 8);
  for (const field of ['attempts', 'leases', 'captures', 'closures', 'providerRequests', 'vaultEntries']) assert.equal(audit[field], 0, `Unexpected ${field}.`);
  for (const field of ['executions', 'findings', 'validations']) assert.deepEqual(audit.state[field], [], `Unexpected ${field}.`);
  assert.equal(targetConnections.length, 0, 'A connection reached the reserved target.');
}
async function metrics() {
  return page.evaluate(() => {
    const element = document.activeElement;
    const text = node => node?.textContent?.replace(/\s+/g, ' ').trim().slice(0, 180) ?? '';
    return { heading: text(document.querySelector('main h1')), focus: element?.getAttribute('aria-label') ?? text(element), tag: element?.tagName ?? '', role: element?.getAttribute('role') ?? '', width: innerWidth, scrollWidth: document.documentElement.scrollWidth };
  });
}
async function waitForNativeReady() {
  // CDP can expose the initial about:blank page before WebView2 navigates.
  // Wait only for the required host; a dev server or blank page is never accepted.
  await page.waitForURL(url => url.hostname === 'tauri.localhost', { timeout: 45000, waitUntil: 'domcontentloaded' });
  await page.waitForFunction(() => location.hostname === 'tauri.localhost' && typeof window.__TAURI_INTERNALS__?.invoke === 'function', null, { timeout: 15000 });
  assert.equal(new URL(page.url()).hostname, 'tauri.localhost', 'Use the custom-protocol binary, not a dev server.');
  await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
  assert.equal(new URL(page.url()).hostname, 'tauri.localhost', 'The ready app must remain on its custom-protocol host.');
}
async function optionalControl(path) {
  try { await plain(path); return JSON.parse(await readFile(path, 'utf8')); }
  catch (error) { if (error.code === 'ENOENT') return null; throw error; }
}
const onSignal = () => { signalRequested = true; };
process.on('SIGINT', onSignal); process.on('SIGTERM', onSignal);
try {
  await assertNoForeignApplication();
  const productVersion = (await powershell('(Get-Item -LiteralPath $env:OPENKODA_FIXTURE_BINARY).VersionInfo.ProductVersion', { OPENKODA_FIXTURE_BINARY: executable })).trim();
  assert.match(productVersion, /^0\.1\.6(?:\.0)?$/, 'Native version metadata must be 0.1.6.');
  receipt.artifact.productVersion = productVersion;
  targetListener = createServer(socket => { targetConnections.push({ at: new Date().toISOString() }); socket.destroy(); });
  const targetPort = await listen(targetListener), target = `http://127.0.0.1:${targetPort}/never-dispatched`;
  const reservation = createServer(), port = await listen(reservation);
  await new Promise((ok, bad) => reservation.close(error => error ? bad(error) : ok()));
  assert.equal(hash(await readFile(executable)), expectedSha256, 'Binary changed before spawn.');
  child = spawn(executable, [], { cwd: dirname(executable), windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: {
    ...process.env, OPENKODA_DATA_DIR: dataDirectory, WEBVIEW2_USER_DATA_FOLDER: join(directory, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`,
  } });
  receipt.ownedPid = child.pid;
  let startupError;
  child.on('error', error => { startupError = error; }); child.once('exit', () => { observedExit = true; });
  child.stdout.on('data', bytes => diagnostics.push(`stdout: ${bytes}`)); child.stderr.on('data', bytes => diagnostics.push(`stderr: ${bytes}`));
  await expect.poll(async () => {
    if (startupError) throw startupError;
    if (observedExit || child.exitCode !== null || child.signalCode !== null) throw new Error('Owned app exited before walkthrough.');
    try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  }, { timeout: 45000 }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context = browser.contexts()[0];
  await expect.poll(() => context.pages().length, { timeout: 15000 }).toBeGreaterThan(0);
  page = context.pages()[0]; page.setDefaultTimeout(15000);
  page.on('pageerror', error => pageErrors.push(error.message));
  page.on('framenavigated', frame => { if (frame === page.mainFrame()) diagnostics.push(`main navigation: ${frame.url()}`); });
  await waitForNativeReady();
  const initial = await request('snapshot');
  assert.deepEqual(initial.projects, []); assert.deepEqual(initial.assessments, []); assert.deepEqual(initial.plans, []);
  assert.equal((await request('ai.status')).hasApiKey, false);
  assertNoEffects(await storageAudit());
  const project = await request('project.create', { name: 'Leitura de tela — fluxo 0.1.6', kind: 'web', target, description: 'Perfil sintético exclusivo. Nenhuma operação será enfileirada.' });
  const assessment = await reviewedAssessment(request, { projectId: project.id, title: 'Fila e revisão humana — ensaio NVDA', kind: 'web', target, objective: 'Ler os controles de fila com entradas sintéticas e sem executar operações.' }, ['http.headers.v1']);
  assessmentId = assessment.id;
  const syntheticText = 'Registro sintético criado para observar nomes, estados e foco dos controles. Nenhum alvo foi consultado. Isto não é uma vulnerabilidade nem uma evidência de cobertura.';
  const imported = await request('evidence.import', { assessmentId, name: 'Texto sintético para revisão humana', sourceDescription: 'Literal criado pelo harness; origem manual não verificada.', content: syntheticText, acknowledgeUnverifiedSource: true });
  const evidence = (await request('snapshot')).assessments.find(item => item.id === assessmentId).evidence.find(item => item.id === imported.id);
  assert.ok(evidence, 'The snapshot must contain the identity returned by evidence.import.');
  // evidence.import preserves a serialized provenance wrapper, not the bare input
  // text. Verify actual stored bytes read-only; never rebuild its serialization.
  const importedAudit = await storageAudit(); assertNoEffects(importedAudit);
  const storedEvidence = importedAudit.state.assessments.find(item => item.id === assessmentId)?.evidence.find(item => item.id === imported.id);
  assert.ok(storedEvidence, 'The imported identity must exist in this owned profile.');
  assert.equal(storedEvidence.sha256, hash(storedEvidence.content), 'Stored evidence bytes must match their canonical hash.');
  assert.equal(evidence.rawSha256, storedEvidence.sha256, 'The public evidence identity must match its canonical stored bytes.');
  assert.equal(evidence.rawBytes, Buffer.byteLength(storedEvidence.content), 'Public evidence length must describe the canonical wrapper.');
  const wrapper = JSON.parse(storedEvidence.content);
  assert.equal(wrapper.schema, 'openkoda.manual-evidence.v1');
  assert.equal(wrapper.origin, 'user_supplied'); assert.equal(wrapper.sourceVerified, false);
  assert.equal(wrapper.sourceDescription, 'Literal criado pelo harness; origem manual não verificada.');
  assert.equal(wrapper.text, syntheticText, 'The wrapper must preserve the synthetic input literally.');
  receipt.syntheticEvidence = { id: evidence.id, name: evidence.name, rawSha256: evidence.rawSha256, rawBytes: evidence.rawBytes, schema: wrapper.schema, sourceVerified: wrapper.sourceVerified, inputTextSha256: hash(syntheticText) };
  const preview = await request('work.plan.prepare', { assessmentId, expectedHead: null, expectedGeneration: 0, goal: 'Observar preparação e leitura de revisões sem confirmar efeitos.', nodes: [
    { key: 'native', existingNodeId: null, purpose: 'Revisar operação nativa sem executar', expectedObservation: 'Plano concreto visível; nenhuma requisição HTTP.', dependsOnKeys: [], intent: { kind: 'native', adapter: 'http.headers.v1', targetRef: null, ports: null } },
    { key: 'human', existingNodeId: null, purpose: 'Revisar texto sintético sem registrar decisão', expectedObservation: 'Campos e referência da evidência legíveis; nenhuma decisão persistida.', dependsOnKeys: [], intent: { kind: 'human_review', inputs: [{ kind: 'evidence', evidenceId: evidence.id, expectedRawSha256: evidence.rawSha256 }] } },
    { key: 'dependent', existingNodeId: null, purpose: 'Dependência aguardando a revisão humana', expectedObservation: 'Permanece bloqueada enquanto a revisão não é registrada.', dependsOnKeys: ['human'], intent: { kind: 'native', adapter: 'http.headers.v1', targetRef: null, ports: null } },
  ], limits: { maxAttemptsTotal: 3, maxCaptureBytesTotal: 65536 } });
  await request('work.plan.commit', { review: preview.review, idempotencyKey: randomUUID(), acknowledgePlan: true });
  const getPlan = async () => (await request('work.plan.get', { assessmentId, revisionId: null })).plan;
  let plan = await getPlan();
  const nodes = Object.fromEntries(plan.revision.body.nodes.map(node => [node.intent.kind === 'human_review' ? 'human' : node.dependsOn.length ? 'dependent' : 'native', node]));
  const preparations = {};
  for (const kind of ['native', 'human']) {
    plan = await getPlan();
    const job = plan.jobs.find(item => item.nodeId === nodes[kind].id);
    assert.equal(job.canPrepare, true);
    const result = await request('work.operation.prepare', { job: { id: job.id, generation: job.generation }, revision: plan.head, idempotencyKey: randomUUID() });
    assert.equal(result.value.kind, kind === 'human' ? 'human_review' : 'native');
    if (kind === 'human') { assert.equal(result.value.canRecord, true); assert.deepEqual(result.value.resolvedInputs, [{ kind: 'evidence', id: evidence.id, rawSha256: evidence.rawSha256 }]); }
    preparations[kind] = { jobId: result.value.job.id, expiresAt: kind === 'native' ? result.value.compiled.expiresAt : result.value.review.expiresAt };
  }
  plan = await getPlan();
  assert.equal(plan.jobs.find(item => item.nodeId === nodes.dependent.id).state, 'waiting_dependencies');
  assert.deepEqual((await request('capture.list', { assessmentId, attemptId: null })).captures, []);
  const closure = await request('work.aiLocalClosure.list', {});
  assert.deepEqual(closure.items, []); assert.deepEqual(closure.candidates, []);
  await page.reload();
  await waitForNativeReady();
  baseline = await storageAudit(); assertNoEffects(baseline);
  await writeFile(join(directory, 'baseline.json'), JSON.stringify(baseline, null, 2), { flag: 'wx' });
  const ready = { schema: 'openkoda.screen-reader-workflow-ready.v1', directory, pid: child.pid, port, version: '0.1.6', binarySha256: expectedSha256, projectId: project.id, assessmentId, assessmentTitle: assessment.title, preparations, finishPath, stopPath, scenarioIds,
    deadline: new Date(Date.now() + 25 * 60 * 1000).toISOString(),
    navigation: ['Avaliações', assessment.title, 'Plano e fila', 'Fila e tentativas'],
    controls: { queue: ['Atualizar fila', 'Pausar novos despachos', 'Cancelar plano'], native: ['Nó: Revisar operação nativa sem executar', 'Reabrir revisão do nó', 'Revisão da operação do nó', 'Fechar revisão do nó'], human: ['Nó: Revisar texto sintético sem registrar decisão', 'Reabrir revisão do nó', 'Decisão da revisão humana', 'Nota da revisão humana', 'Fechar revisão do nó'], closure: ['Encerramento local de IA', 'Consultar encerramentos locais'], forbidden: ['Autorizar e enfileirar esta tentativa', 'Registrar revisão humana', 'Confirmar pausa', 'Confirmar pedido de cancelamento', 'Salvar revisão do plano', 'Confirmar encerramento local'] },
    baseline: await metrics(), captureUi: 'Not rendered: no attempt or capture exists. Empty list was verified through public IPC.' };
  await writeFile(join(directory, 'ready.json'), JSON.stringify(ready, null, 2), { flag: 'wx' });
  await writeFile(join(root, '.local', 'screen-reader-workflow-current.json'), JSON.stringify(ready, null, 2));
  console.log(JSON.stringify({ ready: true, ...ready }));
  let finish;
  while (Date.now() < Date.parse(ready.deadline)) {
    if (signalRequested || await optionalControl(stopPath)) throw new Error('Coordinator stopped the owned walkthrough. Artifacts retained.');
    if (observedExit || child.exitCode !== null || child.signalCode !== null) throw new Error('Owned app exited during walkthrough.');
    assert.equal(targetConnections.length, 0, 'Unexpected connection to the reserved target.');
    const sample = await metrics(), previous = receipt.observations.at(-1);
    if (!previous || previous.heading !== sample.heading || previous.focus !== sample.focus || previous.tag !== sample.tag) receipt.observations.push({ ...sample, at: new Date().toISOString() });
    finish = await optionalControl(finishPath); if (finish) break;
    await new Promise(ok => setTimeout(ok, 500));
  }
  assert.ok(finish, 'No manual receipt received within 25 minutes.');
  assert.equal(finish.schema, 'openkoda.screen-reader-workflow-review.v1');
  assert.equal(finish.readerStarted, true); assert.match(finish.reader, /^NVDA /);
  assert.ok(Array.isArray(finish.observations) && finish.observations.length > 0);
  for (const observation of finish.observations) {
    assert.ok(scenarioIds.includes(observation.id));
    assert.ok(['passed', 'failed', 'not_tested'].includes(observation.status));
    assert.ok(typeof observation.evidence === 'string' && observation.evidence.trim());
  }
  receipt.review = finish;
  const after = await storageAudit(); receipt.after = after; assertNoEffects(after);
  assert.equal(after.coreSha256, baseline.coreSha256, 'Core domain changed during the walkthrough.');
  assert.deepEqual(after.heads, baseline.heads); assert.deepEqual(after.revisions, baseline.revisions);
  for (const command of baseline.commands) assert.deepEqual(after.commands.find(item => item.id === command.id), command);
  const added = after.commands.filter(item => !baseline.commands.some(previous => previous.id === item.id));
  assert.ok(added.every(item => item.action === 'work.operation.prepare'), 'Only operation preparation may append metadata commands.');
  assert.equal(after.jobs.length, baseline.jobs.length);
  for (const job of after.jobs) {
    const before = baseline.jobs.find(item => item.id === job.id); assert.ok(before);
    if (job.nodeId !== nodes.human.id) assert.deepEqual(job, before, 'Native/dependent job must remain unchanged.');
    else {
      assert.equal(job.state, 'awaiting_approval'); assert.equal(job.currentAttempt, before.currentAttempt);
      assert.deepEqual(job.results, []); assert.deepEqual(job.captureIds, []);
      assert.equal(job.compiled?.kind, 'human_review'); assert.deepEqual(job.revision, before.revision);
    }
  }
  const oldEvents = new Set(baseline.events.map(event => event.id));
  for (const event of baseline.events) assert.deepEqual(after.events.find(item => item.id === event.id), event);
  assert.ok(after.events.filter(event => !oldEvents.has(event.id)).every(event => event.type === 'job_state_changed' && event.jobId === preparations.human.jobId && ['preparing', 'awaiting_approval'].includes(event.state)), 'Only human-preparation events are expected.');
  assert.equal((await request('ai.status')).hasApiKey, false);
  assert.deepEqual((await request('capture.list', { assessmentId, attemptId: null })).captures, []);
  const finalClosure = await request('work.aiLocalClosure.list', {}); assert.deepEqual(finalClosure.items, []); assert.deepEqual(finalClosure.candidates, []);
  assert.deepEqual(pageErrors, []);
  receipt.corePreserved = true; receipt.noEffects = true; receipt.addedPreparationCommands = added.length;
  receipt.passed = finish.passed === true && scenarioIds.every(id => finish.observations.some(item => item.id === id && item.status === 'passed')) && finish.observations.every(item => item.status === 'passed');
  if (!receipt.passed) process.exitCode = 1;
} catch (error) { receipt.error = String(error); receipt.stack = error.stack; process.exitCode = 1; }
finally {
  await browser?.close().catch(error => diagnostics.push(String(error)));
  if (child && !observedExit && child.exitCode === null && child.signalCode === null) {
    child.kill();
    await new Promise(ok => { const timer = setTimeout(ok, 5000); child.once('exit', () => { clearTimeout(timer); ok(); }); });
  }
  receipt.ownedProcessClosed = !child || observedExit || child.exitCode !== null || child.signalCode !== null;
  if (!receipt.ownedProcessClosed) { receipt.passed = false; receipt.cleanupError = 'Owned process did not confirm exit.'; process.exitCode = 1; }
  if (targetListener?.listening) await new Promise(ok => targetListener.close(ok));
  if (baseline) {
    try {
      const finalStorage = await storageAudit(); assertNoEffects(finalStorage);
      assert.equal(finalStorage.coreSha256, baseline.coreSha256, 'Core domain changed before shutdown.');
      receipt.finalStorage = { schema: finalStorage.schema, coreSha256: finalStorage.coreSha256, attempts: finalStorage.attempts, leases: finalStorage.leases, captures: finalStorage.captures, closures: finalStorage.closures, providerRequests: finalStorage.providerRequests, vaultEntries: finalStorage.vaultEntries };
    } catch (error) { receipt.passed = false; receipt.finalPreservationError = String(error); process.exitCode = 1; }
  }
  process.removeListener('SIGINT', onSignal); process.removeListener('SIGTERM', onSignal);
  receipt.generatedAt = new Date().toISOString(); receipt.targetConnections = targetConnections; receipt.pageErrors = pageErrors;
  await writeFile(join(directory, 'ipc-log.json'), JSON.stringify(ipc, null, 2));
  await writeFile(join(directory, 'diagnostics.log'), diagnostics.join('\n'));
  await writeFile(join(directory, 'result.json'), JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify({ passed: receipt.passed, error: receipt.error, directory, ownedProcessClosed: receipt.ownedProcessClosed }));
}
