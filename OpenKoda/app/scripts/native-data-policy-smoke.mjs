// OpenKoda real native IPC, exclusively owned synthetic data.
// Requires the exact reviewed custom-protocol debug hash.
// This script never configures a credential, reads API_OPEN_AI.txt/personal files,
// calls a provider, invokes WSL or requests a target operation. Any exported .txt
// read below was created by this exact run from the synthetic string below.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { mkdir, readFile, writeFile, realpath, readdir, lstat } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { reviewedAssessment } from './scope-fixture.mjs';
import { reviewedEvidenceText } from './data-fixture.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const { version } = JSON.parse(await readFile(join(root, 'package.json'), 'utf8'));
const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== '--expected-sha256' || !/^[a-f0-9]{64}$/.test(args[1])) {
  throw new Error('Use --expected-sha256 <reviewed custom-protocol debug hash>.');
}
if (process.platform !== 'win32') throw new Error('This fixture requires native Windows WebView2.');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const binary = join(root, 'src-tauri', 'target', 'debug', 'openkoda.exe');
const binaryBytes = await readFile(binary);
if (await realpath(binary) !== binary || hash(binaryBytes) !== args[1]) throw new Error('Debug binary differs from the reviewed artifact.');
const directory = join(root, '.local', `native-data-policy-${randomUUID()}`);
const dataDirectory = join(directory, 'data');
const targetDirectory = join(directory, 'owned-empty-source');
await mkdir(targetDirectory, { recursive: true });
if (await realpath(directory) !== directory) throw new Error('Fixture directory must not redirect.');
const tailCanary = `OWNED_SYNTHETIC_RAW_TAIL_${randomUUID()}`;
const inputText = `${'Synthetic evidence only; no credentials or executable content. é 🚀\n'.repeat(90)}${tailCanary}\n`;
const result = {
  passed: false, directory, artifact: { version, binary, sha256: hash(binaryBytes), bytes: binaryBytes.length }, records: [],
  limits: [
    'All mutations use the native public request IPC. SQLite is read-only and limited to this run directory; no policy, proof or receipt is injected.',
    'The only network endpoint intentionally opened is the owned loopback CDP harness. No target methods, provider calls, key configuration or WSL commands are requested.',
    'Exported evidence text originates solely from inputText in this script. No personal TXT, API_OPEN_AI.txt, existing profile or credential value is read.',
    'This covers native host wiring and persistence, not visual, keyboard, screen-reader or real-provider behavior.',
    'Restart is an owned application restart, not power-loss or Windows reboot validation. Revocation does not claim recall of previously dispatched data.',
    'Run artifacts are preserved. Cleanup closes only the spawned process/CDP connection and performs no recursive filesystem deletion.',
  ],
};
const diagnostics = [], ipc = [], denials = [], pageErrors = [];
let child, browser, page, failure, boot = 0, projectId, assessmentId, evidenceId, canonicalEvidence, finalHash;

function owned(value, base = directory) {
  const ordinary = value.startsWith('\\\\?\\') && /^[a-z]:\\/i.test(value.slice(4)) ? value.slice(4) : value;
  const path = resolve(ordinary), part = relative(base, path);
  if (!part || isAbsolute(part) || part === '..' || part.startsWith(`..${sep}`)) throw new Error('Artifact escaped the owned fixture.');
  return path;
}
async function plainOwned(value, base = directory) {
  const path = owned(value, base);
  if (await realpath(path) !== path || (await lstat(path)).isSymbolicLink()) throw new Error('Owned artifact redirects outside its exact path.');
  return path;
}
async function filesIn(value) {
  try { return (await readdir(owned(value))).sort(); }
  catch (error) { if (error.code === 'ENOENT') return []; throw error; }
}
async function outputInventory() {
  return { exports: await filesIn(join(dataDirectory, 'exports')), backups: await filesIn(join(dataDirectory, 'backups')) };
}
async function fingerprint(base = dataDirectory) {
  const corePath = await plainOwned(join(base, 'openkoda.sqlite3'));
  const core = new DatabaseSync(corePath, { readOnly: true });
  let raw, version;
  try { raw = core.prepare('SELECT json FROM app_state WHERE id=1').get().json; version = core.prepare('PRAGMA user_version').get().user_version; }
  finally { core.close(); }
  const aiPath = await plainOwned(join(base, 'assistant.sqlite3'));
  const ai = new DatabaseSync(aiPath, { readOnly: true });
  try {
    // Only row counts in this new synthetic profile; no vault value/cipher is read.
    return { sha256: hash(raw), raw, state: JSON.parse(raw), version,
      requests: ai.prepare('SELECT count(*) AS count FROM requests').get().count,
      vaultRows: ai.prepare('SELECT count(*) AS count FROM vault').get().count };
  } finally { ai.close(); }
}
async function assertNoForeignApplications() {
  // Fixed read-only inventory; a pre-existing app is preserved, never terminated.
  const command = "$entries=@(Get-Process -Name 'openkoda','openkoda-debug' -ErrorAction SilentlyContinue | Select-Object Id,Path); ConvertTo-Json -InputObject $entries -Compress; exit 0";
  const output = await new Promise((ok, bad) => {
    const probe = spawn('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', command], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], signal: AbortSignal.timeout(15000) });
    let out = '', err = ''; probe.on('error', bad);
    probe.stdout.on('data', bytes => { out += bytes; if (out.length > 32768) probe.kill(); });
    probe.stderr.on('data', bytes => { err += bytes; });
    probe.once('exit', (code, signal) => code === 0 && !signal ? ok(out) : bad(new Error(`Application inventory failed: ${code ?? signal}; ${err}`)));
  });
  const values = output.trim() ? JSON.parse(output) : [], entries = Array.isArray(values) ? values : [values];
  const allowed = child && child.exitCode === null && child.signalCode === null ? [child.pid] : [];
  if (entries.some(entry => !allowed.includes(entry.Id))) throw new Error('Foreign OpenKoda process is running; preserve it and close it explicitly before this fixture.');
}
async function unusedPort() {
  const server = createServer();
  await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  const port = server.address().port;
  await new Promise((ok, bad) => server.close(error => error ? bad(error) : ok()));
  return port;
}
async function openApp() {
  if (child && child.exitCode === null && child.signalCode === null) throw new Error('Close the previous owned process before restart.');
  await assertNoForeignApplications();
  // Pin every boot, so replacing the executable mid-run cannot change the subject.
  expect(hash(await readFile(binary))).toBe(args[1]);
  boot++; const port = await unusedPort();
  child = spawn(binary, [], { cwd: dirname(binary), windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: {
    ...process.env, OPENKODA_DATA_DIR: dataDirectory, WEBVIEW2_USER_DATA_FOLDER: join(directory, `webview-${boot}`),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`,
  } });
  result.ownedPids ??= []; result.ownedPids.push(child.pid);
  let startupError; child.on('error', error => { startupError = error; });
  child.stdout.on('data', bytes => diagnostics.push(`boot ${boot} stdout: ${bytes}`));
  child.stderr.on('data', bytes => diagnostics.push(`boot ${boot} stderr: ${bytes}`));
  await expect.poll(async () => {
    if (startupError) throw startupError;
    if (child.exitCode !== null || child.signalCode !== null) throw new Error(`Owned app exited: ${child.exitCode ?? child.signalCode}`);
    try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  }, { timeout: 45000, message: 'Owned WebView2 CDP ready' }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context = browser.contexts()[0];
  await expect.poll(() => context.pages().length, { timeout: 15000 }).toBeGreaterThan(0);
  page = context.pages()[0]; page.setDefaultTimeout(15000);
  page.on('console', message => diagnostics.push(`console ${message.type()}: ${message.text()}`));
  page.on('pageerror', error => { pageErrors.push(error.message); diagnostics.push(`pageerror: ${error.message}`); });
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  expect(new URL(page.url()).hostname, 'Embedded custom-protocol frontend required; no dev server fallback').toBe('tauri.localhost');
  await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
}
async function closeApp() {
  try {
    if (browser) {
      await browser.contexts()[0]?.tracing.stop({ path: join(directory, `trace-${boot}.zip`) }).catch(error => diagnostics.push(`trace: ${error}`));
      await browser.close().catch(error => diagnostics.push(`CDP close: ${error}`));
    }
  } finally {
    browser = undefined; page = undefined;
    if (child && child.exitCode === null && child.signalCode === null) {
      child.kill();
      await expect.poll(() => child.exitCode !== null || child.signalCode !== null, { timeout: 10000, message: 'Exact owned process exited' }).toBe(true);
    }
  }
}
async function request(action, payload = {}) {
  const entry = { action, at: new Date().toISOString() }; ipc.push(entry);
  try {
    const value = await page.evaluate(async ({ action, payload }) => {
      let timer;
      try {
        return await Promise.race([
          window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }),
          new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('DATA_POLICY_IPC_DEADLINE')), 30000); }),
        ]);
      } finally { clearTimeout(timer); }
    }, { action, payload });
    entry.status = 'fulfilled'; return value;
  } catch (error) { entry.status = 'rejected'; entry.error = String(error); throw error; }
}
async function pure(task) {
  const before = await fingerprint(), outputs = await outputInventory();
  const value = await task();
  expect((await fingerprint()).sha256).toBe(before.sha256); expect(await outputInventory()).toEqual(outputs);
  return value;
}
async function denied(action, payload, code) {
  const before = await fingerprint(), outputs = await outputInventory(); let error;
  try { await request(action, payload); } catch (caught) { error = String(caught); }
  expect(error, `${action} must explicitly reject`).toBeTruthy();
  expect(error).not.toMatch(/DATA_POLICY_IPC_DEADLINE|Target closed|Page crashed|Execution context was destroyed/);
  expect(error).not.toContain(tailCanary);
  if (code) expect(error).toContain(code);
  expect((await fingerprint()).sha256).toBe(before.sha256); expect(await outputInventory()).toEqual(outputs);
  denials.push({ action, error }); return error;
}
async function gate(id, task) {
  const record = { id, status: 'running', startedAt: new Date().toISOString() }; result.records.push(record);
  try { record.detail = await task(); record.status = 'passed'; }
  catch (error) { record.status = 'failed'; record.error = String(error); throw error; }
  finally { record.endedAt = new Date().toISOString(); }
}
const dataStatus = () => request('data.policy.status', { projectId, assessmentId });
const reportInput = () => ({ assessmentId, format: 'markdown_bundle', evidenceIds: [evidenceId], derivedSubjects: [],
  categories: ['human_text', 'target_metadata', 'evidence_excerpt'], contextTargetRefs: [], packageSensitivity: 'public' });
const prepareWindow = (offset = 0, expectedProjectionSha256) => request('evidence.access.prepare', {
  assessmentId, evidenceId, expectedRawSha256: canonicalEvidence.sha256, offset, maxBytes: 1024,
  ...(expectedProjectionSha256 ? { expectedProjectionSha256 } : {}),
});
function assertProjection(value) {
  expect(value).not.toHaveProperty('content'); expect(value).not.toHaveProperty('sha256');
  expect(value.rawSha256).toBe(canonicalEvidence.sha256); expect(value.rawBytes).toBe(Buffer.byteLength(canonicalEvidence.content));
  expect(value.previewBytes).toBeLessThanOrEqual(2048); expect(value.previewTruncated).toBe(true);
  expect(JSON.stringify(value)).not.toContain(tailCanary);
}

try {
  await openApp();
  await gate('native-empty-profile-and-projected-import', async () => {
    const initial = await request('snapshot'); expect(initial.schema).toBe('openkoda.snapshot-view.v1'); expect(initial.projects).toEqual([]);
    const project = await request('project.create', { name: 'Owned data policy fixture', kind: 'code', target: targetDirectory, description: 'Synthetic native IPC review only.' }); projectId = project.id;
    const assessment = await reviewedAssessment(request, { projectId, title: 'Owned data review', kind: 'code', target: targetDirectory, objective: 'Review stored synthetic data without executing any target method.' }, []); assessmentId = assessment.id;
    const imported = await request('evidence.import', { assessmentId, name: 'Owned synthetic long evidence', sourceDescription: 'Literal generated inside native-data-policy-smoke.mjs.', content: inputText, acknowledgeUnverifiedSource: true }); evidenceId = imported.id;
    const state = await fingerprint(); expect(state.version).toBe(8); expect(state.requests).toBe(0); expect(state.vaultRows).toBe(0);
    canonicalEvidence = state.state.assessments.find(a => a.id === assessmentId).evidence.find(e => e.id === evidenceId);
    expect(hash(canonicalEvidence.content)).toBe(canonicalEvidence.sha256); expect(JSON.parse(canonicalEvidence.content).text).toBe(inputText);
    assertProjection(imported);
    const projected = await request('snapshot'); assertProjection(projected.assessments.find(a => a.id === assessmentId).evidence[0]);
    for (const field of ['dataPolicies', 'dataPolicyRevisions', 'dataBindings', 'dataClassifications', 'credentialReferences', 'dataUseReceipts']) expect(projected).not.toHaveProperty(field);
    const mcp = await request('mcp.request', { assessmentId, message: { jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'evidence_read', arguments: { evidenceId } } } });
    expect(mcp.result.isError).toBe(false); assertProjection(JSON.parse(mcp.result.content[0].text));
    await denied('report.export', { id: assessmentId }, 'data_review_required');
    await denied('report.bundle', { id: assessmentId, includeEvidence: true }, 'data_review_required');
    await denied('backup.create', {}, 'data_review_required');
    await denied('ai.context', { assessmentId, evidenceIds: [evidenceId], contextTargetRefs: [] }, 'data_review_required');
    return { projectId, assessmentId, evidenceId, rawSha256: canonicalEvidence.sha256, rawBytes: Buffer.byteLength(canonicalEvidence.content), projectionAndMcpOmitRaw: true, legacyAiContextDeniedWithoutMutation: true, schema: state.version };
  });
  let prePolicyWindow;
  await gate('explicit-local-windows-and-read-replay', async () => {
    await request('data.session.select', { assessmentId });
    let offset = 0, projectedHash, reconstructed = '', windows = 0;
    do {
      const prepared = await pure(() => prepareWindow(offset, projectedHash));
      expect(prepared.canRead).toBe(true); expect(prepared).not.toHaveProperty('text');
      if (windows === 0) await denied('evidence.access.read', { review: prepared.review, acknowledgeLocalRead: false });
      const payload = { review: prepared.review, acknowledgeLocalRead: true };
      const value = await request('evidence.access.read', payload);
      expect(value.rangeStart).toBe(offset); expect(value.returnedBytes).toBe(Buffer.byteLength(value.text)); expect(value.returnedBytes).toBeGreaterThan(0);
      expect(value.representation).toBe('raw_view'); reconstructed += value.text; offset = value.rangeEnd; projectedHash = value.projectionSha256; windows++;
      const replay = await pure(() => request('evidence.access.read', payload));
      expect(replay.alreadyUsed).toBe(true); expect(replay.receiptRef).toEqual(value.receiptRef); expect(replay).not.toHaveProperty('text');
      if (!value.hasMore) break;
      if (windows > 32) throw new Error('Bounded raw window loop did not finish.');
    } while (true);
    expect(reconstructed).toBe(canonicalEvidence.content); expect(hash(reconstructed)).toBe(projectedHash);
    expect(await reviewedEvidenceText(request, assessmentId, evidenceId, canonicalEvidence.sha256)).toBe(canonicalEvidence.content);
    const stale = await prepareWindow(); await request('data.session.select', { assessmentId: null });
    await denied('evidence.access.read', { review: stale.review, acknowledgeLocalRead: true }, 'data_view_changed');
    await request('data.session.select', { assessmentId }); prePolicyWindow = await prepareWindow();
    expect((await fingerprint()).state.dataBindings ?? []).toEqual([]);
    return { windows, reconstructedBytes: Buffer.byteLength(reconstructed), replayReturnsNoBytes: true, viewEpochInvalidatesPendingWindow: true };
  });
  await gate('policy-adoption-classification-and-explicit-origin', async () => {
    const status = await dataStatus(); expect(status.currentRevision).toBeNull();
    const body = structuredClone(status.defaultBody);
    const report = body.destinations.find(rule => rule.destination === 'report_export');
    Object.assign(report, { enabled: true, categories: ['human_text', 'target_metadata', 'evidence_excerpt'], sensitivities: ['public'], targetCatalogIds: ['primary'], allowOriginalEvidence: false });
    body.recoveryBackup = { enabled: true, sensitivities: ['public'] };
    const prepared = await pure(() => request('data.policy.prepare', { projectId, expectedStateDigest: status.stateDigest, body }));
    expect(prepared.canCommit).toBe(true);
    const payload = { review: prepared.review, idempotencyKey: randomUUID(), acknowledgePolicy: true };
    const saved = await request('data.policy.commit', payload); expect(saved.saved).toBe(true);
    expect((await pure(() => request('data.policy.commit', payload))).replayed).toBe(true);
    await denied('evidence.access.read', { review: prePolicyWindow.review, acknowledgeLocalRead: true }, 'data_policy_changed');
    const beforeAdoption = await dataStatus(); expect(beforeAdoption.binding).toBeNull();
    const adoption = await pure(() => request('data.binding.prepare', { assessmentId, policyRef: saved.policyRef, expectedStateDigest: beforeAdoption.stateDigest }));
    const adoptionPayload = { review: adoption.review, idempotencyKey: randomUUID(), acknowledgeProspectiveAdoption: true };
    const adopted = await request('data.binding.commit', adoptionPayload); expect(adopted.adopted).toBe(true); expect(adopted.binding.origin).toBe('explicit_adoption');
    expect((await pure(() => request('data.binding.commit', adoptionPayload))).replayed).toBe(true);
    const subjects = await request('data.classification.subjects', { assessmentId });
    const origins = subjects.subjects.filter(row => row.subject.kind === 'scope_source').flatMap(row => row.originRefs);
    expect(origins).toHaveLength(1);
    for (const row of subjects.subjects) {
      const payload = { subject: row.subject, expectedSourceFingerprint: row.sourceFingerprint, sensitivity: 'public', reason: 'Synthetic fixture explicitly reviewed for this isolated native test.' };
      if (row.subject.kind === 'evidence' && !row.originVerified && row.originRefs.length === 0) payload.associationRefs = origins;
      const preview = await pure(() => request('data.classification.prepare', payload)); expect(preview.proposal.secretMaterial).toBe(false);
      const classified = await request('data.classification.commit', { review: preview.review, idempotencyKey: randomUUID(), acknowledgeClassification: true });
      expect(classified.classified).toBe(true);
    }
    const current = await request('data.classification.subjects', { assessmentId });
    expect(current.subjects.every(row => row.classification.sensitivity === 'public')).toBe(true);
    const evidence = current.subjects.find(row => row.subject.evidenceId === evidenceId); expect(evidence.originVerified).toBe(false); expect(evidence.originRefs).toEqual(origins);
    expect((await fingerprint()).state.assessments.find(a => a.id === assessmentId).evidence[0]).toEqual(canonicalEvidence);
    return { policyRef: saved.policyRef, explicitBinding: true, subjectsClassified: current.subjects.length, associationVerifiedByHost: false, originalPreserved: true };
  });
  await gate('report-exact-bytes-and-replay', async () => {
    const prepared = await pure(() => request('report.prepare', reportInput()));
    expect(prepared.canExport).toBe(true); expect(prepared.dataReview).toBeTruthy(); expect(prepared.manifestPreview.appVersion).toBe(version);
    const payload = { dataReview: prepared.dataReview, acknowledgeExport: true };
    const report = await request('report.commit', payload); expect(report.completed).toBe(true);
    const destination = await plainOwned(report.directory, dataDirectory);
    const manifestBytes = await readFile(await plainOwned(report.manifestPath, destination));
    const markdownBytes = await readFile(await plainOwned(report.path, destination));
    expect(JSON.parse(manifestBytes.toString('utf8'))).toEqual(prepared.manifestPreview); expect(markdownBytes.toString('utf8')).toBe(prepared.markdownPreview);
    for (const artifact of prepared.artifacts) {
      // Every path is named by this exact synthetic reviewed package and confined
      // to its exclusive report folder; no user TXT or external path is read.
      const bytes = await readFile(await plainOwned(join(destination, artifact.name), destination));
      expect(bytes.length).toBe(artifact.bytes); expect(hash(bytes)).toBe(artifact.sha256);
    }
    const exported = prepared.manifestPreview.evidence.find(e => e.id === evidenceId);
    expect((await readFile(await plainOwned(join(destination, exported.file), destination), 'utf8'))).toBe(canonicalEvidence.content);
    const replay = await pure(() => request('report.commit', payload)); expect(replay.alreadyUsed).toBe(true); expect(replay.receiptRef).toEqual(report.receiptRef);
    expect((await dataStatus()).binding.frozen.receiptRef).toEqual(report.receiptRef);
    return { reportId: report.reportId, receiptRef: report.receiptRef, files: prepared.artifacts, exactReviewedBytes: true, replayCreatedNothing: true };
  });
  await gate('backup-exact-profile-replay-and-restore-inactive', async () => {
    const before = await fingerprint();
    const prepared = await pure(() => request('backup.prepare', { projectIds: [projectId], packageSensitivity: 'public', acknowledgeRecoveryPurpose: true }));
    expect(prepared.canBackup).toBe(true); expect(prepared.dataReview).toBeTruthy();
    const payload = { dataReview: prepared.dataReview, acknowledgeRecoveryBackup: true };
    const backup = await request('backup.commit', payload); expect(backup.verified).toBe(true);
    const destination = await plainOwned(backup.path, dataDirectory);
    expect(backup.manifest.files.some(file => file.name === 'ai-budget.sqlite3')).toBe(false);
    for (const file of backup.manifest.files) {
      const bytes = await readFile(await plainOwned(join(destination, file.name), destination)); expect(bytes.length).toBe(file.bytes); expect(hash(bytes)).toBe(file.sha256);
    }
    const copy = await fingerprint(destination); expect(copy.raw).toBe(before.raw); expect(copy.requests).toBe(0); expect(copy.vaultRows).toBe(0);
    const replay = await pure(() => request('backup.commit', payload)); expect(replay.alreadyUsed).toBe(true); expect(replay.receiptRef).toEqual(backup.receiptRef); expect(replay.backupId).toBe(backup.backupId);
    const liveBeforeRestore = await fingerprint();
    const restored = await request('backup.restore', { id: backup.backupId }); expect(restored.activated).toBe(false);
    const restoredPath = await plainOwned(restored.path, dataDirectory), restoredState = await fingerprint(restoredPath);
    expect((await fingerprint()).sha256).toBe(liveBeforeRestore.sha256); expect(restoredState.version).toBe(8);
    expect(restoredState.state.dataPolicies.every(policy => policy.active === false)).toBe(true);
    expect(restoredState.state.dataPolicies[0].generation).toBeGreaterThan(copy.state.dataPolicies[0].generation);
    expect(restoredState.state.dataPolicies[0].activationGeneration).toBeGreaterThan(copy.state.dataPolicies[0].activationGeneration);
    expect(restoredState.state.dataPolicyRevisions).toEqual(copy.state.dataPolicyRevisions); expect(restoredState.state.dataUseReceipts).toEqual(copy.state.dataUseReceipts);
    expect(restoredState.state.assessments.find(a => a.id === assessmentId).evidence[0]).toEqual(canonicalEvidence);
    expect(restoredState.requests).toBe(0); expect(restoredState.vaultRows).toBe(0);
    return { backupId: backup.backupId, receiptRef: backup.receiptRef, restoredPath, fullCanonicalSnapshotPreservedInBackup: true, restoredPolicyInactive: true, historicalRevisionsAndReceiptsPreserved: true, globalBudgetExcluded: true };
  });
  await gate('revocation-denies-stale-output-and-survives-restart', async () => {
    const pending = await pure(() => request('report.prepare', reportInput())); expect(pending.canExport).toBe(true);
    // Project mutations use the project's digest, while assessment status also
    // binds its pin/classifications and has a deliberately different digest.
    const current = await request('data.policy.status', { projectId });
    const payload = { projectId, expectedStateDigest: current.stateDigest, idempotencyKey: randomUUID(), acknowledgeRevocation: true };
    const revoked = await request('data.policy.revoke', payload); expect(revoked.revoked).toBe(true); expect(revoked.status.active).toBe(false);
    expect((await pure(() => request('data.policy.revoke', payload))).replayed).toBe(true);
    await denied('report.commit', { dataReview: pending.dataReview, acknowledgeExport: true }, 'data_policy');
    const next = await pure(() => request('report.prepare', reportInput())); expect(next.canExport).toBe(false); expect(next.dataReview).toBeNull(); expect(next.reasonCodes).toContain('data_policy_inactive');
    expect(next.manifestPreview).toBeNull(); expect(next.markdownPreview).toBe(''); expect(next.artifacts).toEqual([]); expect(next.omissions).toEqual([]);
    expect(JSON.stringify(next)).not.toContain(tailCanary);
    const before = await fingerprint(); finalHash = before.sha256;
    await closeApp(); await openApp();
    const after = await fingerprint(); expect(after.sha256).toBe(finalHash); expect(after.requests).toBe(0); expect(after.vaultRows).toBe(0);
    expect((await dataStatus()).active).toBe(false);
    const projected = await request('snapshot'); assertProjection(projected.assessments.find(a => a.id === assessmentId).evidence[0]);
    expect(after.state.assessments.find(a => a.id === assessmentId).evidence[0]).toEqual(canonicalEvidence);
    expect(after.state.plans).toEqual([]); expect(after.state.executions).toEqual([]);
    await page.screenshot({ path: join(directory, 'native-after-restart.png'), fullPage: true });
    return { staleOutputDenied: true, revokedStateDurable: true, originalPreserved: true, finalCoreSha256: finalHash, targetPlans: 0, executions: 0, providerRequests: 0, credentialRows: 0 };
  });
  expect(pageErrors).toEqual([]); result.passed = true;
} catch (error) { failure = error; result.error = String(error); result.stack = error?.stack; }
finally {
  if (failure && page) await page.screenshot({ path: join(directory, 'failure.png'), fullPage: true }).catch(error => diagnostics.push(`screenshot: ${error}`));
  await closeApp().catch(error => { failure ??= error; result.passed = false; result.cleanupError = String(error); });
  result.ownedProcessClosed = !child || child.exitCode !== null || child.signalCode !== null;
  if (!result.ownedProcessClosed) result.passed = false;
  try {
    const after = await fingerprint();
    result.finalStorage = { sha256: after.sha256, version: after.version, requests: after.requests, vaultRows: after.vaultRows, receiptCount: (after.state.dataUseReceipts ?? []).length };
    if (result.passed) { expect(after.sha256).toBe(finalHash); expect(after.requests).toBe(0); expect(after.vaultRows).toBe(0); }
  } catch (error) { diagnostics.push(`persistence: ${error}`); if (result.passed) { failure ??= error; result.passed = false; result.persistenceError = String(error); } }
  result.generatedAt = new Date().toISOString(); result.denials = denials; result.pageErrors = pageErrors;
  await writeFile(join(directory, 'ipc-log.json'), JSON.stringify(ipc, null, 2));
  await writeFile(join(directory, 'diagnostics.log'), diagnostics.join('\n'));
  await writeFile(join(directory, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ passed: result.passed, directory, gates: result.records.map(({ id, status }) => ({ id, status })), error: result.error, ownedProcessClosed: result.ownedProcessClosed }, null, 2));
}
if (failure || !result.passed) process.exitCode = 1;
