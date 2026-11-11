// Real managed Kali bridge, with synthetic bytes and the normal application
// profile. Never serialize the real snapshot, provider settings or app console.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { mkdir, readFile, realpath, writeFile } from 'node:fs/promises';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash, randomUUID } from 'node:crypto';
import { reviewedEvidenceText } from './data-fixture.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--expected-sha256' || !/^[a-f0-9]{64}$/.test(args[1])) throw new Error('Use --expected-sha256 <reviewed debug hash>.');
const hash = value => createHash('sha256').update(value).digest('hex');
const canonical = value => JSON.stringify(normalize(value));
function normalize(value) {
  if (Array.isArray(value)) return value.map(normalize);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, normalize(value[key])]));
  return value;
}
function check(condition, code) { if (!condition) throw new Error(`KALI_${code}`); }
const binary = join(root, 'src-tauri', 'target', 'debug', 'openkoda.exe');
const bytes = await readFile(binary);
check(await realpath(binary) === binary && hash(bytes) === args[1], 'BINARY_IDENTITY');
const directory = join(root, '.local', `native-combined-kali-${randomUUID()}`);
await mkdir(directory); check(await realpath(directory) === directory, 'ARTIFACT_DIRECTORY');
const output = { passed: false, directory, artifact: { version: '0.1.6', binary, sha256: hash(bytes), bytes: bytes.length, scriptSha256: hash(await readFile(fileURLToPath(import.meta.url))) }, records: [], ipc: [], ownedPids: [],
  limits: [
    'Uses the normal app profile only to retain the existing managed OpenKoda-Kali binding. No profile activation or data-directory override is requested.',
    'The pre-fixture snapshot is kept only in memory. Receipts contain preservation hashes/counts and owned fixture records; no real settings, snapshot, app console, screenshots or traces are saved.',
    'Data policy and classification are created only for the new synthetic project. Only public evidence_excerpt from its two sources may transfer as original evidence to local Kali; remote AI, reports and backup stay disabled.',
    'Read-only state digests cover the pre-existing projects and assessment data bindings. Snapshot checks concern the public projected DTO; no private database, TXT or vault is read.',
    'The two example.invalid URLs are identifiers only. The only methods are runtime diagnostics, hashing and fixed offline lexical parsing of the manually imported synthetic evidence.',
    'No provider action is allowed. ai.history is inspected only for this fixture. Global provider history is not audited because this API exposes no storage.status path; no vault, key or credential file is read.',
    'Bridge status exposes the latest 50 runs. Negative-call checks compare that complete projection; existing bridge history preservation is checked within its retained window.',
    'No backup, restore, purge, environment maintenance, Linux command, automatic reconciliation or deletion is requested. Failure leaves owned fixture history available for inspection.',
    'A native app restart is checked, not Windows reboot or power-loss recovery. An interrupted Linux operation is never claimed cleaned up without its bridge receipt.',
  ] };
let child, browser, page, boot = 0, baseline, baselineProjection, baselineBridge, baselineActivation, baselineDataStates;
let projectId, assessmentId, scope, refs, imported, importedText, secretEvidence, hashReview, scanReview, archived = false, revoked = false, timedOut = false, failure;
const runs = [], collectionKeys = ['projects', 'assessments', 'findings', 'plans', 'executions', 'validations', 'scopeRevisions', 'assessmentScopes', 'scopeReceipts'];
const ownEvidenceIds = new Set(), ownReviews = new Map();
const allowedActions = new Set(['snapshot', 'activation.status', 'environment.diagnose', 'kali.mcp.status', 'project.create', 'project.targetOptions', 'project.targetAdd', 'assessment.combinedPreview', 'assessment.combinedCreate', 'assessment.scopeStatus', 'evidence.import', 'ai.history', 'kali.mcp.diagnose', 'kali.mcp.hashEvidence', 'kali.mcp.scanEvidence', 'evidence.origins', 'assessment.scopeRevoke', 'project.archive', 'data.policy.status', 'data.policy.prepare', 'data.policy.commit', 'data.binding.prepare', 'data.binding.commit', 'data.classification.subjects', 'data.classification.prepare', 'data.classification.commit', 'data.session.select', 'evidence.access.prepare', 'evidence.access.read', 'kali.data.prepare']);
function safeError(error) {
  const text = String(error);
  return { code: /^Error: KALI_[A-Z0-9_]+$/.test(text) ? text.slice(7) : 'KALI_REVIEW_ARTIFACT_PHASE', detailSha256: hash(text) };
}
const deadline = setTimeout(() => { timedOut = true; if (child && child.exitCode === null && child.signalCode === null) child.kill(); }, 480000);
async function inventory() {
  const command = "$entries=@(Get-Process -Name 'openkoda','openkoda-debug' -ErrorAction SilentlyContinue | Select-Object Id); ConvertTo-Json -InputObject $entries -Compress; exit 0";
  const raw = await new Promise((ok, bad) => {
    const process = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], { windowsHide: true, stdio: ['ignore', 'pipe', 'ignore'], signal: AbortSignal.timeout(15000) });
    let stdout = ''; process.on('error', bad); process.stdout.on('data', data => { stdout += data; if (stdout.length > 32768) process.kill(); });
    process.once('exit', (code, signal) => code === 0 && !signal ? ok(stdout) : bad(new Error('KALI_PROCESS_INVENTORY')));
  });
  const parsed = raw.trim() ? JSON.parse(raw) : []; const entries = Array.isArray(parsed) ? parsed : [parsed];
  const owned = child && child.exitCode === null && child.signalCode === null ? child.pid : undefined;
  check(entries.every(entry => entry.Id === owned), 'FOREIGN_APPLICATION_PRESENT');
}
async function unusedPort() {
  const server = createServer();
  await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  const port = server.address().port; await new Promise((ok, bad) => server.close(error => error ? bad(error) : ok())); return port;
}
async function openApp() {
  check(!timedOut, 'GLOBAL_DEADLINE'); await inventory();
  check(!child || child.exitCode !== null || child.signalCode !== null, 'PREVIOUS_APP_RUNNING');
  check(await realpath(binary) === binary && hash(await readFile(binary)) === args[1], 'BINARY_CHANGED');
  const port = await unusedPort(); boot++;
  const env = { ...process.env, WEBVIEW2_USER_DATA_FOLDER: join(directory, `webview-${boot}`), WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}` };
  delete env.OPENKODA_DATA_DIR;
  child = spawn(binary, [], { cwd: dirname(binary), windowsHide: true, stdio: 'ignore', env }); output.ownedPids.push(child.pid);
  let startupError; child.on('error', error => { startupError = error; });
  await expect.poll(async () => {
    check(!timedOut && !startupError && child.exitCode === null && child.signalCode === null, 'APP_STARTUP');
    try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  }, { timeout: 45000 }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 15000 });
  const context = browser.contexts()[0]; await expect.poll(() => context.pages().length, { timeout: 15000 }).toBeGreaterThan(0);
  page = context.pages()[0]; page.setDefaultTimeout(15000);
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  await expect.poll(() => page.locator('#main-content').getAttribute('aria-busy'), { timeout: 45000 }).toBe('false');
  check(new URL(page.url()).hostname === 'tauri.localhost', 'APP_ORIGIN'); await inventory();
}
async function closeApp() {
  if (browser) { await browser.close(); browser = undefined; page = undefined; }
  if (child && child.exitCode === null && child.signalCode === null) { child.kill(); await expect.poll(() => child.exitCode !== null || child.signalCode !== null, { timeout: 12000 }).toBe(true); }
}
async function request(action, payload = {}) {
  check(!timedOut, 'GLOBAL_DEADLINE'); check(allowedActions.has(action), 'ACTION_NOT_ALLOWED');
  if (action === 'project.archive') check(payload.id === projectId && !!projectId, 'FOREIGN_ARCHIVE');
  const foreignStatus = action === 'data.policy.status' && baseline?.projects.some(project => project.id === payload.projectId) && (payload.assessmentId == null || baseline.assessments.some(assessment => assessment.id === payload.assessmentId && assessment.projectId === payload.projectId));
  if (payload.projectId) check(foreignStatus || payload.projectId === projectId && !!projectId, 'FOREIGN_PROJECT');
  if (payload.assessmentId) check(foreignStatus || payload.assessmentId === assessmentId && !!assessmentId, 'FOREIGN_ASSESSMENT');
  if (payload.subject) check(payload.subject.assessmentId === assessmentId && !!assessmentId, 'FOREIGN_DATA_SUBJECT');
  if (payload.evidenceId) check(ownEvidenceIds.has(payload.evidenceId), 'FOREIGN_EVIDENCE');
  if (payload.review) check(ownReviews.get(payload.review.id) === canonical(payload.review), 'FOREIGN_DATA_REVIEW');
  if (payload.dataReview) check(ownReviews.get(payload.dataReview.id) === canonical(payload.dataReview), 'FOREIGN_TRANSFER_REVIEW');
  const item = { action, at: new Date().toISOString(), status: 'pending' }; output.ipc.push(item);
  try {
    const value = await page.evaluate(async ({ action, payload }) => {
      let timer;
      try { return await Promise.race([window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }), new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('KALI_IPC_DEADLINE')), 90000); })]); }
      finally { clearTimeout(timer); }
    }, { action, payload });
    item.status = 'fulfilled';
    for (const review of [value.review, value.dataReview]) if (review?.id && review?.digest) ownReviews.set(review.id, canonical(review));
    if (action === 'evidence.import') ownEvidenceIds.add(value.id);
    if (['kali.mcp.diagnose', 'kali.mcp.hashEvidence', 'kali.mcp.scanEvidence'].includes(action) && value.run?.assessmentId === assessmentId) {
      if (value.run.evidenceId) ownEvidenceIds.add(value.run.evidenceId);
      output.bridgeAttempts ??= []; output.bridgeAttempts.push(structuredClone(value));
    }
    return value;
  } catch (error) { item.status = 'rejected'; item.error = safeError(error); throw error; }
}
function foreignProjection(state) {
  const copy = { ...state };
  for (const key of collectionKeys) copy[key] = (state[key] ?? []).filter(row => !(key === 'projects' && row.id === projectId) && !(key === 'assessments' && row.id === assessmentId) && !(assessmentId && row.assessmentId === assessmentId));
  return Object.fromEntries(Object.keys(copy).sort().map(key => [key, { sha256: hash(canonical(copy[key])), count: Array.isArray(copy[key]) ? copy[key].length : null }]));
}
async function foreignDataStates() {
  const states = [];
  for (const project of baseline.projects) {
    const status = await request('data.policy.status', { projectId: project.id }); states.push({ projectId: project.id, stateDigest: status.stateDigest });
    for (const assessment of baseline.assessments.filter(row => row.projectId === project.id)) {
      const status = await request('data.policy.status', { projectId: project.id, assessmentId: assessment.id }); states.push({ projectId: project.id, assessmentId: assessment.id, stateDigest: status.stateDigest });
    }
  }
  return { count: states.length, sha256: hash(canonical(states)) };
}
async function preservation(label) {
  const state = await request('snapshot'), projection = foreignProjection(state);
  output.preservation ??= []; output.preservation.push({ label, projection });
  check(canonical(projection) === canonical(baselineProjection), 'FOREIGN_DOMAIN_CHANGED');
  const dataStates = await foreignDataStates(); check(canonical(dataStates) === canonical(baselineDataStates), 'FOREIGN_DATA_AUTHORITY_CHANGED');
  output.preservation.at(-1).dataStates = dataStates;
  check(hash(canonical(await request('activation.status'))) === hash(canonical(baselineActivation)), 'PROFILE_SELECTION_CHANGED');
  return state;
}
async function noAi() {
  const history = await request('ai.history', { assessmentId });
  check(history.requests.length === 0 && history.messages.length === 0 && history.historySentToProvider === false, 'FIXTURE_PROVIDER_ACTIVITY');
  return { requests: 0, messages: 0, providerActionsDispatched: 0, globalHistoryAudited: false };
}
async function denied(action, payload, reason) {
  const before = await request('kali.mcp.status'); const domainHash = hash(canonical(await request('snapshot'))); let rejection;
  try { await request(action, payload); } catch (error) { rejection = String(error); }
  check(!!rejection && !/KALI_IPC_DEADLINE|Target closed|Page crashed|Execution context was destroyed/.test(rejection), 'EXPECTED_DOMAIN_DENIAL');
  if (reason) check(rejection.includes(reason), 'DENIAL_REASON');
  check(hash(canonical(await request('kali.mcp.status'))) === hash(canonical(before)), 'DENIAL_CREATED_BRIDGE_RUN');
  check(hash(canonical(await request('snapshot'))) === domainHash, 'DENIAL_MUTATED_DOMAIN');
  return { rejected: true, noNewBridgeRun: true, unchangedDomain: true, rejectionSha256: hash(rejection) };
}
async function gate(id, task) {
  const record = { id, status: 'running', startedAt: new Date().toISOString() }; output.records.push(record);
  try { record.detail = await task(); record.status = 'passed'; }
  catch (error) { record.status = 'failed'; record.error = safeError(error); throw error; }
  finally { record.endedAt = new Date().toISOString(); }
}
async function authorizeOwnedKaliData() {
  check(!!projectId && !!assessmentId, 'DATA_FIXTURE_MISSING');
  let status = await request('data.policy.status', { projectId, assessmentId });
  check(status.currentRevision == null && status.binding == null, 'UNEXPECTED_EXISTING_FIXTURE_POLICY');
  const body = structuredClone(status.defaultBody), rule = body.destinations.find(item => item.destination === 'local_kali');
  check(!!rule && body.destinations.every(item => !item.enabled) && body.recoveryBackup.enabled === false, 'POLICY_DEFAULTS');
  const sources = status.sourceOptions.filter(item => !item.inactive).map(item => item.targetCatalogId);
  check(sources.length === 2, 'POLICY_OWN_SOURCES');
  Object.assign(rule, { enabled: true, categories: ['evidence_excerpt'], sensitivities: ['public'], targetCatalogIds: sources, allowOriginalEvidence: true });
  const preview = await request('data.policy.prepare', { projectId, expectedStateDigest: status.stateDigest, body }); check(preview.canCommit === true, 'POLICY_PREVIEW');
  const saved = await request('data.policy.commit', { review: preview.review, idempotencyKey: randomUUID(), acknowledgePolicy: true }); check(saved.saved === true, 'POLICY_COMMIT');
  status = await request('data.policy.status', { projectId, assessmentId });
  const adoption = await request('data.binding.prepare', { assessmentId, policyRef: saved.policyRef, expectedStateDigest: status.stateDigest }); check(adoption.canCommit === true, 'DATA_BINDING_PREVIEW');
  const bound = await request('data.binding.commit', { review: adoption.review, idempotencyKey: randomUUID(), acknowledgeProspectiveAdoption: true });
  check(bound.adopted === true && bound.binding.assessmentId === assessmentId && bound.binding.projectId === projectId, 'DATA_BINDING');
  const listing = await request('data.classification.subjects', { assessmentId });
  const selected = listing.subjects.filter(row => row.subject.kind === 'scope_source' || row.subject.kind === 'evidence' && row.subject.evidenceId === imported.id).sort((a, b) => Number(a.subject.kind === 'evidence') - Number(b.subject.kind === 'evidence'));
  check(selected.length === 3, 'CLASSIFICATION_OWN_SELECTION');
  const classRefs = [];
  for (const row of selected) {
    check(row.subject.assessmentId === assessmentId && row.originRefs.length > 0, 'CLASSIFICATION_ORIGIN');
    const review = await request('data.classification.prepare', { subject: row.subject, expectedSourceFingerprint: row.sourceFingerprint, sensitivity: 'public', reason: 'Owned synthetic data reviewed only for fixed offline hashing and lexical parsing.' });
    check(review.proposal.secretMaterial === false && review.downgrade === false, 'FIXTURE_CLASSIFICATION_SECRET');
    const classified = await request('data.classification.commit', { review: review.review, idempotencyKey: randomUUID(), acknowledgeClassification: true });
    check(classified.classified === true, 'CLASSIFICATION_COMMIT'); classRefs.push(classified.classificationRef);
  }
  return { policyRef: saved.policyRef, destination: 'local_kali', categories: ['evidence_excerpt'], sensitivities: ['public'], sourceCount: 2, allowOriginalEvidence: true, classificationRefs: classRefs, remoteAiEnabled: false, reportEnabled: false, backupEnabled: false };
}
async function prepareTransfer(operation) {
  const preview = await request('kali.data.prepare', { assessmentId, evidenceId: imported.id, operation, packageSensitivity: 'public' });
  check(preview.canTransfer === true && preview.representation === 'original_evidence' && preview.requiresSensitiveConfirmation === false && !!preview.dataReview, 'TRANSFER_REVIEW');
  check(preview.rawSha256 === imported.rawSha256 && preview.rawBytes === imported.rawBytes && preview.rawSha256 === hash(importedText), 'TRANSFER_ORIGINAL_BYTES');
  check(canonical(preview.origins.map(origin => origin.targetRef).sort((a, b) => a.entryId.localeCompare(b.entryId))) === canonical(refs), 'TRANSFER_ORIGINS');
  return preview.dataReview;
}
async function verifyRun(response, tool, expectedRefs) {
  const run = response.run;
  check(run.assessmentId === assessmentId && run.tool === tool && run.status === 'completed' && run.cleanupConfirmed === true && response.cleanupConfirmed === true && run.launchAttempted === true, 'BRIDGE_NOT_COMPLETED');
  check(canonical(run.sourceTargetRefs ?? []) === canonical(expectedRefs), 'RUN_TARGET_REFS');
  check(canonical(run.scopeRef) === canonical({ assessmentId, revisionId: scope.revision.id, digest: scope.revision.digest }), 'RUN_SCOPE_REF');
  check(run.sourceSha256 === hash(await readFile(join(root, 'scripts', 'kali-executor.py'))), 'EXECUTOR_SOURCE_HASH');
  check((tool === 'runtime_diagnostics' ? run.sourceEvidenceId == null : run.sourceEvidenceId === imported.id), 'RUN_SOURCE_EVIDENCE');
  const state = await request('snapshot'); const assessment = state.assessments.find(row => row.id === assessmentId);
  const evidence = assessment.evidence.find(row => row.id === run.evidenceId); check(!!evidence && !Object.hasOwn(evidence, 'content') && !Object.hasOwn(evidence, 'sha256') && /^[a-f0-9]{64}$/.test(evidence.rawSha256), 'BRIDGE_EVIDENCE_DTO');
  const reviewedText = await reviewedEvidenceText(request, assessmentId, evidence.id, evidence.rawSha256);
  check(evidence.rawSha256 === hash(reviewedText) && evidence.rawBytes === Buffer.byteLength(reviewedText), 'BRIDGE_EVIDENCE_HASH');
  const document = JSON.parse(reviewedText);
  check(document.schema === 'openkoda.kali-evidence.v2' && document.runId === run.id && document.environmentId === run.environmentId && document.protocolVersion === '2025-11-25' && document.tool === tool && document.sourceSha256 === run.sourceSha256, 'BRIDGE_DOCUMENT');
  check(canonical(document.targetRefs) === canonical(expectedRefs) && canonical(document.scopeRef) === canonical(run.scopeRef) && canonical(document.result) === canonical(response.result), 'DOCUMENT_BINDING');
  check(document.sourceEvidenceId === (tool === 'runtime_diagnostics' ? null : imported.id), 'DOCUMENT_SOURCE');
  check(state.findings.filter(row => row.assessmentId === assessmentId).length === 0, 'AUTOMATIC_FINDING');
  runs.push(structuredClone(run)); return { run, evidence, result: response.result };
}
try {
  await openApp();
  await gate('real-profile-idle-and-managed-environment', async () => {
    baseline = await request('snapshot'); check(baseline.schema === 'openkoda.snapshot-view.v1', 'SNAPSHOT_DTO'); baselineProjection = foreignProjection(baseline);
    baselineDataStates = await foreignDataStates(); output.baselineDataStates = baselineDataStates;
    output.baseline = baselineProjection;
    baselineActivation = await request('activation.status');
    check(baselineActivation.pendingPlan == null && baselineActivation.restartRequired === false, 'PROFILE_PENDING');
    const busy = ['preparing', 'planning', 'queued', 'running', 'pausing', 'validating', 'canceling', 'unknown', 'awaiting_approval'];
    check(!baseline.assessments.some(row => busy.includes(row.status)), 'PROFILE_WORK_PENDING');
    check(!baseline.plans.some(row => ['approved', 'running', 'unknown'].includes(row.status)), 'PROFILE_PLAN_PENDING');
    check(!baseline.executions.some(row => ['running', 'canceling', 'unknown'].includes(row.status)), 'PROFILE_EXECUTION_PENDING');
    baselineBridge = await request('kali.mcp.status'); check(!baselineBridge.hasUnresolved && baselineBridge.active == null, 'BRIDGE_PENDING');
    const environment = await request('environment.diagnose');
    check(environment.status === 'ready' && environment.distro === 'OpenKoda-Kali' && environment.managedRegistration === true, 'ENVIRONMENT_NOT_READY');
    check(environment.details.uid > 0 && environment.details.interopEnabled === false && Array.isArray(environment.details.windowsMounts) && environment.details.windowsMounts.length === 0, 'ENVIRONMENT_BOUNDARY');
    await preservation('before-fixture');
    return { status: 'ready', distro: 'OpenKoda-Kali', uid: environment.details.uid, interopEnabled: false, windowsMounts: [], priorBridgeRunProjection: { count: baselineBridge.recent.length, sha256: hash(canonical(baselineBridge.recent)) } };
  });
  await gate('combined-fixture-and-explicit-manual-association', async () => {
    const project = await request('project.create', { name: `Synthetic combined Kali ${randomUUID()}`, kind: 'web', target: 'https://example.invalid/one', description: 'Owned integration fixture. No URL access; only imported synthetic bytes go to the managed bridge.' }); projectId = project.id; output.projectId = projectId;
    let catalog = await request('project.targetOptions', { projectId });
    const added = await request('project.targetAdd', { projectId, expectedCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), label: 'Second synthetic identifier', kind: 'web', target: 'https://example.invalid/two' });
    catalog = await request('project.targetOptions', { projectId });
    const methods = ['kali.hash-evidence.v1', 'kali.rg.signals.v1'];
    const input = { projectId, title: 'Real bridge with two synthetic contributors', objective: 'Verify fixed offline tools over explicitly imported synthetic text; never access the URL identifiers.', window: { startsAt: null, endsAt: null }, projectCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), selection: { primaryProjectTargetId: 'primary', entries: [{ projectTargetId: 'primary', methods }, { projectTargetId: added.targetId, methods }], commonMethods: ['kali.runtime-diagnostics.v1'], excludedProjectTargets: [], localExclusions: [], connectionExclusions: [] } };
    const preview = await request('assessment.combinedPreview', input); check(preview.canCommit === true, 'COMBINED_PREVIEW');
    const created = await request('assessment.combinedCreate', { ...input, reviewDigest: preview.reviewDigest, acknowledgeScope: true }); assessmentId = created.assessmentId; output.assessmentId = assessmentId; scope = created.scopeStatus;
    check(scope.phase === 'draft' && scope.targetOptions.length === 2, 'COMBINED_CREATED');
    refs = scope.targetOptions.map(row => row.targetRef).sort((a, b) => a.entryId.localeCompare(b.entryId));
    const text = 'eval(synthetic_fixture_only);\n// Literal test data; never execute this content.\n';
    imported = await request('evidence.import', { assessmentId, name: 'Synthetic two-entry parser input', sourceDescription: 'Literal fixture text associated by a human with both selected entries; no URL access or source collection.', content: text, acknowledgeUnverifiedSource: true, contextTargetRefs: refs });
    check(!Object.hasOwn(imported, 'content') && !Object.hasOwn(imported, 'sha256') && /^[a-f0-9]{64}$/.test(imported.rawSha256), 'IMPORTED_DTO');
    importedText = await reviewedEvidenceText(request, assessmentId, imported.id, imported.rawSha256);
    check(imported.rawSha256 === hash(importedText) && imported.rawBytes === Buffer.byteLength(importedText), 'IMPORTED_HASH');
    const document = JSON.parse(importedText); check(document.schema === 'openkoda.manual-evidence.v2' && document.sourceVerified === false && document.text === text && canonical(document.targetRefs) === canonical(refs), 'MANUAL_ASSOCIATION');
    await preservation('after-fixture');
    return { projectId, assessmentId, scope, imported, inputBytes: imported.rawBytes, inputSha256: imported.rawSha256, ai: await noAi() };
  });
  await gate('transfer-without-data-review-has-no-run', async () => {
    const hashRefusal = await denied('kali.mcp.hashEvidence', { assessmentId, evidenceId: imported.id, acknowledgeLocalTransfer: true }, 'data_review_required');
    const scanRefusal = await denied('kali.mcp.scanEvidence', { assessmentId, evidenceId: imported.id, acknowledgeLocalTransfer: true }, 'data_review_required');
    return { hashRefusal, scanRefusal };
  });
  await gate('owned-project-local-data-policy', authorizeOwnedKaliData);
  await gate('sensitive-and-recognized-secret-inputs-never-transfer', async () => {
    const sensitivity = await denied('kali.data.prepare', { assessmentId, evidenceId: imported.id, operation: 'hash', packageSensitivity: 'sensitive' }, 'data_destination_denied');
    secretEvidence = await request('evidence.import', { assessmentId, name: 'Owned recognized-secret denial fixture', sourceDescription: 'Synthetic marker retained only to verify that recognized secret bytes never transfer.', content: 'Authorization: Bearer SYNTHETIC-KALI-DENIAL-MARKER-123456\n', acknowledgeUnverifiedSource: true, contextTargetRefs: refs });
    check(secretEvidence.classification.secretMaterial === true && !Object.hasOwn(secretEvidence, 'content'), 'SECRET_DTO_REDACTED');
    const hashRefusal = await denied('kali.data.prepare', { assessmentId, evidenceId: secretEvidence.id, operation: 'hash', packageSensitivity: 'public' }, 'data_secret_material');
    const scanRefusal = await denied('kali.data.prepare', { assessmentId, evidenceId: secretEvidence.id, operation: 'scan', packageSensitivity: 'public' }, 'data_secret_material');
    return { sensitivity, secretEvidenceId: secretEvidence.id, rawSha256: secretEvidence.rawSha256, hashRefusal, scanRefusal, originalSecretBytesTransferred: false };
  });
  await gate('common-runtime-diagnostics-v2', async () => {
    const response = await request('kali.mcp.diagnose', { assessmentId });
    check(response.result?.uid > 0 && response.result?.interopEnabled === false && canonical(response.result?.windowsMounts) === '[]', 'RUNTIME_BOUNDARY');
    return verifyRun(response, 'runtime_diagnostics', []);
  });
  await gate('two-entry-real-hash-v2', async () => {
    hashReview = await prepareTransfer('hash');
    await denied('kali.mcp.hashEvidence', { assessmentId, evidenceId: imported.id, dataReview: hashReview, acknowledgeLocalTransfer: false });
    const input = { assessmentId, evidenceId: imported.id, dataReview: hashReview, acknowledgeLocalTransfer: true };
    const response = await request('kali.mcp.hashEvidence', input);
    check(response.result?.sha256 === imported.rawSha256 && response.result?.bytes === imported.rawBytes, 'REAL_HASH_BYTES');
    const verified = await verifyRun(response, 'hash_bytes', refs);
    await denied('kali.mcp.hashEvidence', input, 'data_review_used'); return { ...verified, dataReview: hashReview, originalBytesReviewed: true, replayDenied: true };
  });
  await gate('two-entry-real-fixed-lexical-scan-v2', async () => {
    scanReview = await prepareTransfer('scan'); check(scanReview.id !== hashReview.id && scanReview.digest !== hashReview.digest, 'SEPARATE_TOOL_REVIEW');
    await denied('kali.mcp.scanEvidence', { assessmentId, evidenceId: imported.id, dataReview: hashReview, acknowledgeLocalTransfer: true }, 'data_review_changed');
    await denied('kali.mcp.scanEvidence', { assessmentId, evidenceId: imported.id, dataReview: scanReview, acknowledgeLocalTransfer: false });
    const input = { assessmentId, evidenceId: imported.id, dataReview: scanReview, acknowledgeLocalTransfer: true };
    const response = await request('kali.mcp.scanEvidence', input);
    check(response.result?.adapter === 'kali.rg.signals.v1' && response.result?.inputSha256 === imported.rawSha256 && response.result?.bytes === imported.rawBytes && response.result?.termination === 'completed', 'REAL_SCAN_BYTES');
    check(response.result?.tool?.name === 'ripgrep' && /^[a-f0-9]{64}$/.test(response.result?.tool?.binarySha256 ?? '') && response.result?.candidates?.length > 0, 'REAL_SCAN_SIGNALS');
    const verified = await verifyRun(response, 'scan_bytes', refs);
    await denied('kali.mcp.scanEvidence', input, 'data_review_used'); return { ...verified, dataReview: scanReview, originalBytesReviewed: true, replayDenied: true };
  });
  await gate('origins-no-findings-and-no-provider', async () => {
    const origins = await request('evidence.origins', { assessmentId });
    check(origins.combined === true && origins.evidence.length === 5 && origins.findings.length === 0, 'ORIGIN_COUNTS');
    for (const row of origins.evidence) {
      const diagnostic = runs.find(run => run.tool === 'runtime_diagnostics');
      check(row.verified === true && canonical(row.targetRefs) === canonical(row.evidenceId === diagnostic.evidenceId ? [] : refs), 'ORIGIN_TARGETS');
      check(row.originKind === ([imported.id, secretEvidence.id].includes(row.evidenceId) ? 'user_association' : 'host_record'), 'ORIGIN_KIND');
    }
    const bridge = await request('kali.mcp.status'); check(!bridge.hasUnresolved && bridge.active == null, 'BRIDGE_NOT_IDLE');
    check(bridge.recent.filter(row => row.assessmentId === assessmentId).length === 3, 'OWN_RUN_COUNT');
    check(canonical(bridge.recent.filter(row => row.assessmentId !== assessmentId)) === canonical(baselineBridge.recent.slice(0, 47)), 'EXISTING_BRIDGE_HISTORY_CHANGED');
    await preservation('after-bridge'); return { origins, ai: await noAi(), ownRuns: 3, automaticFindings: 0, cleanupConfirmed: true };
  });
  await gate('revocation-denies-run-and-archives-only-fixture', async () => {
    const unusedReview = await prepareTransfer('hash');
    const current = await request('assessment.scopeStatus', { assessmentId });
    const response = await request('assessment.scopeRevoke', { assessmentId, expectedStateDigest: current.stateDigest, idempotencyKey: randomUUID(), rationale: 'Synthetic native bridge verification complete; deny every further admission.', acknowledgeRevocation: true });
    check(response.scopeStatus.phase === 'revoked' && response.cancellationRequested === false, 'REVOCATION'); revoked = true;
    const refusal = await denied('kali.mcp.hashEvidence', { assessmentId, evidenceId: imported.id, dataReview: unusedReview, acknowledgeLocalTransfer: true }, 'scope_revoked');
    await request('project.archive', { id: projectId }); archived = true;
    const state = await preservation('after-archive'); check(state.projects.find(row => row.id === projectId)?.archived === true, 'FIXTURE_NOT_ARCHIVED');
    return { projectId, assessmentId, revoked: true, archived: true, refusal, ai: await noAi() };
  });
  await gate('restart-preserves-fixture-origins-and-existing-profile', async () => {
    const before = await request('snapshot'), beforeHash = hash(canonical(before));
    const bridgeBefore = await request('kali.mcp.status');
    const activation = await request('activation.status'); check(activation.pendingPlan == null && !activation.restartRequired, 'RESTART_PROFILE_PENDING');
    await closeApp(); await openApp();
    const after = await preservation('after-restart'); check(hash(canonical(after)) === beforeHash, 'RESTART_DOMAIN_CHANGED');
    check(hash(canonical(await request('kali.mcp.status'))) === hash(canonical(bridgeBefore)), 'RESTART_BRIDGE_CHANGED');
    check((await request('assessment.scopeStatus', { assessmentId })).phase === 'revoked', 'RESTART_REVOCATION');
    const origins = await request('evidence.origins', { assessmentId }); check(origins.evidence.length === 5 && origins.evidence.every(row => row.verified), 'RESTART_ORIGINS');
    const fixture = { project: after.projects.find(row => row.id === projectId), assessment: after.assessments.find(row => row.id === assessmentId), scopeRevisions: (after.scopeRevisions ?? []).filter(row => row.assessmentId === assessmentId), assessmentScopes: (after.assessmentScopes ?? []).filter(row => row.assessmentId === assessmentId), scopeReceipts: (after.scopeReceipts ?? []).filter(row => row.assessmentId === assessmentId), runs, origins };
    await writeFile(join(directory, 'fixture.json'), JSON.stringify(fixture, null, 2));
    return { domainSha256: beforeHash, bridgeProjectionSha256: hash(canonical(bridgeBefore)), archived: true, revoked: true, originsVerified: 5, ai: await noAi() };
  });
  output.passed = true;
} catch (error) { failure = error; output.error = safeError(error); output.stoppedWithoutRepair = true; }
finally {
  // No retry, re-dispatch, purge or automatic environment repair follows failure.
  if (baselineProjection && page && !timedOut) {
    try { await preservation('final'); if (assessmentId) output.finalAi = await noAi(); }
    catch (error) { failure ??= error; output.passed = false; output.preservationError = safeError(error); }
  }
  try { await closeApp(); } catch (error) { failure ??= error; output.passed = false; output.cleanupError = safeError(error); if (child && child.exitCode === null && child.signalCode === null) child.kill(); }
  clearTimeout(deadline);
  output.ownedProcessClosed = !child || child.exitCode !== null || child.signalCode !== null;
  output.fixtureArchived = archived; output.scopeRevoked = revoked; output.globalDeadlineReached = timedOut;
  output.confirmedBridgeCleanups = runs.filter(run => run.cleanupConfirmed).length;
  if (!output.ownedProcessClosed || timedOut) output.passed = false;
  output.generatedAt = new Date().toISOString();
  await writeFile(join(directory, 'result.json'), JSON.stringify(output, null, 2));
  console.log(JSON.stringify({ passed: output.passed, directory, records: output.records.map(({ id, status }) => ({ id, status })), error: output.error, ownedProcessClosed: output.ownedProcessClosed, fixtureArchived: archived, scopeRevoked: revoked }, null, 2));
}
if (failure || !output.passed) process.exitCode = 1;
