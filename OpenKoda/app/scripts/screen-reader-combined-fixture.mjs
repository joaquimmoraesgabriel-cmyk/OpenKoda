// Controlled 0.1.4 fixture. IPC prepares synthetic records only; all walkthrough
// UI input belongs to the coordinator using Windows computer control and NVDA.
import assert from 'node:assert/strict';
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer } from 'node:net';
import { mkdir, readFile, writeFile, realpath } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
assert.ok(args.length === 4 && args[0] === '--release-manifest' && args[2] === '--expected-sha256' && /^[a-f0-9]{64}$/.test(args[3]), 'Use --release-manifest <reviewed workspace manifest> --expected-sha256 <reviewed debug hash>.');
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const expectedSha256 = args[3];
function confined(path, base = root) {
  const absolute = resolve(base, path), part = relative(base, absolute);
  assert.ok(part && !isAbsolute(part) && part !== '..' && !part.startsWith(`..${sep}`), 'Path must remain inside its owned workspace.');
  return absolute;
}
async function exact(path) {
  assert.equal(await realpath(path), path, 'A fixture or artifact path must not redirect.');
  return path;
}
const manifestPath = await exact(confined(args[1]));
const manifest = JSON.parse((await readFile(manifestPath, 'utf8')).replace(/^\uFEFF/, ''));
assert.equal(manifest.version, '0.1.4');
assert.equal(manifest.nativeTestBinarySha256, expectedSha256, 'The walkthrough must target the reviewed 0.1.4 debug.');
const executable = await exact(confined(manifest.nativeTestBinary));
const binary = await readFile(executable);
assert.equal(sha256(binary), expectedSha256, 'The debug artifact changed since review.');
await exact(join(root, '.local'));
const directory = confined(join('.local', `screen-reader-combined-${randomUUID()}`));
await mkdir(directory);
await exact(directory);
const dataDirectory = join(directory, 'data'), sourceDirectory = join(directory, 'source');
await mkdir(sourceDirectory);
await exact(sourceDirectory);
const sourceFile = join(sourceDirectory, 'sample.txt');
const sourceBytes = 'Synthetic combined scope accessibility fixture. No personal data.\n';
await writeFile(sourceFile, sourceBytes, { flag: 'wx' });
const receipt = {
  passed: false, directory, manifestPath,
  artifact: { version: manifest.version, path: executable, sha256: expectedSha256, bytes: binary.length },
  observations: [],
  limits: [
    'This helper prepares an isolated synthetic profile and checks preservation; it does not operate the walkthrough UI.',
    'The coordinator observes NVDA 2026.3 with Speech Viewer and silence synthesis. This is not an auditory evaluation or a WCAG conformance assessment.',
    'No provider credential, provider call, Kali command, approved operation, target read, remediation, or real user profile is requested.',
    'All scope confirmation controls must remain unsubmitted during the walkthrough; transient form edits and previews do not authorize changes.',
    'Startup may inspect Windows/WSL registration metadata. It has no owned distro in this isolated profile.',
  ],
};
let child, browser, page, observedExit = false;
function databaseFingerprint() {
  const core = new DatabaseSync(confined('openkoda.sqlite3', dataDirectory), { readOnly: true });
  const ai = new DatabaseSync(confined('assistant.sqlite3', dataDirectory), { readOnly: true });
  try {
    const raw = core.prepare('SELECT json FROM app_state WHERE id=1').get().json;
    return {
      coreSha256: sha256(raw), userVersion: core.prepare('PRAGMA user_version').get().user_version,
      providerRequests: ai.prepare('SELECT count(*) AS count FROM requests').get().count,
      vaultEntries: ai.prepare('SELECT count(*) AS count FROM vault').get().count,
    };
  } finally { core.close(); ai.close(); }
}
async function unusedPort() {
  const listener = createServer();
  await new Promise((ok, bad) => { listener.once('error', bad); listener.listen(0, '127.0.0.1', ok); });
  const port = listener.address().port;
  await new Promise((ok, bad) => listener.close(error => error ? bad(error) : ok()));
  return port;
}
try {
  const port = await unusedPort();
  child = spawn(executable, [], { cwd: dirname(executable), windowsHide: true, stdio: 'ignore', env: {
    ...process.env, OPENKODA_DATA_DIR: dataDirectory, WEBVIEW2_USER_DATA_FOLDER: join(directory, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}`,
  } });
  receipt.ownedPid = child.pid;
  child.once('exit', () => { observedExit = true; });
  let startupError;
  child.on('error', error => { startupError = error; });
  await expect.poll(async () => {
    if (startupError) throw startupError;
    if (observedExit || child.exitCode !== null || child.signalCode !== null) throw new Error(`Owned app exited before walkthrough: ${child.exitCode ?? child.signalCode}`);
    try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  }, { timeout: 45000 }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context = browser.contexts()[0];
  await expect.poll(() => context.pages().length).toBeGreaterThan(0);
  page = context.pages()[0];
  await expect(page.locator('main')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
  const request = (action, payload = {}) => page.evaluate(({ action, payload }) => window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }), { action, payload });
  const project = await request('project.create', { name: 'Laboratório de escopo combinado', kind: 'files', target: sourceDirectory, description: 'Somente exemplos sintéticos para observar o leitor de tela.' });
  let catalog = await request('project.targetOptions', { projectId: project.id });
  const secondary = await request('project.targetAdd', { projectId: project.id, expectedCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), label: 'Página secundária de exemplo', kind: 'web', target: 'https://example.invalid/selected' });
  catalog = await request('project.targetOptions', { projectId: project.id });
  const selection = { primaryProjectTargetId: 'primary', entries: [{ projectTargetId: 'primary', methods: ['local.inventory.v1', 'ai.remote-review.v1'] }, { projectTargetId: secondary.targetId, methods: ['http.headers.v1', 'ai.remote-review.v1'] }], commonMethods: [], excludedProjectTargets: [], localExclusions: [{ projectTargetId: 'primary', relativePath: 'private', match: 'subtree', reason: 'Pasta fora do ensaio de leitura.' }], connectionExclusions: [] };
  async function createCombined(title, objective) {
    const input = { projectId: project.id, title, objective, window: { startsAt: null, endsAt: null }, projectCatalogDigest: catalog.projectCatalogDigest, idempotencyKey: randomUUID(), selection };
    const preview = await request('assessment.combinedPreview', input);
    const created = await request('assessment.combinedCreate', { ...input, reviewDigest: preview.reviewDigest, acknowledgeScope: true });
    return { id: created.assessmentId, title };
  }
  const draft = await createCombined('Fontes combinadas em revisão — leitura', 'Verificar entradas, métodos e exclusões sem confirmar alterações.');
  const frozen = await createCombined('Fontes combinadas congeladas — leitura', 'Verificar origem de evidência e seleção de contexto sem enviar dados.');
  const frozenStatus = await request('assessment.scopeStatus', { assessmentId: frozen.id });
  const secondaryEntry = frozenStatus.targetOptions.find(option => option.entry.projectTargetId === secondary.targetId);
  assert.ok(secondaryEntry?.targetRef, 'The host must project the secondary target reference.');
  await request('evidence.import', { assessmentId: frozen.id, name: 'Observação sintética da página secundária', sourceDescription: 'Texto criado exclusivamente para este ensaio.', content: 'Exemplo sem vulnerabilidade real. Nenhuma operação foi executada.', acknowledgeUnverifiedSource: true, contextTargetRefs: [secondaryEntry.targetRef] });
  const statuses = {
    draft: await request('assessment.scopeStatus', { assessmentId: draft.id }),
    frozen: await request('assessment.scopeStatus', { assessmentId: frozen.id }),
  };
  assert.equal(statuses.draft.phase, 'draft'); assert.equal(statuses.frozen.phase, 'frozen');
  assert.equal(statuses.draft.actions.canSave, true); assert.equal(statuses.frozen.actions.canRevoke, true);
  await page.reload();
  await expect(page.locator('main')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
  const before = await request('snapshot'), beforeFingerprint = databaseFingerprint();
  assert.equal(before.plans.length, 0); assert.equal(before.executions.length, 0); assert.equal(before.findings.length, 0);
  assert.equal(beforeFingerprint.providerRequests, 0); assert.equal(beforeFingerprint.vaultEntries, 0); assert.equal(beforeFingerprint.userVersion, 5);
  const metrics = () => page.evaluate(() => ({
    heading: document.querySelector('main h1')?.textContent ?? '',
    focus: document.activeElement?.getAttribute('aria-label') ?? document.activeElement?.textContent?.slice(0, 120) ?? '',
    width: innerWidth, scrollWidth: document.documentElement.scrollWidth,
  }));
  const ready = { directory, pid: child.pid, port, version: manifest.version, binarySha256: expectedSha256, projectId: project.id,
    draft: { id: draft.id, title: draft.title }, frozen: { id: frozen.id, title: frozen.title }, baseline: await metrics(),
    finishPath: join(directory, 'finish.json'), baselineFingerprint: beforeFingerprint };
  await writeFile(join(directory, 'ready.json'), JSON.stringify(ready, null, 2));
  await writeFile(join(directory, 'baseline.json'), JSON.stringify({ snapshot: before, fingerprint: beforeFingerprint }, null, 2));
  await writeFile(join(root, '.local', 'screen-reader-combined-current.json'), JSON.stringify(ready, null, 2));
  console.log(JSON.stringify({ ready: true, ...ready }));
  const deadline = Date.now() + 25 * 60 * 1000;
  let finish;
  while (Date.now() < deadline) {
    if (observedExit || child.exitCode !== null || child.signalCode !== null) throw new Error(`Owned app exited during walkthrough: ${child.exitCode ?? child.signalCode}`);
    const sample = await metrics();
    if (receipt.observations.at(-1)?.heading !== sample.heading) receipt.observations.push({ ...sample, at: new Date().toISOString() });
    try { finish = JSON.parse(await readFile(join(directory, 'finish.json'), 'utf8')); break; } catch (error) { if (error.code !== 'ENOENT') throw error; }
    await new Promise(ok => setTimeout(ok, 500));
  }
  assert.ok(finish, 'No manual observation receipt was provided within 25 minutes.');
  assert.equal(finish.reader, 'NVDA 2026.3'); assert.equal(finish.readerStarted, true);
  assert.ok(Array.isArray(finish.observations) && finish.observations.length > 0);
  receipt.review = finish;
  assert.deepEqual(await request('snapshot'), before, 'A walkthrough must not mutate the domain.');
  receipt.afterFingerprint = databaseFingerprint();
  assert.deepEqual(receipt.afterFingerprint, beforeFingerprint, 'Core JSON, provider requests and vault must remain unchanged.');
  assert.equal((await request('ai.status')).hasApiKey, false);
  assert.equal(await readFile(sourceFile, 'utf8'), sourceBytes, 'Synthetic source bytes must remain unchanged.');
  receipt.domainPreserved = true; receipt.sourcePreserved = true;
  receipt.passed = finish.passed === true;
  if (!receipt.passed) process.exitCode = 1;
} catch (error) { receipt.error = String(error); process.exitCode = 1; }
finally {
  await browser?.close().catch(() => {});
  if (child && !observedExit && child.exitCode === null && child.signalCode === null) {
    child.kill();
    await new Promise(ok => { child.once('exit', ok); setTimeout(ok, 5000); });
  }
  receipt.ownedProcessClosed = !child || observedExit || child.exitCode !== null || child.signalCode !== null;
  if (!receipt.ownedProcessClosed) { receipt.passed = false; receipt.cleanupError = 'Owned process did not confirm exit.'; process.exitCode = 1; }
  receipt.generatedAt = new Date().toISOString();
  await writeFile(join(directory, 'result.json'), JSON.stringify(receipt, null, 2));
  console.log(JSON.stringify({ passed: receipt.passed, error: receipt.error, directory, ownedProcessClosed: receipt.ownedProcessClosed }));
}
