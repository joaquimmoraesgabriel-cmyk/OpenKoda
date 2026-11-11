// Native integration, on owned data only. No provider credential, provider
// request, WSL invocation, arbitrary process adapter or real profile is used.
// The preserved 0.1.3 app creates the V1 migration fixture through public IPC.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer as httpServer } from 'node:http';
import { createServer as tcpServer } from 'node:net';
import { mkdir, readFile, writeFile, realpath } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { reviewedAssessment, unboundedScopeWindow } from './scope-fixture.mjs';
import { authorizeData, reviewedReport, reviewedBackup, reviewedEvidenceText } from './data-fixture.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const { version } = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--expected-sha256' || !/^[a-f0-9]{64}$/.test(args[1])) throw new Error('Use --expected-sha256 <reviewed final debug hash>.');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const binary = join(root, 'src-tauri', 'target', 'debug', 'openkoda.exe');
const binaryBytes = await readFile(binary);
if (await realpath(binary) !== binary || hash(binaryBytes) !== args[1]) throw new Error('Final debug identity differs from the reviewed artifact.');
const oldManifestPath = join(root, 'releases', '0.1.3-local-20261108', 'release.json');
const oldManifest = JSON.parse((await readFile(oldManifestPath, 'utf8')).replace(/^\uFEFF/, ''));
const oldBinary = join(root, 'releases', '0.1.3-local-20261108', 'openkoda-debug.exe');
if (oldManifest.version !== '0.1.3' || await realpath(oldBinary) !== oldBinary || hash(await readFile(oldBinary)) !== oldManifest.nativeTestBinarySha256) throw new Error('Preserved 0.1.3 debug identity differs.');

const directory = join(root, '.local', `native-combined-${randomUUID()}`), dataDirectory = join(directory, 'data');
const primaryDirectory = join(directory, 'primary-source'), secondaryDirectory = join(directory, 'secondary-source');
await mkdir(join(secondaryDirectory, 'private'), { recursive: true });
await mkdir(primaryDirectory);
if (await realpath(directory) !== directory) throw new Error('Fixture path must not redirect.');
const sources = new Map([
  [join(primaryDirectory, 'primary.txt'), 'PRIMARY_CONTENT_NOT_SELECTED_FOR_AI\n'],
  [join(secondaryDirectory, 'selected.ts'), 'export const selectedNumber = 7; // SECONDARY_SELECTED_CONTENT\n'],
  [join(secondaryDirectory, 'excluded.txt'), 'EXCLUDED_FILE_CONTENT_MUST_STAY_LOCAL\n'],
  [join(secondaryDirectory, 'private', 'nested.txt'), 'EXCLUDED_TREE_CONTENT_MUST_STAY_LOCAL\n'],
]);
for (const [path, content] of sources) await writeFile(path, content, { flag: 'wx' });
function owned(value, base = directory) {
  const ordinary = value.startsWith('\\\\?\\') && /^[a-z]:\\/i.test(value.slice(4)) ? value.slice(4) : value;
  const path = resolve(ordinary), part = relative(base, path);
  if (!part || isAbsolute(part) || part === '..' || part.startsWith(`..${sep}`)) throw new Error('Artifact escaped the owned fixture.');
  return path;
}
const result = { passed: false, directory, artifact: { version, binary, sha256: hash(binaryBytes), bytes: binaryBytes.length },
  legacyArtifact: { version: oldManifest.version, binary: oldBinary, sha256: oldManifest.nativeTestBinarySha256, manifestSha256: hash(await readFile(oldManifestPath)) }, records: [],
  limits: [
    'One native Windows fixture and four owned loopback target listeners. CDP is an engineering test harness, not an assumed attacker capability.',
    'V1 migration history is created by the preserved 0.1.3 public API. SQLite access in this script is read-only; no grants, scopes or proofs are injected.',
    'Source exclusions are checked with exclusively locked file sentinels, exact manifests, omissions and rejected capture paths. This does not claim an OS-wide audit of every filesystem metadata access.',
    'HTTP/TCP denials use live owned listeners and assert zero accepted sockets throughout the run. Positive TCP sends no application bytes.',
    'Source capture and reviewed report preview remain local; AI previews without a credential are refused. Provider requests and vault rows must stay zero. No paid API or Linux process is requested.',
    'Scope revocation blocks new admission but does not claim to cancel previously dispatched effects. Backup restoration creates a separate copy and never activates it.',
    'Restart is application restart, not Windows reboot, crash recovery or power-loss validation. Screen-reader and visual walkthrough are separate receipts.',
  ] };
const diagnostics = [], ipc = [], denials = [], httpRequests = [], targetListeners = [], sockets = new Set();
const lockedSources = new Map();
const traffic = { allowedHttpSockets: 0, deniedHttpSockets: 0, allowedTcpSockets: 0, deniedTcpSockets: 0, tcpClosed: 0, tcpBytes: 0 };
let child, browser, page, failure, boot = 0, project, catalog, selection, assessmentId, status, sourcePlan, sourceExecution, captured, manual, derivedId, legacy, finalHash;
let allowedHttp, deniedHttp, allowedTcp, deniedTcp;
const fingerprint = (base = dataDirectory) => {
  const core = new DatabaseSync(owned(join(base, 'openkoda.sqlite3')), { readOnly: true });
  const ai = new DatabaseSync(owned(join(base, 'assistant.sqlite3')), { readOnly: true });
  try {
    const raw = core.prepare('SELECT json FROM app_state WHERE id=1').get().json;
    return { sha256: hash(raw), state: JSON.parse(raw), version: core.prepare('PRAGMA user_version').get().user_version,
      requests: ai.prepare('SELECT count(*) AS count FROM requests').get().count, vault: ai.prepare('SELECT count(*) AS count FROM vault').get().count };
  } finally { core.close(); ai.close(); }
};
const snapshot = () => fingerprint().state;
const counters = () => JSON.stringify(traffic);
async function unchangedSources() { for (const [path, content] of sources) if (!lockedSources.has(path)) expect(await readFile(path, 'utf8'), path).toBe(content); }
async function assertNoForeignApplications() {
  // Fixed read-only process inspection. A pre-existing app is never terminated.
  const command = "$entries=@(Get-Process -Name 'openkoda','openkoda-debug' -ErrorAction SilentlyContinue | Select-Object Id,Path); ConvertTo-Json -InputObject $entries -Compress; exit 0";
  const output = await new Promise((ok, bad) => {
    const process = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], signal: AbortSignal.timeout(15000) });
    let out = '', err = ''; process.on('error', bad);
    process.stdout.on('data', bytes => { out += bytes; if (out.length > 32768) process.kill(); }); process.stderr.on('data', bytes => { err += bytes; });
    process.once('exit', (code, signal) => code === 0 && !signal ? ok(out) : bad(new Error(`Application inventory failed: ${code ?? signal}; ${err}`)));
  });
  const values = output.trim() ? JSON.parse(output) : []; const entries = Array.isArray(values) ? values : [values];
  const allowed = child && child.exitCode === null && child.signalCode === null ? [child.pid] : [];
  if (entries.some(entry => !allowed.includes(entry.Id))) throw new Error('A pre-existing or foreign OpenKoda process is running. Preserve it and close it explicitly before this fixture.');
}
async function lockExcludedSources() {
  const helper = join(directory, 'lock-owned-sentinel.ps1');
  await writeFile(helper, "param([string]$Target)\n$ErrorActionPreference='Stop'\n$stream=[System.IO.File]::Open($Target,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::None)\ntry { [Console]::WriteLine('READY'); [Console]::Out.Flush(); [void][Console]::ReadLine() } finally { $stream.Dispose() }\n", { flag: 'wx' });
  for (const path of [join(secondaryDirectory, 'excluded.txt'), join(secondaryDirectory, 'private', 'nested.txt')]) {
    expect(await readFile(path, 'utf8')).toBe(sources.get(path));
    const locker = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-File', helper, '-Target', path], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
    lockedSources.set(path, locker); let ready = false, error;
    locker.on('error', failure => { error = failure; });
    locker.stdout.on('data', data => { if (String(data).includes('READY')) ready = true; }); locker.stderr.on('data', data => diagnostics.push(`owned lock: ${data}`));
    await expect.poll(() => { if (error) throw error; if (locker.exitCode !== null || locker.signalCode !== null) throw new Error('Owned sentinel lock exited early.'); return ready; }, { timeout: 15000, message: 'Owned excluded sentinel is locked exclusively' }).toBe(true);
    await expect(readFile(path)).rejects.toThrow();
  }
  result.exclusiveSentinels = [...lockedSources.keys()];
}
async function releaseExcludedSources() {
  let firstError;
  for (const [path, locker] of lockedSources) {
    try {
      if (locker.exitCode === null && locker.signalCode === null) {
        locker.stdin.end('\n');
        try { await expect.poll(() => locker.exitCode !== null || locker.signalCode !== null, { timeout: 10000 }).toBe(true); }
        catch (error) { locker.kill(); await expect.poll(() => locker.exitCode !== null || locker.signalCode !== null, { timeout: 10000 }).toBe(true); throw error; }
      }
      expect(locker.exitCode, 'Owned sentinel helper must close normally').toBe(0);
      expect(await readFile(path, 'utf8')).toBe(sources.get(path));
    } catch (error) { firstError ??= error; }
    finally {
      if (locker.exitCode !== null || locker.signalCode !== null) lockedSources.delete(path);
    }
  }
  if (firstError) throw firstError;
}
async function listen(server) {
  await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  targetListeners.push(server); return server.address().port;
}
async function unusedPort() {
  const server = tcpServer();
  await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  const port = server.address().port;
  await new Promise((ok, bad) => server.close(error => error ? bad(error) : ok())); return port;
}
async function openApp(executable) {
  if (child && child.exitCode === null && child.signalCode === null) throw new Error('Close the previous owned app before another boot.');
  await assertNoForeignApplications();
  boot++; const port = await unusedPort();
  child = spawn(executable, [], { cwd: dirname(executable), windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: {
    ...process.env, OPENKODA_DATA_DIR: dataDirectory, WEBVIEW2_USER_DATA_FOLDER: join(directory, `webview-${boot}`),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`,
  } });
  result.ownedPids ??= []; result.ownedPids.push(child.pid);
  let startupError; child.on('error', error => { startupError = error; });
  child.stdout.on('data', data => diagnostics.push(`boot ${boot} stdout: ${data}`)); child.stderr.on('data', data => diagnostics.push(`boot ${boot} stderr: ${data}`));
  await expect.poll(async () => {
    if (startupError) throw startupError;
    if (child.exitCode !== null || child.signalCode !== null) throw new Error(`Owned app exited before readiness: ${child.exitCode ?? child.signalCode}`);
    try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  }, { timeout: 45000, message: 'Owned native WebView CDP ready' }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context = browser.contexts()[0]; await expect.poll(() => context.pages().length, { timeout: 15000 }).toBeGreaterThan(0);
  page = context.pages()[0]; page.setDefaultTimeout(15000);
  page.on('console', item => diagnostics.push(`console ${item.type()}: ${item.text()}`)); page.on('pageerror', error => diagnostics.push(`pageerror: ${error.message}`));
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
  expect(new URL(page.url()).hostname).toBe('tauri.localhost');
  await assertNoForeignApplications();
}
async function closeApp() {
  if (browser) {
    await browser.contexts()[0]?.tracing.stop({ path: join(directory, `trace-${boot}.zip`) }).catch(error => diagnostics.push(`trace: ${error}`));
    await browser.close().catch(error => diagnostics.push(`CDP close: ${error}`)); browser = undefined; page = undefined;
  }
  if (child && child.exitCode === null && child.signalCode === null) {
    child.kill(); await expect.poll(() => child.exitCode !== null || child.signalCode !== null, { timeout: 10000 }).toBe(true);
  }
}
async function request(action, payload = {}) {
  const item = { action, at: new Date().toISOString(), trafficBefore: { ...traffic } }; ipc.push(item);
  try {
    const value = await page.evaluate(async ({ action, payload }) => {
      let timer;
      try { return await Promise.race([window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }), new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('COMBINED_IPC_DEADLINE')), 30000); })]); }
      finally { clearTimeout(timer); }
    }, { action, payload }); item.status = 'fulfilled'; return value;
  } catch (error) { item.status = 'rejected'; item.error = String(error); throw error; }
  finally { item.trafficAfter = { ...traffic }; }
}
async function pure(task) {
  const before = fingerprint(), hits = counters(); const value = await task();
  expect(fingerprint().sha256).toBe(before.sha256); expect(counters()).toBe(hits); return value;
}
async function denied(action, payload, reason) {
  const before = fingerprint(), hits = counters(); let error;
  try { await request(action, payload); } catch (caught) { error = String(caught); }
  expect(error, `${action} must reject`).toBeTruthy(); expect(error).not.toMatch(/COMBINED_IPC_DEADLINE|Target closed|Page crashed|Execution context was destroyed/);
  if (reason) expect(error).toContain(reason);
  expect(fingerprint().sha256).toBe(before.sha256); expect(counters()).toBe(hits); await unchangedSources();
  denials.push({ action, error }); return error;
}
async function gate(id, task) {
  const entry = { id, status: 'running', startedAt: new Date().toISOString() }; result.records.push(entry);
  try { entry.detail = await task(); entry.status = 'passed'; } catch (error) { entry.status = 'failed'; entry.error = String(error); throw error; }
  finally { entry.endedAt = new Date().toISOString(); }
}
const scopeStatus = id => request('assessment.scopeStatus', { assessmentId: id });
const refreshCatalog = async () => catalog = await request('project.targetOptions', { projectId: project.id });
const scopeRef = value => ({ assessmentId: value.assessmentId, revisionId: value.revision.id, digest: value.revision.digest });
const option = (value, targetId) => { const found = value.targetOptions.find(value => value.entry.projectTargetId === targetId); expect(found).toBeTruthy(); return found; };
const targetPlanInput = (value, targetId, adapter, extra = {}) => ({ assessmentId: value.assessmentId, scopeRef: scopeRef(value), targetRef: option(value, targetId).targetRef, adapter, ...extra });
async function add(label, kind, target) {
  await refreshCatalog(); const input = { projectId: project.id, expectedCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), label, kind, target };
  const result = await request('project.targetAdd', input); await refreshCatalog(); return { id: result.targetId, input, result };
}
async function run(plan) {
  const decision = { id: plan.id, digest: plan.digest }; await denied('plan.run', decision);
  const expectedTcpClosed = traffic.tcpClosed + (plan.adapter === 'network.tcp-connect.v1' ? plan.resolvedAddresses.length : 0);
  await request('plan.approve', decision); const execution = await request('plan.run', decision); expect(execution.status).toBe('completed');
  if (plan.adapter === 'network.tcp-connect.v1') await expect.poll(() => traffic.tcpClosed, { timeout: 10000, message: 'Owned TCP peer observed connection closure before replay check' }).toBe(expectedTcpClosed);
  await denied('plan.run', decision); return execution;
}
const evidence = id => { const item = snapshot().assessments.find(a => a.id === assessmentId).evidence.find(e => e.id === id); expect(item).toBeTruthy(); expect(hash(item.content)).toBe(item.sha256); return item; };
const evidenceDto = async id => {
  const state = await request('snapshot'); const item = state.assessments.find(a => a.id === assessmentId).evidence.find(e => e.id === id);
  expect(state.schema).toBe('openkoda.snapshot-view.v1'); expect(item).toBeTruthy(); expect(item).not.toHaveProperty('content'); expect(item).not.toHaveProperty('sha256');
  expect(item.rawSha256).toBe(evidence(id).sha256); return item;
};
const reviewedText = async id => { const item = await evidenceDto(id); return reviewedEvidenceText(request, assessmentId, id, item.rawSha256); };
const aiPreviewInput = (evidenceId, contextTargetRefs) => ({ assessmentId, evidenceIds: [evidenceId], contextTargetRefs, operation: 'ask', message: 'Review only this owned fixture.', messageSensitivity: 'public', packageSensitivity: 'public', categories: ['human_text', 'target_metadata', 'source_excerpt'], credentialReferenceId: randomUUID() });
const localMethods = ['local.inventory.v1', 'source.rules.v1', 'source.capture.v1', 'ai.remote-review.v1'];
let secondary, webAllowed, webDenied, network, unused;
try {
  allowedHttp = httpServer((incoming, response) => {
    const record = { method: incoming.method, url: incoming.url, bytes: 0 }; httpRequests.push(record); incoming.on('data', data => { record.bytes += data.length; });
    response.writeHead(incoming.method === 'HEAD' && incoming.url === '/selected' ? 200 : 405, { 'Content-Length': '0', 'X-Content-Type-Options': 'nosniff', 'Connection': 'close' }); response.end();
  });
  allowedHttp.on('connection', socket => { traffic.allowedHttpSockets++; sockets.add(socket); socket.on('close', () => sockets.delete(socket)); });
  deniedHttp = httpServer((_, response) => { response.writeHead(500); response.end(); });
  deniedHttp.on('connection', socket => { traffic.deniedHttpSockets++; sockets.add(socket); socket.on('close', () => sockets.delete(socket)); });
  allowedTcp = tcpServer(socket => { traffic.allowedTcpSockets++; sockets.add(socket); socket.on('data', data => { traffic.tcpBytes += data.length; }); socket.on('error', error => diagnostics.push(`owned TCP: ${error.message}`)); socket.on('close', () => { traffic.tcpClosed++; sockets.delete(socket); }); socket.resume(); });
  deniedTcp = tcpServer(socket => { traffic.deniedTcpSockets++; sockets.add(socket); socket.on('close', () => sockets.delete(socket)); socket.destroy(); });
  const ports = { http: await listen(allowedHttp), httpDenied: await listen(deniedHttp), tcp: await listen(allowedTcp), tcpDenied: await listen(deniedTcp) };
  result.listeners = ports;
  await openApp(oldBinary);
  await gate('v1-frozen-history-created-by-0.1.3', async () => {
    const p = await request('project.create', { name: 'Preserved V1 fixture', kind: 'files', target: primaryDirectory });
    const a = await reviewedAssessment(request, { projectId: p.id, title: 'V1 before schema five', kind: 'files', target: p.target, objective: 'Preserve V1 scope, inventory and evidence across the migration.' }, ['local.inventory.v1']);
    const plan = await request('assessment.plan', { id: a.id }); await request('plan.approve', { id: plan.id, digest: plan.digest });
    const execution = await request('plan.run', { id: plan.id, digest: plan.digest }); expect(execution.status).toBe('completed');
    const frozen = await scopeStatus(a.id); expect(frozen.phase).toBe('frozen'); expect(frozen.revision.body).not.toHaveProperty('targetSet');
    legacy = { id: a.id, scope: frozen, before: fingerprint() }; expect(legacy.before.version).toBe(4); expect(legacy.before.requests).toBe(0); expect(legacy.before.vault).toBe(0);
    await writeFile(join(directory, 'v1-before-migration.json'), JSON.stringify(legacy, null, 2));
    return { assessmentId: a.id, planDigest: plan.digest, evidenceId: execution.evidenceId, coreSha256: legacy.before.sha256, userVersion: 4 };
  });
  await closeApp(); await openApp(binary);
  await gate('schema-four-to-eight-preserves-json-and-proof', async () => {
    const after = fingerprint(); expect(after.version).toBe(8); expect(after.sha256).toBe(legacy.before.sha256); expect(after.state).toEqual(legacy.before.state);
    const now = await pure(() => scopeStatus(legacy.id)); expect(now.revision).toEqual(legacy.scope.revision); expect(now.frozen).toEqual(legacy.scope.frozen);
    await denied('report.bundle', { id: legacy.id, includeEvidence: true }, 'data_review_required');
    const report = await reviewedReport(request, { assessmentId: legacy.id, includeEvidence: true });
    const manifest = JSON.parse(await readFile(owned(report.manifestPath), 'utf8')); expect(manifest.scope.revisions).toEqual(legacy.before.state.scopeRevisions);
    expect(snapshot().plans).toEqual(legacy.before.state.plans); expect(snapshot().plans[0]).not.toHaveProperty('targetRef'); expect(manifest.evidence[0].rawSha256).toBe(legacy.before.state.assessments[0].evidence[0].sha256);
    expect(manifest.schema).toBe('openkoda.reviewed-report.v1'); expect(report.receiptRef).toBeTruthy();
    return { userVersion: [4, 8], exactJsonPreserved: true, frozenScopeUnchanged: true, explicitlyReviewedHistoricalReport: true };
  });
  await gate('catalog-review-stale-input-inactivation-and-replay', async () => {
    project = await request('project.create', { name: 'Owned combined sources', kind: 'files', target: primaryDirectory });
    const empty = await pure(() => refreshCatalog()); expect(empty.defaultSelection.entries).toEqual([]); expect(empty.targets).toHaveLength(1);
    secondary = await add('Secondary code source', 'code', secondaryDirectory);
    expect((await pure(() => request('project.targetAdd', secondary.input))).replayed).toBe(true);
    await denied('project.targetAdd', { ...secondary.input, label: 'Different replay' }, 'scope_idempotency_conflict');
    await denied('project.targetAdd', { ...secondary.input, idempotencyKey: randomUUID(), label: 'Stale catalog', target: join(directory, 'never-opened') }, 'scope_catalog_changed');
    webAllowed = await add('Selected HTTP', 'web', `http://127.0.0.1:${ports.http}/selected`);
    webDenied = await add('Excluded HTTP peer', 'web', `http://127.0.0.1:${ports.httpDenied}/must-not-contact`);
    network = await add('Bounded TCP', 'network', '127.0.0.1');
    unused = await add('CATALOG_UNSELECTED_MARKER', 'web', 'http://127.0.0.1:1/catalog-unselected-marker');
    await request('project.targetUpdate', { projectId: project.id, targetId: unused.id, expectedCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), inactive: true }); await refreshCatalog();
    expect(catalog.targets.find(t => t.id === unused.id).inactive).toBe(true);
    selection = { primaryProjectTargetId: 'primary', entries: [{ projectTargetId: 'primary', methods: ['local.inventory.v1', 'ai.remote-review.v1'] }, { projectTargetId: secondary.id, methods: localMethods }, { projectTargetId: webAllowed.id, methods: ['http.headers.v1', 'ai.remote-review.v1'] }, { projectTargetId: webDenied.id, methods: ['http.headers.v1'] }, { projectTargetId: network.id, methods: ['network.tcp-connect.v1'] }], commonMethods: [], excludedProjectTargets: [],
      localExclusions: [{ projectTargetId: secondary.id, relativePath: 'excluded.txt', match: 'exact', reason: 'Synthetic excluded file.' }, { projectTargetId: secondary.id, relativePath: 'private', match: 'subtree', reason: 'Synthetic excluded subtree.' }],
      connectionExclusions: [{ address: '127.0.0.1', ports: [ports.httpDenied, ports.tcpDenied], reason: 'Owned listeners must receive zero connections.' }] };
    const invalid = { projectId: project.id, title: 'Inactive selection denied', objective: 'No traffic', window: unboundedScopeWindow(), projectCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), selection: { ...selection, entries: [...selection.entries, { projectTargetId: unused.id, methods: ['http.headers.v1'] }] } };
    await denied('assessment.combinedPreview', invalid, 'scope_catalog_target_inactive');
    return { projectId: project.id, catalogEntries: catalog.targets.length, replayPreservedTargetId: secondary.id, inactiveNotSelectable: true, listenersUntouched: counters() };
  });
  await gate('combined-create-review-and-append-only-edit', async () => {
    const input = { projectId: project.id, title: 'Owned combined native evaluation', objective: 'Only explicitly selected observations, with no execution of source code.', window: unboundedScopeWindow(), projectCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), selection };
    const preview = await pure(() => request('assessment.combinedPreview', input)); expect(preview.canCommit).toBe(true); expect(preview.proposal.body.targetSet.entries).toHaveLength(5);
    const decision = { ...input, reviewDigest: preview.reviewDigest, acknowledgeScope: true };
    await denied('assessment.combinedCreate', { ...decision, acknowledgeScope: false });
    await denied('assessment.combinedCreate', { ...decision, reviewDigest: '0'.repeat(64) }, 'scope_review_changed');
    const created = await request('assessment.combinedCreate', decision); assessmentId = created.assessmentId; expect(assessmentId).toBe(preview.assessmentId); expect(created.scopeStatus.phase).toBe('draft');
    expect((await pure(() => request('assessment.combinedCreate', decision))).replayed).toBe(true);
    status = await scopeStatus(assessmentId); const original = structuredClone(status.revision);
    const edit = { assessmentId, expectedStateDigest: status.stateDigest, projectCatalogDigest: catalog.projectCatalogDigest, objective: 'Revised purpose: compare selected fixture sources while preserving all exclusions.', window: unboundedScopeWindow(), selection };
    const revised = await pure(() => request('assessment.combinedScopePreview', edit)); expect(revised.canCommit).toBe(true);
    const savedInput = { ...edit, reviewDigest: revised.reviewDigest, idempotencyKey: randomUUID(), acknowledgeScope: true };
    const saved = await request('assessment.combinedScopeSave', savedInput); expect(saved.scopeStatus.revision.number).toBe(2);
    expect(snapshot().scopeRevisions.find(r => r.id === original.id)).toEqual(original);
    expect((await pure(() => request('assessment.combinedScopeSave', savedInput))).replayed).toBe(true);
    await denied('assessment.combinedScopeSave', { ...savedInput, idempotencyKey: randomUUID() }, 'scope_state_changed');
    status = await scopeStatus(assessmentId);
    await denied('assessment.plan', { id: assessmentId });
    await denied('assessment.targetPlan', { ...targetPlanInput(status, secondary.id, 'source.rules.v1'), targetRef: { ...option(status, secondary.id).targetRef, entryDigest: '0'.repeat(64) } });
    await denied('assessment.targetPlan', targetPlanInput(status, 'primary', 'http.headers.v1'));
    return { assessmentId, revisions: [original.id, status.revision.id], explicitEntries: status.targetOptions.map(v => v.targetRef), noExecutions: true };
  });
  await gate('secondary-local-plan-excludes-file-and-subtree', async () => {
    await lockExcludedSources();
    sourcePlan = await request('assessment.targetPlan', targetPlanInput(status, secondary.id, 'source.rules.v1'));
    status = await scopeStatus(assessmentId); expect(status.phase).toBe('frozen');
    expect(sourcePlan.targetRef).toEqual(option(status, secondary.id).targetRef); expect(sourcePlan.target).toBe(secondaryDirectory); expect(sourcePlan.inputFiles.map(f => f.path)).toEqual(['selected.ts']);
    expect(sourcePlan.scopeOmissions.map(o => o.value).sort()).toEqual(['excluded.txt', 'private']); expect(sourcePlan.scopeOmissions.every(o => o.kind === 'local' && /^[a-f0-9]{64}$/.test(o.ruleDigest))).toBe(true);
    expect(sourcePlan.identityPreflightRefs).toEqual(expect.arrayContaining([option(status, 'primary').targetRef, option(status, secondary.id).targetRef])); expect(sourcePlan.identityPreflightRefs).toHaveLength(2);
    sourceExecution = await run(sourcePlan);
    const observedText = await reviewedText(sourceExecution.evidenceId); for (const value of ['PRIMARY_CONTENT_NOT_SELECTED_FOR_AI', 'EXCLUDED_FILE_CONTENT', 'EXCLUDED_TREE_CONTENT']) expect(observedText).not.toContain(value);
    const observation = JSON.parse(observedText); expect(observation.adapter).toBe('source.rules.v1'); expect(observation.manifestSha256).toBe(sourcePlan.sourceScope.manifestSha256);
    const inventory = await request('assessment.targetPlan', targetPlanInput(status, secondary.id, 'local.inventory.v1')); expect(inventory.inputFiles.map(f => f.path)).toEqual(['selected.ts']); await run(inventory);
    return { planId: sourcePlan.id, executionId: sourceExecution.id, targetRef: sourcePlan.targetRef, inputPaths: ['selected.ts'], omissions: sourcePlan.scopeOmissions, preflightRefs: sourcePlan.identityPreflightRefs, exclusiveSentinelsPreserved: true };
  });
  await gate('reviewed-capture-and-explicit-context-origins', async () => {
    const input = { assessmentId, planId: sourcePlan.id, paths: ['selected.ts'] };
    await denied('evidence.sourcePreview', { ...input, paths: ['excluded.txt'] }); await denied('evidence.sourcePreview', { ...input, paths: ['private/nested.txt'] });
    const sourceText = sources.get(join(secondaryDirectory, 'selected.ts'));
    // The existing conservative-lines-v1 redactor removes the terminal LF in
    // its derivative. Raw source bytes and the manifest hash must stay intact.
    const derivedText = 'export const selectedNumber = 7; // SECONDARY_SELECTED_CONTENT';
    const sourceFile = { path: 'selected.ts', bytes: Buffer.byteLength(sourceText), sourceSha256: hash(sourceText), redactedSha256: hash(derivedText), text: derivedText, redactionChanged: true };
    expect(sourcePlan.inputFiles).toEqual([expect.objectContaining({ path: sourceFile.path, size: sourceFile.bytes, sha256: sourceFile.sourceSha256 })]);
    const preview = await pure(() => request('evidence.sourcePreview', input));
    expect(preview).toMatchObject({ assessmentId, planId: sourcePlan.id, planDigest: sourcePlan.digest, manifestSha256: sourcePlan.sourceScope.manifestSha256, paths: ['selected.ts'], totalSourceBytes: sourceFile.bytes, sourceFilesIncluded: true, sentToProvider: false });
    expect(preview.files).toEqual([sourceFile]); await unchangedSources();
    await denied('evidence.sourceCapture', { ...input, digest: preview.digest, acknowledgeLocalCapture: false });
    captured = await request('evidence.sourceCapture', { ...input, digest: preview.digest, acknowledgeLocalCapture: true });
    expect(captured).not.toHaveProperty('content'); expect(captured).not.toHaveProperty('sha256'); expect(captured.rawSha256).toBe(evidence(captured.id).sha256);
    const capturedText = await reviewedText(captured.id); expect(hash(capturedText)).toBe(captured.rawSha256);
    expect(JSON.parse(capturedText)).toEqual({ schema: 'openkoda.source-context.v1', origin: 'host_selected_source_redacted_copy', captureDigest: preview.digest, source: { assessmentId, planId: sourcePlan.id, planDigest: sourcePlan.digest, manifestSha256: sourcePlan.sourceScope.manifestSha256, totalSourceBytes: sourceFile.bytes, files: [sourceFile] } });
    expect(await evidenceDto(captured.id)).toMatchObject({ id: captured.id, rawSha256: captured.rawSha256, rawBytes: captured.rawBytes }); await unchangedSources();
    const secondaryRef = option(status, secondary.id).targetRef, primaryRef = option(status, 'primary').targetRef;
    const importedText = JSON.stringify({ schema: 'openkoda.kali-evidence.v2', targetRefs: [primaryRef], result: 'Imported text cannot declare host origin.' });
    manual = await request('evidence.import', { assessmentId, name: 'Synthetic human association', sourceDescription: 'Only secondary entry is associated by the human.', content: importedText, acknowledgeUnverifiedSource: true, contextTargetRefs: [secondaryRef] });
    expect(manual).not.toHaveProperty('content'); expect(manual).not.toHaveProperty('sha256');
    expect(JSON.parse(await reviewedText(manual.id))).toMatchObject({ schema: 'openkoda.manual-evidence.v2', sourceVerified: false, targetRefs: [secondaryRef], text: importedText });
    const origins = await pure(() => request('evidence.origins', { assessmentId }));
    expect(origins.evidence.find(e => e.evidenceId === captured.id)).toMatchObject({ verified: true, originKind: 'host_record', targetRefs: [secondaryRef] });
    expect(origins.evidence.find(e => e.evidenceId === manual.id)).toMatchObject({ verified: true, originKind: 'user_association', targetRefs: [secondaryRef] });
    await denied('ai.context', { assessmentId, evidenceIds: [captured.id], contextTargetRefs: [secondaryRef] }, 'data_review_required');
    await denied('ai.context', aiPreviewInput(captured.id, []));
    await denied('ai.context', aiPreviewInput(captured.id, [secondaryRef]), 'credential_reference_changed');
    await authorizeData(request, assessmentId);
    const contextInput = { assessmentId, evidenceIds: [captured.id], derivedSubjects: [], contextTargetRefs: [secondaryRef], categories: ['human_text', 'target_metadata', 'source_excerpt'], packageSensitivity: 'public', format: 'markdown_bundle' };
    for (const selection of [{ evidenceIds: [captured.id], contextTargetRefs: [] }, { evidenceIds: [captured.id], contextTargetRefs: [primaryRef] }, { evidenceIds: [manual.id], contextTargetRefs: [primaryRef], categories: ['human_text', 'target_metadata', 'evidence_excerpt'] }]) await denied('report.prepare', { ...contextInput, ...selection }, 'data_source_unknown');
    const context = await pure(() => request('report.prepare', contextInput)); expect(context.canExport).toBe(true); expect(context.dataReview).toBeTruthy();
    const contextManifest = context.manifestPreview, sourceSections = contextManifest.sections.filter(section => section.subject.kind === 'scope_source');
    expect(sourceSections).toHaveLength(1); expect(sourceSections[0].value.target).toBe(secondaryDirectory); expect(sourceSections[0].subject.targetRef).toEqual(secondaryRef);
    expect(contextManifest.evidence).toEqual([expect.objectContaining({ id: captured.id, rawSha256: captured.rawSha256, targetRefs: [secondaryRef] })]);
    expect(JSON.stringify(contextManifest)).not.toContain(JSON.stringify(primaryDirectory).slice(1, -1));
    for (const value of ['PRIMARY_CONTENT_NOT_SELECTED_FOR_AI', 'CATALOG_UNSELECTED_MARKER', 'catalog-unselected-marker', 'EXCLUDED_FILE_CONTENT', 'EXCLUDED_TREE_CONTENT']) expect(JSON.stringify(contextManifest)).not.toContain(value);
    expect(contextManifest).not.toHaveProperty('coverage'); expect(contextManifest.assessment).not.toHaveProperty('target');
    await pure(() => request('data.review.cancel', { review: context.dataReview }));
    expect(fingerprint().requests).toBe(0); expect(fingerprint().vault).toBe(0);
    for (const path of lockedSources.keys()) await expect(readFile(path)).rejects.toThrow();
    await releaseExcludedSources(); await unchangedSources();
    return { capturedEvidenceId: captured.id, manualEvidenceId: manual.id, targetRef: secondaryRef, reviewedPackageDigest: context.dataReview.digest, previewDestination: 'report_export', aiWithoutCredentialDenied: true, sourceFilesIncluded: true, sourceBytes: sourceFile.bytes, sourceSha256: sourceFile.sourceSha256, derivedBytes: Buffer.byteLength(derivedText), derivedSha256: sourceFile.redactedSha256, redactionChanged: true, providerRequests: 0, vaultRows: 0 };
  });
  await gate('shared-connection-negatives-before-socket', async () => {
    await denied('assessment.targetPlan', targetPlanInput(status, webDenied.id, 'http.headers.v1'), 'scope_all_connections_excluded');
    expect(traffic.deniedHttpSockets).toBe(0);
    const http = await request('assessment.targetPlan', targetPlanInput(status, webAllowed.id, 'http.headers.v1')); expect(http.resolvedAddresses).toEqual([`127.0.0.1:${ports.http}`]);
    const httpExecution = await run(http); const observedHttp = JSON.parse(await reviewedText(httpExecution.evidenceId));
    expect(observedHttp).toMatchObject({ method: 'HEAD', target: webAllowed.result.catalog.targets.find(t => t.id === webAllowed.id).target, captureComplete: true, redirectFollowed: false });
    expect(httpRequests).toEqual([{ method: 'HEAD', url: '/selected', bytes: 0 }]);
    await denied('assessment.targetPlan', targetPlanInput(status, network.id, 'network.tcp-connect.v1', { ports: [ports.tcpDenied] }), 'scope_all_connections_excluded');
    const tcp = await request('assessment.targetPlan', targetPlanInput(status, network.id, 'network.tcp-connect.v1', { ports: [ports.tcp, ports.tcpDenied] }));
    expect(tcp.requestedPorts).toEqual([ports.tcp, ports.tcpDenied]); expect(tcp.resolvedAddresses).toEqual([`127.0.0.1:${ports.tcp}`]);
    expect(tcp.scopeOmissions).toEqual([expect.objectContaining({ kind: 'connection', value: `127.0.0.1:${ports.tcpDenied}` })]);
    const tcpExecution = await run(tcp); await expect.poll(() => traffic.tcpClosed, { timeout: 10000 }).toBe(1);
    const observedTcp = JSON.parse(await reviewedText(tcpExecution.evidenceId)); expect(observedTcp).toMatchObject({ protocol: 'TCP', applicationBytesSent: 0, dnsUsed: false, plannedEndpoints: [`127.0.0.1:${ports.tcp}`] });
    expect(traffic.allowedTcpSockets).toBe(1); expect(traffic.tcpBytes).toBe(0); expect(traffic.deniedTcpSockets).toBe(0); expect(traffic.deniedHttpSockets).toBe(0);
    return { httpExecutionId: httpExecution.id, tcpExecutionId: tcpExecution.id, targetRefs: [http.targetRef, tcp.targetRef], requestedPorts: tcp.requestedPorts, omissions: tcp.scopeOmissions, traffic: { ...traffic } };
  });
  await gate('coverage-export-and-backup-retain-selected-origins', async () => {
    const coverage = await pure(() => scopeStatus(assessmentId)); expect(coverage.targetCoverage).toHaveLength(5);
    const localCoverage = coverage.targetCoverage.find(row => row.targetRef.entryId === option(status, secondary.id).targetRef.entryId);
    expect(localCoverage.methods.find(m => m.method === 'source.rules.v1').state).toBe('partial'); expect(localCoverage.methods.find(m => m.method === 'source.rules.v1').partial).toBe(true); expect(localCoverage.methods.find(m => m.method === 'source.capture.v1').state).toBe('separate_record');
    const untouched = coverage.targetCoverage.find(row => row.targetRef.entryId === option(status, webDenied.id).targetRef.entryId); expect(untouched.methods[0].state).toBe('not_attempted');
    await denied('report.bundle', { id: assessmentId, includeEvidence: true }, 'data_review_required');
    const report = await reviewedReport(request, { assessmentId, includeEvidence: true });
    const manifest = JSON.parse(await readFile(owned(report.manifestPath), 'utf8')); expect(manifest.partial).toBe(true); expect(manifest.assessment.targetRole).toBe('presentation_anchor_only');
    expect(manifest.targetCoverage).toEqual(coverage.targetCoverage); expect(manifest.evidence.find(e => e.id === captured.id).targetRefs).toEqual([option(status, secondary.id).targetRef]);
    expect(JSON.stringify(manifest)).not.toContain('catalog-unselected-marker'); expect(JSON.stringify(manifest)).not.toContain('CATALOG_UNSELECTED_MARKER');
    await denied('backup.create', {}, 'data_review_required');
    for (const assessment of snapshot().assessments) await authorizeData(request, assessment.id);
    const before = fingerprint(); const backup = await reviewedBackup(request); const verified = await pure(() => request('backup.verify', { id: backup.id })); expect(verified.verified).toBe(true);
    const restore = await pure(() => request('backup.restore', { id: backup.id })); expect(restore.activated).toBe(false);
    const copy = fingerprint(owned(restore.path)); expect(copy.version).toBe(8); expect(copy.requests).toBe(0); expect(copy.vault).toBe(0);
    const expectedCopy = structuredClone(before.state); for (const policy of expectedCopy.dataPolicies ?? []) { policy.active = false; policy.generation += 1; policy.activationGeneration += 1; }
    expect(copy.state).toEqual(expectedCopy); expect(copy.state.dataPolicies.every(policy => policy.active === false)).toBe(true);
    expect(backup.manifest.files.some(file => file.name === 'ai-budget.sqlite3')).toBe(false); await unchangedSources();
    return { reportDirectory: report.directory, backupId: backup.id, restoredPath: restore.path, exactScopeAndEvidencePreserved: true, restoredCopyActivated: false, coverageEntries: 5, partial: true };
  });
  await gate('combined-derivation-requires-new-review-and-refs', async () => {
    status = await scopeStatus(assessmentId); await refreshCatalog();
    const input = { assessmentId, expectedStateDigest: status.stateDigest, projectCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), title: 'Derived combined fixture', copyFields: ['objective', 'window', 'targets', 'exclusions'] };
    const preview = await pure(() => request('assessment.combinedDerivePreview', input)); expect(preview.canCreate).toBe(true);
    const commit = { ...input, reviewDigest: preview.reviewDigest, acknowledgeNewAssessment: true };
    const before = structuredClone(snapshot().assessments.find(a => a.id === assessmentId)); const created = await request('assessment.combinedDeriveCreate', commit); derivedId = created.assessmentId;
    expect(created.scopeStatus.phase).toBe('unreviewed'); expect(created.scopeStatus.revision.review).toBeNull(); expect((await pure(() => request('assessment.combinedDeriveCreate', commit))).replayed).toBe(true);
    const child = snapshot().assessments.find(a => a.id === derivedId); expect(child.evidence).toEqual([]); expect(snapshot().assessments.find(a => a.id === assessmentId)).toEqual(before);
    for (const table of ['plans', 'executions', 'findings', 'validations']) expect(snapshot()[table].filter(row => row.assessmentId === derivedId)).toEqual([]);
    const oldIds = status.targetOptions.map(v => v.targetRef.entryId); expect(created.scopeStatus.targetOptions.every(v => !oldIds.includes(v.targetRef.entryId))).toBe(true);
    await denied('assessment.targetPlan', targetPlanInput(created.scopeStatus, secondary.id, 'source.rules.v1'), 'scope_unreviewed');
    const edit = { assessmentId: derivedId, expectedStateDigest: created.scopeStatus.stateDigest, projectCatalogDigest: catalog.projectCatalogDigest, objective: child.objective, window: unboundedScopeWindow(), selection };
    const reviewed = await pure(() => request('assessment.combinedScopePreview', edit));
    const saved = await request('assessment.combinedScopeSave', { ...edit, reviewDigest: reviewed.reviewDigest, idempotencyKey: randomUUID(), acknowledgeScope: true });
    const planned = await request('assessment.targetPlan', targetPlanInput(saved.scopeStatus, secondary.id, 'source.rules.v1')); expect(planned.targetRef).not.toEqual(sourcePlan.targetRef);
    await request('plan.revoke', { id: planned.id, digest: planned.digest });
    return { derivedId, allEntryIdsRemapped: true, noProofOrApprovalCopied: true, reviewRequired: true, parentPreserved: true };
  });
  await gate('combined-revocation-and-restart-preserve-history', async () => {
    status = await scopeStatus(assessmentId);
    const queued = await request('assessment.targetPlan', targetPlanInput(status, webAllowed.id, 'http.headers.v1')); await request('plan.approve', { id: queued.id, digest: queued.digest });
    const current = await scopeStatus(assessmentId); const input = { assessmentId, expectedStateDigest: current.stateDigest, idempotencyKey: randomUUID(), rationale: 'End all further admissions in the owned combined fixture.', acknowledgeRevocation: true };
    const revoked = await request('assessment.scopeRevoke', input); expect(revoked.scopeStatus.phase).toBe('revoked'); expect(revoked.cancellationRequested).toBe(false);
    await denied('plan.run', { id: queued.id, digest: queued.digest }, 'scope_revoked'); await request('plan.revoke', { id: queued.id, digest: queued.digest });
    await denied('assessment.targetPlan', targetPlanInput(status, secondary.id, 'source.rules.v1'), 'scope_revoked');
    await denied('ai.context', aiPreviewInput(captured.id, [option(status, secondary.id).targetRef]), 'scope_revoked');
    const origins = await pure(() => request('evidence.origins', { assessmentId })); expect(origins.evidence.every(row => row.verified)).toBe(true);
    const before = fingerprint(); finalHash = before.sha256; await writeFile(join(directory, 'before-restart.json'), JSON.stringify(before, null, 2));
    await closeApp(); await openApp(binary); const after = fingerprint(); expect(after.sha256).toBe(finalHash); expect(after.version).toBe(8); expect(after.requests).toBe(0); expect(after.vault).toBe(0);
    expect((await scopeStatus(assessmentId)).phase).toBe('revoked'); expect((await scopeStatus(derivedId)).phase).toBe('frozen');
    expect((await scopeStatus(legacy.id)).revision).toEqual(legacy.scope.revision);
    await unchangedSources(); expect(traffic.deniedHttpSockets).toBe(0); expect(traffic.deniedTcpSockets).toBe(0); expect(traffic.allowedTcpSockets).toBe(1); expect(traffic.tcpBytes).toBe(0); expect(httpRequests).toHaveLength(1);
    await page.screenshot({ path: join(directory, 'native-after-restart.png'), fullPage: true });
    return { finalCoreSha256: finalHash, schema: 8, frozenAndRevokedPreserved: true, sourcesPreserved: true, providerRequests: 0, vaultEntries: 0, traffic: { ...traffic } };
  });
  result.passed = true;
} catch (error) { failure = error; result.error = String(error); result.stack = error?.stack; }
finally {
  if (failure && page) await page.screenshot({ path: join(directory, 'failure.png'), fullPage: true }).catch(error => diagnostics.push(`screenshot: ${error}`));
  await closeApp().catch(error => { failure ??= error; result.passed = false; result.cleanupError = String(error); });
  await releaseExcludedSources().catch(error => { failure ??= error; result.passed = false; result.sentinelCleanupError = String(error); });
  result.ownedProcessClosed = !child || child.exitCode !== null || child.signalCode !== null;
  for (const socket of sockets) socket.destroy();
  for (const server of targetListeners) if (server.listening) await new Promise((ok, bad) => { server.close(error => error ? bad(error) : ok()); server.closeAllConnections?.(); }).catch(error => { failure ??= error; result.passed = false; result.listenerCleanupError = String(error); });
  result.ownedListenersClosed = targetListeners.every(server => !server.listening);
  result.ownedSentinelsClosed = lockedSources.size === 0;
  if (!result.ownedProcessClosed || !result.ownedListenersClosed || !result.ownedSentinelsClosed) result.passed = false;
  try {
    const after = fingerprint(); await writeFile(join(directory, 'final-database.json'), JSON.stringify(after, null, 2));
    if (result.passed) { expect(after.sha256).toBe(finalHash); expect(after.requests).toBe(0); expect(after.vault).toBe(0); await unchangedSources(); }
  } catch (error) { diagnostics.push(`persistence: ${error}`); if (result.passed) { failure ??= error; result.passed = false; result.persistenceError = String(error); } }
  result.generatedAt = new Date().toISOString(); result.denials = denials; result.traffic = traffic; result.httpRequests = httpRequests;
  result.sourceHashes = Array.from(sources, ([path, content]) => ({ path, sha256: hash(content), bytes: Buffer.byteLength(content) }));
  await writeFile(join(directory, 'ipc-log.json'), JSON.stringify(ipc, null, 2)); await writeFile(join(directory, 'diagnostics.log'), diagnostics.join('\n'));
  await writeFile(join(directory, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ passed: result.passed, directory, gates: result.records.map(({ id, status }) => ({ id, status })), traffic, error: result.error, ownedProcessClosed: result.ownedProcessClosed, ownedListenersClosed: result.ownedListenersClosed }, null, 2));
}
if (failure || !result.passed) process.exitCode = 1;
