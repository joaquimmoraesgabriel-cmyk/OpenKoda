// V1 scope compatibility on schema 8. Isolated fixture: one real loopback HEAD,
// local inventory, no provider/Kali. The preserved 0.1.2 binary creates legacy
// history through its own public API; no SQL policy/grant injection is used.
// Run only after the final debug build: --expected-sha256 <reviewed hash>.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createServer as createPortServer } from 'node:net';
import { mkdir, readFile, writeFile, realpath } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { reviewedAssessment, unboundedScopeWindow } from './scope-fixture.mjs';
import { reviewedReport } from './data-fixture.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--expected-sha256' || !/^[a-f0-9]{64}$/.test(args[1])) throw new Error('Use --expected-sha256 <reviewed final debug hash>.');
const sha256 = value => createHash('sha256').update(value).digest('hex');
const binary = join(root, 'src-tauri', 'target', 'debug', 'openkoda.exe');
const bytes = await readFile(binary);
if (await realpath(binary) !== binary || sha256(bytes) !== args[1]) throw new Error('Final debug identity differs from the reviewed artifact.');
const oldRelease = JSON.parse(await readFile(join(root, 'releases', '0.1.2-local-20261108', 'release.json'), 'utf8'));
const oldBinary = join(root, 'releases', '0.1.2-local-20261108', 'openkoda-debug.exe');
if (oldRelease.version !== '0.1.2' || await realpath(oldBinary) !== oldBinary || sha256(await readFile(oldBinary)) !== oldRelease.nativeTestBinarySha256) throw new Error('Preserved legacy debug identity differs.');
const directory = join(root, '.local', `native-scope-${randomUUID()}`), dataDirectory = join(directory, 'data');
const sourceDirectory = join(directory, 'owned-source'), sourceFile = join(sourceDirectory, 'fixture.txt');
await mkdir(sourceDirectory, { recursive: true });
if (await realpath(directory) !== directory) throw new Error('Fixture path must not redirect.');
const source = 'Owned scope fixture. Only inventory and hashes; no executable input.\n';
await writeFile(sourceFile, source);
const owned = value => {
  const ordinary = value.startsWith('\\\\?\\') && /^[a-z]:\\/i.test(value.slice(4)) ? value.slice(4) : value;
  const path = resolve(ordinary), part = relative(directory, path);
  if (!part || isAbsolute(part) || part === '..' || part.startsWith(`..${sep}`)) throw new Error('Artifact escaped the owned fixture.');
  return path;
};
const result = { passed: false, directory, artifact: { binary, sha256: sha256(bytes), bytes: bytes.length },
  legacyArtifact: { binary: oldBinary, sha256: oldRelease.nativeTestBinarySha256 }, records: [],
  limits: ['One isolated native profile; legacy history is created by the preserved 0.1.2 API, never by fabricated grants or SQL edits.',
    'Exactly one approved HEAD reaches the owned loopback listener. No provider credential/request, Linux command or real project is used.',
    'Future/expired windows are checked without altering Windows time or waiting for a timer; deadline interruption remains separate unit coverage.',
    'Revocation denies new admissions; this test does not claim cancellation of an already dispatched remote operation.',
    'Source bytes and domain hashes are checked; no claim is made to observe every denied filesystem read or OS scheduling race.',
    'Same-project derivation covers objective/methods/window only, without multi-target scope, exclusions, scheduler or generic execution.'] };
const diagnostics = [], traffic = [], denials = [], ipc = [];
let child, browser, page, server, failure, boot = 0, legacy, primary, primaryPlan, childAssessmentId, finalHash;
const fingerprint = () => {
  const core = new DatabaseSync(owned(join(dataDirectory, 'openkoda.sqlite3')), { readOnly: true });
  const ai = new DatabaseSync(owned(join(dataDirectory, 'assistant.sqlite3')), { readOnly: true });
  try {
    const json = core.prepare('SELECT json FROM app_state WHERE id=1').get().json;
    return { sha256: sha256(json), state: JSON.parse(json), version: core.prepare('PRAGMA user_version').get().user_version,
      requests: ai.prepare('SELECT count(*) AS count FROM requests').get().count, vault: ai.prepare('SELECT count(*) AS count FROM vault').get().count };
  } finally { core.close(); ai.close(); }
};
async function unusedPort() {
  const socket = createPortServer();
  await new Promise((ok, bad) => { socket.once('error', bad); socket.listen(0, '127.0.0.1', ok); });
  const port = socket.address().port; await new Promise((ok, bad) => socket.close(error => error ? bad(error) : ok())); return port;
}
async function openApp(executable) {
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
    if (child.exitCode !== null) throw new Error(`Owned native app exited: ${child.exitCode}`);
    try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  }, { timeout: 45000, message: 'Owned WebView CDP ready' }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context = browser.contexts()[0]; await expect.poll(() => context.pages().length, { timeout: 15000 }).toBeGreaterThan(0);
  page = context.pages()[0]; page.setDefaultTimeout(15000);
  page.on('console', item => diagnostics.push(`console ${item.type()}: ${item.text()}`)); page.on('pageerror', error => diagnostics.push(`pageerror: ${error.message}`));
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
  expect(new URL(page.url()).hostname).toBe('tauri.localhost');
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
  const item = { action, at: new Date().toISOString(), hitsBefore: traffic.length }; ipc.push(item);
  try {
    const value = await page.evaluate(async ({ action, payload }) => {
      let timer;
      try { return await Promise.race([window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }), new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('SCOPE_IPC_DEADLINE')), 30000); })]); }
      finally { clearTimeout(timer); }
    }, { action, payload }); item.status = 'fulfilled'; return value;
  } catch (error) { item.status = 'rejected'; item.error = String(error); throw error; }
  finally { item.hitsAfter = traffic.length; }
}
async function pure(task) {
  const before = fingerprint(), hits = traffic.length;
  const value = await task(); expect(fingerprint().sha256).toBe(before.sha256); expect(traffic).toHaveLength(hits); return value;
}
async function denied(action, payload, code) {
  const before = fingerprint(), hits = traffic.length; let error;
  try { await request(action, payload); } catch (caught) { error = String(caught); }
  expect(error, `${action} must explicitly reject`).toBeTruthy(); expect(error).not.toMatch(/SCOPE_IPC_DEADLINE|Target closed|Page crashed|Execution context was destroyed/);
  if (code) expect(error).toContain(code);
  expect(fingerprint().sha256).toBe(before.sha256); expect(traffic).toHaveLength(hits); expect(await readFile(sourceFile, 'utf8')).toBe(source);
  denials.push({ action, error }); return error;
}
async function gate(id, task) {
  const entry = { id, status: 'running', startedAt: new Date().toISOString() }; result.records.push(entry);
  try { entry.detail = await task(); entry.status = 'passed'; } catch (error) { entry.status = 'failed'; entry.error = String(error); throw error; }
  finally { entry.endedAt = new Date().toISOString(); }
}
const scopeStatus = assessmentId => request('assessment.scopeStatus', { assessmentId });
async function saveScope(assessmentId, objective, methods, window = unboundedScopeWindow()) {
  const current = await scopeStatus(assessmentId), payload = { assessmentId, expectedStateDigest: current.stateDigest, objective, methods, window };
  const preview = await pure(() => request('assessment.scopePreview', payload));
  expect(preview.canCommit).toBe(true);
  const input = { ...payload, reviewDigest: preview.reviewDigest, idempotencyKey: randomUUID(), acknowledgeScope: true };
  return { payload: input, action: preview.commitAction, saved: await request(preview.commitAction, input), preview };
}
const snapshot = () => fingerprint().state;

try {
  server = createServer((incoming, response) => {
    const item = { method: incoming.method, url: incoming.url, bytes: 0, at: new Date().toISOString() }; traffic.push(item);
    incoming.on('data', bytes => { item.bytes += bytes.length; });
    response.writeHead(incoming.method === 'HEAD' && incoming.url === '/scope' ? 200 : 405, { 'Content-Length': '0', 'Content-Type': 'text/plain', 'Cache-Control': 'no-store' }); response.end();
  });
  await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  result.target = `http://127.0.0.1:${server.address().port}/scope`;
  await openApp(oldBinary);
  await gate('legacy-fixture-from-preserved-api', async () => {
    const project = await request('project.create', { name: 'Owned legacy scope fixture', kind: 'code', target: sourceDirectory });
    const assessment = await request('assessment.create', { projectId: project.id, title: 'Preserved legacy history', kind: 'code', target: sourceDirectory, objective: 'Original legacy objective must remain unchanged.' });
    const plan = await request('assessment.plan', { id: assessment.id });
    await request('plan.approve', { id: plan.id, digest: plan.digest });
    const execution = await request('plan.run', { id: plan.id, digest: plan.digest }); expect(execution.status).toBe('completed');
    legacy = { project, assessmentId: assessment.id, planId: plan.id, before: fingerprint() };
    expect(legacy.before.version).toBe(3); expect(legacy.before.state.assessments[0]).not.toHaveProperty('scopeId');
    expect(traffic).toHaveLength(0); await writeFile(join(directory, 'legacy-before.json'), JSON.stringify(legacy.before, null, 2));
    return { assessmentId: assessment.id, executionId: execution.id, planDigest: plan.digest, domainSha256: legacy.before.sha256, providerRequests: 0 };
  });
  await closeApp(); await openApp(binary);
  await gate('legacy-migration-is-not-authorization', async () => {
    const after = fingerprint(); expect(after.version).toBe(8); expect(after.sha256).toBe(legacy.before.sha256);
    const status = await pure(() => scopeStatus(legacy.assessmentId)); expect(status.phase).toBe('legacy'); expect(status.revision).toBeNull(); expect(status.legacySummary.objective).toBe(legacy.before.state.assessments[0].objective);
    await denied('assessment.plan', { id: legacy.assessmentId }, 'scope_legacy_requires_adoption');
    await denied('report.bundle', { id: legacy.assessmentId, includeEvidence: true }, 'data_review_required');
    await denied('report.prepare', { assessmentId: legacy.assessmentId, evidenceIds: [], derivedSubjects: [], categories: ['human_text'], contextTargetRefs: [], packageSensitivity: 'public', format: 'markdown_bundle' }, 'data_source_unknown');
    expect(snapshot().scopeRevisions ?? []).toEqual([]); expect(snapshot().dataPolicies ?? []).toEqual([]);
    return { userVersion: [legacy.before.version, after.version], canonicalJsonPreserved: true, unreviewedLegacyExportDenied: true, legacyHasNoInventedMethods: true, hits: traffic.length };
  });
  let webProject;
  await gate('scope-options-create-and-unreviewed-refusals', async () => {
    webProject = await request('project.create', { name: 'Owned scoped loopback', kind: 'web', target: result.target });
    const options = await pure(() => request('assessment.scopeOptions', { projectId: webProject.id }));
    expect(options.schema).toBe('openkoda.scope-options.v1'); expect(options.defaultProposal.methods).toEqual([]);
    const base = { projectId: webProject.id, title: 'Explicit scope required', kind: 'web', target: result.target, objective: 'Only the owned HEAD response may be observed.' };
    const block = { methods: ['http.headers.v1'], window: unboundedScopeWindow(), projectDigest: options.projectDigest, catalogVersion: options.catalogVersion, acknowledgeScope: true };
    for (const change of [{ acknowledgeScope: false }, { projectDigest: '0'.repeat(64) }, { catalogVersion: 'untrusted-catalog' }, { methods: ['local.inventory.v1'] }, { methods: ['http.headers.v1', 'http.headers.v1'] }, { exclusions: [] }]) await denied('assessment.create', { ...base, scope: { ...block, ...change } });
    const localOptions = await pure(() => request('assessment.scopeOptions', { projectId: legacy.project.id }));
    await denied('assessment.create', { projectId: legacy.project.id, title: 'Missing capture dependency', kind: 'code', target: sourceDirectory, objective: 'Do not read source without the required method pair.',
      scope: { methods: ['source.capture.v1'], window: unboundedScopeWindow(), projectDigest: localOptions.projectDigest, catalogVersion: localOptions.catalogVersion, acknowledgeScope: true } }, 'scope_method_dependency_missing');
    primary = await request('assessment.create', base); result.assessmentId = primary.id;
    const status = await pure(() => scopeStatus(primary.id)); expect(status.phase).toBe('unreviewed'); expect(primary.scopeId).toBeTruthy(); expect(status.revision).toBeNull();
    await denied('assessment.plan', { id: primary.id }, 'scope_unreviewed');
    await denied('evidence.import', { assessmentId: primary.id, name: 'Unreviewed fixture', sourceDescription: 'Synthetic local text', content: 'Must not commit before objective review.', acknowledgeUnverifiedSource: true }, 'scope_unreviewed');
    const mcp = await pure(() => request('mcp.request', { assessmentId: primary.id, message: { jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'assessment_plan', arguments: {} } } }));
    expect(mcp.result.isError).toBe(true); expect(JSON.stringify(mcp.result.content)).toContain('scope_unreviewed');
    return { assessmentId: primary.id, scopeId: primary.scopeId, nativeAndMcpPreparationDenied: true, plans: snapshot().plans.filter(plan => plan.assessmentId === primary.id).length, hits: traffic.length };
  });
  await gate('scope-review-revisions-and-idempotency', async () => {
    const original = await scopeStatus(primary.id);
    const input = { assessmentId: primary.id, expectedStateDigest: original.stateDigest, objective: primary.objective, methods: ['http.headers.v1'], window: unboundedScopeWindow() };
    const preview = await pure(() => request('assessment.scopePreview', input)); expect(preview.commitAction).toBe('assessment.scopeSave'); expect(preview.willFreeze).toBe(false);
    await denied('assessment.scopeSave', { ...input, reviewDigest: preview.reviewDigest, idempotencyKey: randomUUID(), acknowledgeScope: false });
    await denied('assessment.scopeSave', { ...input, reviewDigest: '0'.repeat(64), idempotencyKey: randomUUID(), acknowledgeScope: true }, 'scope_review_changed');
    const saved = await saveScope(primary.id, primary.objective, ['http.headers.v1']);
    expect(saved.saved.scopeStatus.phase).toBe('draft'); expect(saved.saved.scopeStatus.frozen).toBeNull();
    const replay = await pure(() => request(saved.action, saved.payload)); expect(replay.replayed).toBe(true); expect(replay.revisionId).toBe(saved.saved.revisionId);
    await denied(saved.action, { ...saved.payload, objective: 'Changed intent with the same key.' }, 'scope_idempotency_conflict');
    await denied('assessment.scopeSave', { ...input, reviewDigest: preview.reviewDigest, idempotencyKey: randomUUID(), acknowledgeScope: true }, 'scope_state_changed');
    const revision2 = await saveScope(primary.id, 'Revised human objective: only one owned HEAD, without exploitation.', ['http.headers.v1']);
    expect(revision2.saved.scopeStatus.revision.number).toBe(2); expect(revision2.saved.scopeStatus.revision.previousRevisionId).toBe(saved.saved.revisionId);
    expect(snapshot().scopeRevisions.find(item => item.id === saved.saved.revisionId).body.objective).toBe(primary.objective);
    expect(traffic).toHaveLength(0);
    return { revisionIds: [saved.saved.revisionId, revision2.saved.revisionId], reviewedWithoutPlanOrTraffic: true, repeatedIntentCreatedNoRevision: true };
  });
  await gate('scope-method-and-window-admission', async () => {
    const now = Date.now();
    for (const [label, window, reason, state] of [
      ['future', { startsAt: new Date(now + 86400000).toISOString(), endsAt: new Date(now + 172800000).toISOString() }, 'scope_not_yet_open', 'not_yet_open'],
      ['expired', { startsAt: new Date(now - 172800000).toISOString(), endsAt: new Date(now - 86400000).toISOString() }, 'scope_expired', 'expired'],
    ]) {
      const assessment = await reviewedAssessment(request, { projectId: webProject.id, title: `Owned ${label} window`, kind: 'web', target: result.target, objective: 'This window must deny a new HEAD before traffic.' }, ['http.headers.v1'], window);
      expect((await scopeStatus(assessment.id)).windowStatus).toBe(state); await denied('assessment.plan', { id: assessment.id }, reason);
      expect((await scopeStatus(assessment.id)).phase).toBe('draft');
    }
    const localOnly = await reviewedAssessment(request, { projectId: webProject.id, title: 'Review stored text only', kind: 'web', target: result.target, objective: 'No acquisition method is selected.' }, []);
    await denied('assessment.plan', { id: localOnly.id }, 'scope_method_not_selected');
    await request('evidence.import', { assessmentId: localOnly.id, name: 'Local-only text', sourceDescription: 'Human supplied fixture', content: 'Stored data only; no authority to contact the target.', acknowledgeUnverifiedSource: true });
    const frozen = await scopeStatus(localOnly.id); expect(frozen.phase).toBe('frozen'); expect(frozen.revision.body.methods).toEqual([]); expect(frozen.frozen.triggerAction).toBe('evidence.import');
    await denied('assessment.plan', { id: localOnly.id }, 'scope_method_not_selected'); expect(traffic).toHaveLength(0);
    return { futureAndExpiredBlocked: true, emptyMethodsAllowsStoredTextOnly: true, firstSubstantiveWorkFrozen: true, hits: 0 };
  });
  await gate('scope-freeze-approval-and-revocation', async () => {
    primaryPlan = await request('assessment.plan', { id: primary.id });
    const frozen = await scopeStatus(primary.id); expect(frozen.phase).toBe('frozen'); expect(frozen.frozen.reason).toBe('first_work');
    expect(primaryPlan.scopeRef).toEqual({ assessmentId: primary.id, revisionId: frozen.revision.id, digest: frozen.revision.digest });
    expect(traffic).toHaveLength(0);
    const edit = { assessmentId: primary.id, expectedStateDigest: frozen.stateDigest, objective: 'Forbidden objective after first preparation.', methods: ['http.headers.v1'], window: unboundedScopeWindow() };
    const preview = await pure(() => request('assessment.scopePreview', edit)); expect(preview.canCommit).toBe(false); expect(preview.reasonCodes).toContain('scope_frozen');
    await denied('assessment.scopeSave', { ...edit, reviewDigest: preview.reviewDigest, idempotencyKey: randomUUID(), acknowledgeScope: true }, 'scope_frozen');
    const decision = { id: primaryPlan.id, digest: primaryPlan.digest };
    await denied('plan.run', decision); await request('plan.approve', decision);
    const execution = await request('plan.run', decision); expect(execution.status).toBe('completed'); expect(traffic).toHaveLength(1);
    const queued = await request('assessment.plan', { id: primary.id }); await request('plan.approve', { id: queued.id, digest: queued.digest });
    const current = await scopeStatus(primary.id), revocationInput = { assessmentId: primary.id, expectedStateDigest: current.stateDigest, idempotencyKey: randomUUID(), rationale: 'Do not admit further work in this owned fixture.', acknowledgeRevocation: true };
    const revoked = await request('assessment.scopeRevoke', revocationInput); expect(revoked.revoked).toBe(true); expect(revoked.scopeStatus.phase).toBe('revoked'); expect(revoked.cancellationRequested).toBe(false); expect(Array.isArray(revoked.admittedWork)).toBe(true);
    expect(snapshot().plans.find(item => item.id === queued.id).status).toBe('approved');
    const repeated = await pure(() => request('assessment.scopeRevoke', revocationInput)); expect(repeated.replayed).toBe(true);
    await denied('plan.run', { id: queued.id, digest: queued.digest }, 'scope_revoked');
    await request('plan.revoke', { id: queued.id, digest: queued.digest });
    await denied('assessment.plan', { id: primary.id }, 'scope_revoked');
    const historical = await pure(() => request('finding.httpPreview', { executionId: execution.id, ruleId: 'http.x-content-type-options-nosniff.v1' })); expect(historical.observation.outcome).toBe('control_missing');
    expect(traffic).toHaveLength(1);
    return { scopeRef: primaryPlan.scopeRef, executionId: execution.id, queuedPlanPreservedUntilSeparateRevoke: true, revocationDidNotClaimCancellation: true, historicalReviewAvailable: true, hits: 1 };
  });
  await gate('scope-derived-assessment-does-not-inherit-authority', async () => {
    const parentBefore = structuredClone(snapshot().assessments.find(item => item.id === primary.id));
    const current = await scopeStatus(primary.id), payload = { assessmentId: primary.id, expectedStateDigest: current.stateDigest, title: 'Derived owned assessment', copyFields: ['objective', 'methods', 'window'] };
    const preview = await pure(() => request('assessment.derivePreview', payload)); expect(preview.canCreate).toBe(true); expect(preview.proposal.projectId).toBe(primary.projectId); expect(preview.proposal.body.target).toBe(result.target);
    await denied('assessment.derivePreview', { ...payload, projectId: legacy.project.id });
    await denied('assessment.derivePreview', { ...payload, objective: 'Ambiguous copied and supplied field.' });
    const input = { ...payload, reviewDigest: preview.reviewDigest, idempotencyKey: randomUUID(), acknowledgeNewAssessment: true };
    await denied('assessment.deriveCreate', { ...input, acknowledgeNewAssessment: false });
    const created = await request('assessment.deriveCreate', input); childAssessmentId = created.assessmentId; result.derivedAssessmentId = childAssessmentId;
    expect(created.created).toBe(true); expect(created.scopeStatus.phase).toBe('unreviewed'); expect(created.scopeStatus.revision.origin).toBe('derived_prefill'); expect(created.scopeStatus.revision.review).toBeNull();
    const replay = await pure(() => request('assessment.deriveCreate', input)); expect(replay.replayed).toBe(true); expect(replay.assessmentId).toBe(childAssessmentId);
    await denied('assessment.deriveCreate', { ...input, title: 'Different child, same intent key' }, 'scope_idempotency_conflict');
    const state = snapshot(), derived = state.assessments.find(item => item.id === childAssessmentId);
    expect(derived.projectId).toBe(primary.projectId); expect(derived.evidence).toEqual([]); expect(derived.scopeId).not.toBe(primary.scopeId);
    for (const table of ['plans', 'executions', 'findings', 'validations']) expect(state[table].filter(item => item.assessmentId === childAssessmentId)).toEqual([]);
    expect(state.assessments.find(item => item.id === primary.id)).toEqual(parentBefore);
    await denied('assessment.plan', { id: childAssessmentId }, 'scope_unreviewed');
    const reviewed = await saveScope(childAssessmentId, derived.objective, ['http.headers.v1']); expect(reviewed.saved.scopeStatus.phase).toBe('draft');
    const plan = await request('assessment.plan', { id: childAssessmentId }); expect(plan.scopeRef.revisionId).toBe(reviewed.saved.revisionId); expect(plan.scopeRef.revisionId).not.toBe(primaryPlan.scopeRef.revisionId);
    await request('plan.revoke', { id: plan.id, digest: plan.digest }); expect(traffic).toHaveLength(1);
    return { childAssessmentId, sameProject: true, parentHistoryUnchanged: true, childHadNoProofsOrApprovals: true, reviewRequiredBeforePreparation: true, hits: 1 };
  });
  await gate('legacy-adoption-is-prospective', async () => {
    const oldAssessment = legacy.before.state.assessments.find(item => item.id === legacy.assessmentId), originalPlans = legacy.before.state.plans;
    const status = await scopeStatus(legacy.assessmentId);
    const edit = { assessmentId: legacy.assessmentId, expectedStateDigest: status.stateDigest, objective: 'Do not rewrite historical purpose.', methods: ['local.inventory.v1'], window: unboundedScopeWindow() };
    await denied('assessment.scopePreview', edit, 'scope_legacy_objective_immutable');
    const adopted = await saveScope(legacy.assessmentId, oldAssessment.objective, ['local.inventory.v1']); expect(adopted.action).toBe('assessment.scopeAdopt'); expect(adopted.saved.scopeStatus.phase).toBe('frozen'); expect(adopted.saved.scopeStatus.frozen.reason).toBe('legacy_adoption');
    const current = snapshot().assessments.find(item => item.id === legacy.assessmentId);
    expect(current.objective).toBe(oldAssessment.objective); expect(current.kind).toBe(oldAssessment.kind); expect(current.target).toBe(oldAssessment.target); expect(current.evidence).toEqual(oldAssessment.evidence);
    for (const plan of originalPlans) expect(snapshot().plans.find(item => item.id === plan.id)).toEqual(plan);
    await denied('plan.run', { id: legacy.planId, digest: originalPlans.find(item => item.id === legacy.planId).digest }, 'scope_legacy_plan_requires_reprepare');
    await denied('report.bundle', { id: legacy.assessmentId, includeEvidence: true }, 'data_review_required');
    const report = await reviewedReport(request, { assessmentId: legacy.assessmentId, includeEvidence: true });
    const manifest = JSON.parse(await readFile(owned(report.manifestPath), 'utf8'));
    expect(manifest.schema).toBe('openkoda.reviewed-report.v1'); expect(report.receiptRef).toMatchObject({ id: expect.any(String), digest: expect.any(String) });
    expect(manifest.scope.legacyUnversioned).toBe(false); expect(manifest.scope.binding.frozen.reason).toBe('legacy_adoption');
    expect(manifest.scope.revisions.map(revision => revision.id)).toEqual([adopted.saved.revisionId]);
    for (const oldPlan of originalPlans) { expect(snapshot().plans.find(item => item.id === oldPlan.id)).toEqual(oldPlan); expect(oldPlan).not.toHaveProperty('scopeRef'); }
    for (const oldEvidence of oldAssessment.evidence) expect(manifest.evidence.find(item => item.id === oldEvidence.id).rawSha256).toBe(oldEvidence.sha256);
    const plan = await request('assessment.plan', { id: legacy.assessmentId }); expect(plan.scopeRef.revisionId).toBe(adopted.saved.revisionId);
    await request('plan.approve', { id: plan.id, digest: plan.digest }); expect((await request('plan.run', { id: plan.id, digest: plan.digest })).status).toBe('completed');
    for (const item of oldAssessment.evidence) expect(snapshot().assessments.find(a => a.id === legacy.assessmentId).evidence.find(e => e.id === item.id)).toEqual(item);
    return { adoptedRevisionId: adopted.saved.revisionId, objectiveAndOldProofsPreserved: true, oldPlansNotBackfilled: true, newPlanScopeRef: plan.scopeRef, hits: traffic.length };
  });
  await gate('scope-restart-preserves-authority-and-history', async () => {
    const before = fingerprint(); finalHash = before.sha256; expect(before.requests).toBe(0); expect(before.vault).toBe(0);
    await writeFile(join(directory, 'final-before-restart.json'), JSON.stringify(before, null, 2));
    await closeApp(); await openApp(binary); const after = fingerprint(); expect(after.sha256).toBe(finalHash); expect(after.version).toBe(8);
    expect((await scopeStatus(primary.id)).phase).toBe('revoked'); expect((await scopeStatus(childAssessmentId)).phase).toBe('frozen'); expect((await scopeStatus(legacy.assessmentId)).frozen.reason).toBe('legacy_adoption');
    await denied('assessment.plan', { id: primary.id }, 'scope_revoked');
    expect(await readFile(sourceFile, 'utf8')).toBe(source); expect(traffic).toHaveLength(1); expect(traffic[0]).toMatchObject({ method: 'HEAD', url: '/scope', bytes: 0 });
    await page.screenshot({ path: join(directory, 'native-after-restart.png'), fullPage: true });
    result.finalCoreSha256 = finalHash;
    return { coreSha256: finalHash, revokedAndFrozenSurvivedRestart: true, sourceSha256: sha256(source), providerRequests: 0, vaultEntries: 0, totalHeadRequests: 1 };
  });
  result.passed = true;
} catch (error) { failure = error; result.error = String(error); result.stack = error?.stack; }
finally {
  if (failure && page) await page.screenshot({ path: join(directory, 'failure.png'), fullPage: true }).catch(error => diagnostics.push(`screenshot: ${error}`));
  await closeApp().catch(error => { failure ??= error; result.passed = false; result.cleanupError = String(error); });
  result.ownedProcessClosed = !child || child.exitCode !== null || child.signalCode !== null;
  if (server?.listening) await new Promise((ok, bad) => { server.close(error => error ? bad(error) : ok()); server.closeAllConnections(); }).catch(error => { failure ??= error; result.passed = false; result.serverCleanupError = String(error); });
  result.ownedServerClosed = !server?.listening;
  try {
    const current = fingerprint(); await writeFile(join(directory, 'final-database.json'), JSON.stringify(current, null, 2));
    if (result.passed) { expect(current.sha256).toBe(finalHash); expect(current.requests).toBe(0); expect(current.vault).toBe(0); }
  } catch (error) { diagnostics.push(`database check: ${error}`); if (result.passed) { failure ??= error; result.passed = false; result.persistenceError = String(error); } }
  result.generatedAt = new Date().toISOString(); result.denials = denials; result.httpRequests = traffic;
  await writeFile(join(directory, 'ipc-log.json'), JSON.stringify(ipc, null, 2)); await writeFile(join(directory, 'diagnostics.log'), diagnostics.join('\n'));
  await writeFile(join(directory, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ passed: result.passed, directory, gates: result.records.map(({ id, status }) => ({ id, status })), httpRequests: traffic.length, error: result.error,
    ownedProcessClosed: result.ownedProcessClosed, ownedServerClosed: result.ownedServerClosed }, null, 2));
}
if (failure || !result.passed) process.exitCode = 1;
