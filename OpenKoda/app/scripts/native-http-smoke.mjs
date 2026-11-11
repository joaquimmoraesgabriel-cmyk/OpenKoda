// Real native IPC + three HEAD requests to an owned loopback server. All data
// belongs to this UUID fixture. Execute only against the coordinator's final
// custom-protocol debug binary: --expected-sha256 <reviewed binary sha256>.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createServer as createPortServer } from 'node:net';
import { mkdir, readFile, writeFile, realpath } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';
import { reviewedAssessment } from './scope-fixture.mjs';
import { reviewedEvidenceText, reviewedReport } from './data-fixture.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2), options = {};
for (let index = 0; index < args.length; index += 2) {
  if (!['--binary', '--expected-sha256'].includes(args[index]) || !args[index + 1] || options[args[index]]) throw new Error('Use --expected-sha256 <sha256> [--binary <workspace debug exe>].');
  options[args[index]] = args[index + 1];
}
if (!/^[a-f0-9]{64}$/.test(options['--expected-sha256'] ?? '')) throw new Error('An explicit reviewed binary SHA-256 is required.');
const binary = resolve(root, options['--binary'] ?? 'src-tauri/target/debug/openkoda.exe');
const binaryRelative = relative(root, binary);
if (isAbsolute(binaryRelative) || binaryRelative === '..' || binaryRelative.startsWith(`..${sep}`) || await realpath(binary) !== binary) throw new Error('Binary must be a non-redirected path within this application workspace.');
const sha256 = value => createHash('sha256').update(value).digest('hex');
const binaryBytes = await readFile(binary);
if (sha256(binaryBytes) !== options['--expected-sha256']) throw new Error('Binary changed from the explicitly reviewed artifact.');
const directory = join(root, '.local', `native-http-${randomUUID()}`);
const dataDirectory = join(directory, 'data'), cacheDirectory = join(directory, 'webview');
await mkdir(directory, { recursive: true });
if (await realpath(directory) !== directory) throw new Error('Fixture directory must not redirect outside the workspace.');
const owned = path => {
  // Rust canonicalize returns extended-length drive paths on Windows. Normalize
  // only that spelling; UNC/device paths still fail the owned-directory check.
  const ordinary = path.startsWith('\\\\?\\') && /^[a-z]:\\/i.test(path.slice(4)) ? path.slice(4) : path;
  const absolute = resolve(ordinary), part = relative(directory, absolute);
  if (!part || isAbsolute(part) || part === '..' || part.startsWith(`..${sep}`)) throw new Error('Artifact escaped the owned fixture.');
  return absolute;
};
const RULE = 'http.x-content-type-options-nosniff.v1';
const HEADER_NAMES = ['content-type', 'content-length', 'server', 'cache-control', 'location', 'strict-transport-security', 'content-security-policy', 'x-content-type-options', 'x-frame-options', 'referrer-policy'];
const result = { passed: false, directory, dataDirectory, binary, artifact: { sha256: sha256(binaryBytes), bytes: binaryBytes.length }, records: [],
  limits: ['Three real HEAD requests to one owned 127.0.0.1 server, using a fresh isolated native debug profile.',
    'The fixture operator changes the synthetic server response. OpenKoda does not apply a remote configuration change.',
    'The final fixed state confirms only the reviewed response, target, peer and instant; it does not prove exploitation, GET/browser behavior or site-wide security.',
    'Domain setup and decisions use real native IPC; this harness does not replace browser UI coverage or screen-reader testing.',
    'No provider request, provider credential, Linux command, real project, installed profile or external target is used.',
    'Startup may read WSL registration metadata; this empty fixture owns no distribution and requests no WSL operation.'] };
const diagnostics = [], requests = [], ipc = [], denied = [];
let child, browser, page, server, failure, bootNumber = 0, mode = 'missing';
let assessment, baseline, findingId, positive, confirmation, finalCoreSha256;

function databaseState() {
  const db = new DatabaseSync(owned(join(dataDirectory, 'openkoda.sqlite3')), { readOnly: true });
  const ai = new DatabaseSync(owned(join(dataDirectory, 'assistant.sqlite3')), { readOnly: true });
  try {
    const json = db.prepare('SELECT json FROM app_state WHERE id=1').get().json;
    return { state: JSON.parse(json), sha256: sha256(json), providerRequests: ai.prepare('SELECT count(*) AS count FROM requests').get().count,
      vaultEntries: ai.prepare('SELECT count(*) AS count FROM vault').get().count };
  } finally { db.close(); ai.close(); }
}
async function unusedPort() {
  const socket = createPortServer();
  await new Promise((ok, bad) => { socket.once('error', bad); socket.listen(0, '127.0.0.1', ok); });
  const port = socket.address().port;
  await new Promise((ok, bad) => socket.close(error => error ? bad(error) : ok()));
  return port;
}
async function request(action, payload = {}) {
  const item = { action, startedAt: new Date().toISOString(), hitsBefore: requests.length }; ipc.push(item);
  try {
    const value = await page.evaluate(async ({ action, payload }) => {
      let timer;
      try {
        return await Promise.race([
          window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }),
          new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('NATIVE_HTTP_IPC_DEADLINE')), 30000); }),
        ]);
      } finally { clearTimeout(timer); }
    }, { action, payload });
    item.status = 'fulfilled'; return value;
  } catch (error) { item.status = 'rejected'; item.error = String(error); throw error; }
  finally { item.hitsAfter = requests.length; item.endedAt = new Date().toISOString(); }
}
async function rejectWithoutEffects(action, payload, reason) {
  const before = databaseState(), hits = requests.length;
  let error;
  try { await request(action, payload); } catch (caught) { error = String(caught); }
  expect(error, `${action} must explicitly reject`).toBeTruthy();
  expect(error).not.toMatch(/NATIVE_HTTP_IPC_DEADLINE|Target closed|Execution context was destroyed|Page crashed/i);
  if (reason) expect(error).toMatch(reason);
  expect(requests).toHaveLength(hits);
  expect(databaseState().sha256, `${action} rejection must preserve the domain`).toBe(before.sha256);
  denied.push({ action, error }); return error;
}
async function noTraffic(task) {
  const hits = requests.length;
  const value = await task(); expect(requests).toHaveLength(hits); return value;
}
async function record(id, task) {
  const entry = { id, status: 'running', startedAt: new Date().toISOString() }; result.records.push(entry);
  try { entry.detail = await task(); entry.status = 'passed'; }
  catch (error) { entry.status = 'failed'; entry.error = String(error); throw error; }
  finally { entry.endedAt = new Date().toISOString(); }
}
async function startApp() {
  const port = await unusedPort(); bootNumber++;
  child = spawn(binary, [], { cwd: dirname(binary), windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'], env: {
    ...process.env, OPENKODA_DATA_DIR: dataDirectory, WEBVIEW2_USER_DATA_FOLDER: cacheDirectory,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`,
  } });
  result.ownedPids ??= []; result.ownedPids.push(child.pid);
  let startupError;
  child.on('error', error => { startupError = error; });
  child.stdout.on('data', bytes => diagnostics.push(`boot ${bootNumber} stdout: ${bytes}`));
  child.stderr.on('data', bytes => diagnostics.push(`boot ${bootNumber} stderr: ${bytes}`));
  await expect.poll(async () => {
    if (startupError) throw startupError;
    if (child.exitCode !== null) throw new Error(`Owned app exited: ${child.exitCode}`);
    try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  }, { timeout: 45000, message: 'Owned native WebView CDP becomes ready' }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context = browser.contexts()[0];
  await expect.poll(() => context.pages().length, { timeout: 15000 }).toBeGreaterThan(0);
  page = context.pages()[0]; page.setDefaultTimeout(15000);
  page.on('console', message => diagnostics.push(`console ${message.type()}: ${message.text()}`));
  page.on('pageerror', error => diagnostics.push(`pageerror: ${error.message}`));
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
  expect(new URL(page.url()).hostname, 'Require bundled custom-protocol UI').toBe('tauri.localhost');
  result.uiUrls ??= []; result.uiUrls.push(page.url());
  await page.screenshot({ path: join(directory, `startup-${bootNumber}.png`), fullPage: true });
}
async function stopApp() {
  if (browser) {
    await browser.contexts()[0]?.tracing.stop({ path: join(directory, `trace-${bootNumber}.zip`) }).catch(error => diagnostics.push(`trace: ${error}`));
    await browser.close().catch(error => diagnostics.push(`CDP close: ${error}`)); browser = undefined; page = undefined;
  }
  if (child && child.exitCode === null && child.signalCode === null) {
    child.kill();
    await expect.poll(() => child.exitCode !== null || child.signalCode !== null, { timeout: 10000, message: 'Only owned app closes' }).toBe(true);
  }
}
async function prepare() {
  const plan = await noTraffic(() => request('assessment.plan', { id: assessment.id }));
  expect(plan.adapter).toBe('http.headers.v1'); expect(plan.target).toBe(result.target);
  expect(plan.resolvedAddresses).toEqual([result.peer]); expect(plan.status).toBe('proposed');
  return plan;
}
async function runPlan(plan) {
  const decision = { id: plan.id, digest: plan.digest }, hits = requests.length;
  await noTraffic(() => request('plan.approve', decision));
  const execution = await request('plan.run', decision);
  expect(execution.status).toBe('completed'); expect(requests).toHaveLength(hits + 1);
  expect(requests.at(-1)).toMatchObject({ method: 'HEAD', url: '/fixture', mode, requestBytes: 0 });
  await rejectWithoutEffects('plan.run', decision, /aprova|encerrad|consumid|plano|Escopo, projeto ou estado mudou; a operação não foi iniciada/i);
  const state = databaseState().state;
  const evidence = state.assessments.find(a => a.id === assessment.id).evidence.find(e => e.id === execution.evidenceId);
  expect(evidence.sha256).toBe(sha256(evidence.content));
  const capture = JSON.parse(evidence.content);
  expect(capture).toMatchObject({ schema: 'openkoda.http-observation.v2', adapter: 'http.headers.v1', planDigest: plan.digest,
    target: result.target, method: 'HEAD', status: 200, remoteAddress: result.peer, captureComplete: true, redirectFollowed: false });
  expect(capture.resolvedAddresses).toEqual([result.peer]);
  expect(Object.keys(capture.headerValues).sort()).toEqual([...HEADER_NAMES].sort());
  expect(state.assessments.find(a => a.id === assessment.id).status).toBe('partial');
  return { execution, evidence, capture, plan };
}
const status = () => request('finding.httpStatus', { findingId });
const retestPreview = executionId => request('finding.httpRetestPreview', { findingId, executionId });

try {
  server = createServer((incoming, response) => {
    const item = { method: incoming.method, url: incoming.url, mode, requestBytes: 0, at: new Date().toISOString(),
      userAgent: incoming.headers['user-agent'], authorizationPresent: 'authorization' in incoming.headers, cookiePresent: 'cookie' in incoming.headers };
    requests.push(item); incoming.on('data', bytes => { item.requestBytes += bytes.length; });
    if (incoming.method !== 'HEAD' || incoming.url !== '/fixture') { response.writeHead(405); response.end(); return; }
    response.setHeader('Content-Length', '0'); response.setHeader('Content-Type', 'text/plain'); response.setHeader('Cache-Control', 'no-store');
    if (mode === 'duplicate') response.setHeader('X-Content-Type-Options', ['nosniff', 'other']);
    if (mode === 'present') response.setHeader('X-Content-Type-Options', 'nosniff');
    response.writeHead(200); response.end();
  });
  await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  result.peer = `127.0.0.1:${server.address().port}`; result.target = `http://${result.peer}/fixture`;
  await startApp();
  await record('http-native-approved-v2-baseline', async () => {
    expect((await request('snapshot')).projects).toHaveLength(0);
    const project = await request('project.create', { name: 'Owned native HTTP fixture', kind: 'web', target: result.target });
    assessment = await reviewedAssessment(request, { projectId: project.id, title: 'Factual HEAD review', kind: 'web', target: result.target, objective: 'Only the owned synthetic loopback response.' }, ['http.headers.v1']);
    result.projectId = project.id; result.assessmentId = assessment.id;
    const plan = await prepare();
    await rejectWithoutEffects('plan.run', { id: plan.id, digest: plan.digest }, /aprova/i);
    await rejectWithoutEffects('plan.approve', { id: plan.id, digest: '0'.repeat(64) }, /digest|plano|mud/i);
    expect(requests).toHaveLength(0);
    baseline = await runPlan(plan);
    expect(baseline.capture.headerValues['x-content-type-options']).toEqual([]);
    expect(databaseState().state.findings).toHaveLength(0);
    return { planId: plan.id, executionId: baseline.execution.id, evidenceId: baseline.evidence.id, evidenceSha256: baseline.evidence.sha256, hits: requests.length, autoFindings: 0 };
  });
  await record('http-preview-candidate-and-untrusted-import', () => noTraffic(async () => {
    const before = databaseState().sha256;
    const preview = await request('finding.httpPreview', { executionId: baseline.execution.id, ruleId: RULE });
    expect(preview.canCreateCandidate).toBe(true); expect(preview.observation.outcome).toBe('control_missing');
    expect(databaseState().sha256).toBe(before);
    const payload = { executionId: baseline.execution.id, ruleId: RULE, reviewDigest: preview.reviewDigest, acknowledgeLimits: true };
    await rejectWithoutEffects('finding.httpCandidate', { ...payload, acknowledgeLimits: false }, /limites/i);
    await rejectWithoutEffects('finding.httpCandidate', { ...payload, reviewDigest: '0'.repeat(64) }, /revisão|digest/i);
    const created = await request('finding.httpCandidate', payload); findingId = created.finding.id; result.findingId = findingId;
    expect(created.created).toBe(true); expect(created.finding.status).toBe('candidate'); expect(created.finding.severity).toBe('low');
    const freshPreview = await request('finding.httpPreview', { executionId: baseline.execution.id, ruleId: RULE });
    const repeatBefore = databaseState().sha256;
    const repeated = await request('finding.httpCandidate', { ...payload, reviewDigest: freshPreview.reviewDigest });
    expect(repeated.created).toBe(false); expect(repeated.finding.id).toBe(findingId); expect(databaseState().sha256).toBe(repeatBefore);
    const claim = databaseState().state.assessments.find(a => a.id === assessment.id).evidence.find(e => e.id === created.status.claim.evidenceId);
    const imported = await request('evidence.import', { assessmentId: assessment.id, name: 'Synthetic copied HTTP claim', sourceDescription: 'Unverified local test text, deliberately imitating the typed claim.', content: claim.content, acknowledgeUnverifiedSource: true });
    expect(imported).not.toHaveProperty('content');
    const importedText = await reviewedEvidenceText(request, assessment.id, imported.id, imported.rawSha256);
    expect(JSON.parse(importedText).schema).not.toBe('openkoda.http-claim.v1');
    const generic = await request('candidate.record', { assessmentId: assessment.id, title: 'Unverified copied assertion', description: 'Copied text is not native provenance.', severity: 'low', recommendation: 'Review provenance.', evidenceIds: [imported.id] });
    expect((await request('finding.httpStatus', { findingId: generic.id })).supported).toBe(false);
    await rejectWithoutEffects('finding.httpValidate', { findingId: generic.id, stateDigest: (await status()).stateDigest, rationale: 'The wrapper must not become a trusted claim.', acknowledgeLimits: true });
    await rejectWithoutEffects('finding.httpPreview', { executionId: imported.id, ruleId: RULE });
    result.untrustedFindingId = generic.id;
    return { findingId, duplicateCreated: false, importedEvidenceId: imported.id, untrustedFindingId: generic.id, untrustedSupported: false, hits: requests.length };
  }));
  await record('http-human-validation-and-stale-review', () => noTraffic(async () => {
    const stale = await status();
    const revoked = await prepare(); await request('plan.revoke', { id: revoked.id, digest: revoked.digest });
    const payload = { findingId, stateDigest: stale.stateDigest, rationale: 'Only this recorded HEAD response lacked the selected header.', acknowledgeLimits: true };
    await rejectWithoutEffects('finding.httpValidate', payload, /revisão|digest/i);
    const fresh = await status(); expect(fresh.stateDigest).not.toBe(stale.stateDigest);
    await rejectWithoutEffects('finding.httpValidate', { ...payload, stateDigest: fresh.stateDigest, acknowledgeLimits: false }, /limites/i);
    await rejectWithoutEffects('finding.validate', { id: findingId, method: 'controlled_reproduction', expected: 'Arbitrary exploitation', observed: 'Only HEAD evidence exists', evidenceIds: [baseline.evidence.id] });
    const validated = await request('finding.httpValidate', { ...payload, stateDigest: fresh.stateDigest });
    expect(validated.findingStatus).toBe('validated'); expect(validated.validation.createdAt).toBeTruthy();
    result.validation = validated.validation;
    const same = await retestPreview(baseline.execution.id);
    expect(same.canRecord).toBe(false); expect(same.reasons).toEqual(expect.arrayContaining(['same_execution', 'same_plan', 'predates_validation']));
    await rejectWithoutEffects('finding.httpRetest', { findingId, executionId: baseline.execution.id, reviewDigest: same.reviewDigest }, /Reteste|execução/i);
    await rejectWithoutEffects('finding.update', { id: findingId, status: 'fixed' });
    return { validation: validated.validation, staleDigestRejected: true, sameExecutionRejected: true, findingStatus: validated.findingStatus, hits: requests.length };
  }));
  await record('http-duplicate-header-is-inconclusive', async () => {
    mode = 'duplicate'; const run = await runPlan(await prepare());
    expect(run.capture.headerValues['x-content-type-options'].map(value => Buffer.from(value, 'base64').toString())).toEqual(['nosniff', 'other']);
    return noTraffic(async () => {
      const preview = await retestPreview(run.execution.id);
      expect(preview.canRecord).toBe(true); expect(preview.outcome).toBe('inconclusive'); expect(preview.observation.reasons).toContain('duplicate_header');
      expect(Object.values(preview.comparison)).toEqual(Array(6).fill(true));
      const recorded = await request('finding.httpRetest', { findingId, executionId: run.execution.id, reviewDigest: preview.reviewDigest });
      expect(recorded.recorded).toBe(true); expect(recorded.retest.outcome).toBe('inconclusive'); expect(recorded.status.findingStatus).toBe('validated');
      expect(recorded.status.actions.canConfirm).toBe(false);
      await rejectWithoutEffects('finding.httpConfirm', { findingId, retestEvidenceId: recorded.retest.evidenceId, stateDigest: recorded.status.stateDigest, rationale: 'Duplicate header must remain inconclusive.', acknowledgeLimits: true }, /último|positivo|comparável/i);
      return { executionId: run.execution.id, evidenceSha256: run.evidence.sha256, retestEvidenceId: recorded.retest.evidenceId, outcome: recorded.retest.outcome, confirmationAllowed: false, hits: requests.length };
    });
  });
  await record('http-positive-retest-separate-confirmation', async () => {
    mode = 'present'; positive = await runPlan(await prepare());
    expect(positive.capture.headerValues['x-content-type-options']).toEqual([Buffer.from('nosniff').toString('base64')]);
    return noTraffic(async () => {
      const preview = await retestPreview(positive.execution.id);
      expect(preview.canRecord).toBe(true); expect(preview.outcome).toBe('control_present'); expect(Object.values(preview.comparison)).toEqual(Array(6).fill(true));
      const payload = { findingId, executionId: positive.execution.id, reviewDigest: preview.reviewDigest };
      await rejectWithoutEffects('finding.httpRetest', { ...payload, reviewDigest: '0'.repeat(64) }, /revisão|digest/i);
      const recorded = await request('finding.httpRetest', payload);
      expect(recorded.status.findingStatus).toBe('validated'); expect(recorded.status.actions.canConfirm).toBe(true);
      const repeatedBefore = databaseState().sha256, repeated = await request('finding.httpRetest', payload);
      expect(repeated.recorded).toBe(false); expect(repeated.retest.evidenceId).toBe(recorded.retest.evidenceId); expect(databaseState().sha256).toBe(repeatedBefore);
      const confirm = { findingId, retestEvidenceId: recorded.status.actions.confirmRetestEvidenceId, stateDigest: recorded.status.stateDigest,
        rationale: 'Only this new comparable HEAD response contains a single nosniff value. The fixture operator changed the server; OpenKoda applied no remote change.', acknowledgeLimits: true };
      await rejectWithoutEffects('finding.httpConfirm', { ...confirm, acknowledgeLimits: false }, /limites/i);
      await rejectWithoutEffects('finding.httpConfirm', { ...confirm, stateDigest: '0'.repeat(64) }, /revisão|digest/i);
      confirmation = await request('finding.httpConfirm', confirm);
      expect(confirmation.findingStatus).toBe('fixed'); expect(confirmation.baseline.executionId).toBe(baseline.execution.id); expect(confirmation.baseline.outcome).toBe('control_missing');
      expect(confirmation.confirmation.retestEvidenceId).toBe(recorded.retest.evidenceId); expect(confirmation.actions.canConfirm).toBe(false);
      result.confirmation = confirmation.confirmation;
      return { executionId: positive.execution.id, retestEvidenceId: recorded.retest.evidenceId, outcome: recorded.retest.outcome,
        remainedValidatedBeforeHumanConfirmation: true, confirmation: confirmation.confirmation, findingStatus: confirmation.findingStatus, hits: requests.length };
    });
  });
  await record('http-proof-report-and-restart', async () => {
    const report = await noTraffic(() => reviewedReport(request, { assessmentId: assessment.id, includeEvidence: true }));
    const manifest = JSON.parse(await readFile(owned(report.manifestPath), 'utf8'));
    const row = manifest.httpValidations.find(item => item.findingId === findingId);
    expect(row.findingStatus).toBe('fixed'); expect(row.baseline.outcome).toBe('control_missing'); expect(row.retests.map(item => item.outcome)).toEqual(['inconclusive', 'control_present']);
    expect(row.retests[1].observation.headerTextIncluded).toBe(false); expect(row.retests[1].observation.headerValues[0]).not.toHaveProperty('text');
    const markdown = await readFile(owned(report.path), 'utf8'); expect(markdown).toContain('Validação factual e retestes HTTP');
    const checksums = JSON.parse(await readFile(owned(join(report.directory, 'checksums.json')), 'utf8'));
    expect(checksums.find(item => item.name === 'manifest.json').sha256).toBe(sha256(await readFile(owned(report.manifestPath)))); expect(checksums.find(item => item.name === 'report.md').sha256).toBe(sha256(markdown));
    const before = databaseState(); finalCoreSha256 = before.sha256;
    const evidence = before.state.assessments.find(a => a.id === assessment.id).evidence;
    for (const item of evidence) expect(item.sha256).toBe(sha256(item.content));
    expect(before.state.validations.filter(item => item.findingId === findingId).map(item => item.method)).toEqual(['passive_observation', 'human_review']);
    expect(before.providerRequests).toBe(0); expect(before.vaultEntries).toBe(0);
    await stopApp(); await startApp();
    const after = await noTraffic(status);
    expect(after).toEqual(confirmation); expect(databaseState().sha256).toBe(finalCoreSha256);
    expect(databaseState().state.findings.find(item => item.id === result.untrustedFindingId).status).toBe('candidate');
    expect(requests).toHaveLength(3); expect(requests.every(item => item.method === 'HEAD' && item.requestBytes === 0 && !item.authorizationPresent && !item.cookiePresent)).toBe(true);
    result.finalCoreSha256 = finalCoreSha256; result.finalHttpStatus = after;
    await writeFile(join(directory, 'final-snapshot.json'), JSON.stringify(databaseState().state, null, 2));
    await page.getByRole('navigation', { name: 'Navegação principal', exact: true }).getByRole('button', { name: 'Achados', exact: true }).click();
    await page.getByRole('button', { name: /Resposta HEAD sem X-Content-Type-Options/ }).click();
    await expect(page.getByRole('heading', { name: 'Alegação factual HTTP', exact: true })).toBeVisible();
    await page.screenshot({ path: join(directory, 'http-final-summary.png'), fullPage: true });
    await page.getByRole('button', { name: 'Correção', exact: true }).click();
    await expect(page.getByRole('heading', { name: 'Reteste do controle HTTP', exact: true })).toBeVisible();
    const confirmationRegion = page.getByRole('region', { name: 'Confirmação HTTP registrada', exact: true });
    await confirmationRegion.scrollIntoViewIfNeeded(); await expect(confirmationRegion).toBeVisible();
    await expect(confirmationRegion).toContainText(after.confirmation.retestEvidenceId);
    await page.screenshot({ path: join(directory, 'http-final-confirmation.png'), fullPage: true });
    expect(databaseState().sha256).toBe(finalCoreSha256); expect(requests).toHaveLength(3);
    result.nativeUiConfirmation = { rendered: true, findingId, retestEvidenceId: after.confirmation.retestEvidenceId, screenshot: join(directory, 'http-final-confirmation.png') };
    return { restartPreservedProofs: true, coreSha256: finalCoreSha256, evidenceHashesVerified: evidence.length, reportManifest: report.manifestPath,
      baselinePreserved: true, retests: ['inconclusive', 'control_present'], providerRequests: 0, vaultEntries: 0, totalHeadRequests: 3 };
  });
  result.passed = true;
} catch (error) { failure = error; result.error = String(error); result.stack = error?.stack; }
finally {
  if (failure && page) await page.screenshot({ path: join(directory, 'failure.png'), fullPage: true }).catch(error => diagnostics.push(`failure screenshot: ${error}`));
  await stopApp().catch(error => { failure ??= error; result.passed = false; result.cleanupError = String(error); });
  result.ownedProcessClosed = !child || child.exitCode !== null || child.signalCode !== null;
  if (server?.listening) await new Promise((ok, bad) => { server.close(error => error ? bad(error) : ok()); server.closeAllConnections(); }).catch(error => { failure ??= error; result.passed = false; result.serverCleanupError = String(error); });
  result.ownedServerClosed = !server?.listening;
  if (failure && result.ownedProcessClosed) {
    try { await writeFile(join(directory, 'failure-state.json'), JSON.stringify(databaseState(), null, 2)); }
    catch (error) { diagnostics.push(`Failure database artifact unavailable: ${error}`); }
  }
  if (finalCoreSha256 && result.ownedProcessClosed) {
    try { const state = databaseState(); expect(state.sha256).toBe(finalCoreSha256); expect(state.providerRequests).toBe(0); expect(state.vaultEntries).toBe(0); }
    catch (error) { failure ??= error; result.passed = false; result.persistenceError = String(error); }
  }
  result.generatedAt = new Date().toISOString(); result.denials = denied; result.httpRequests = requests;
  await writeFile(join(directory, 'diagnostics.log'), diagnostics.join('\n'));
  await writeFile(join(directory, 'ipc-log.json'), JSON.stringify(ipc, null, 2));
  await writeFile(join(directory, 'http-requests.json'), JSON.stringify(requests, null, 2));
  await writeFile(join(directory, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ passed: result.passed, directory, records: result.records.map(({ id, status }) => ({ id, status })), error: result.error,
    httpRequests: requests.length, ownedProcessClosed: result.ownedProcessClosed, ownedServerClosed: result.ownedServerClosed }, null, 2));
}
if (failure || !result.passed) process.exitCode = 1;
