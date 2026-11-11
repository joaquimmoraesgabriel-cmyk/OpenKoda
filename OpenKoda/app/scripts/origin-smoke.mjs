// Scope: an explicitly identified debug build, one isolated profile, an owned HTTP
// loopback origin, and benign snapshot/project.create commands. No provider,
// Linux tool, arbitrary shell command, production edit, or real profile access.
import { chromium, expect } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createServer as createHttpServer } from 'node:http';
import { createServer as createPortServer } from 'node:net';
import { mkdir, readFile, writeFile, realpath } from 'node:fs/promises';
import { resolve, dirname, join, relative, isAbsolute, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { randomUUID, createHash } from 'node:crypto';
import { DatabaseSync } from 'node:sqlite';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
if (args.length && (args.length !== 2 || args[0] !== '--release-manifest')) throw new Error('Use [--release-manifest <workspace manifest>].');
function workspaceArtifact(path) {
  const absolute = resolve(root, path), part = relative(root, absolute);
  if (!part || isAbsolute(part) || part === '..' || part.startsWith(`..${sep}`)) throw new Error('Build artifacts must remain within the application workspace.');
  return absolute;
}
const manifestPath = workspaceArtifact(args[1] ?? 'releases/0.1.1-local-20261107/release.json');
if (await realpath(manifestPath) !== manifestPath) throw new Error('Manifest must not redirect outside the workspace.');
const manifest = JSON.parse(await readFile(manifestPath, 'utf8'));
const binary = workspaceArtifact(manifest.nativeTestBinary);
const digest = value => createHash('sha256').update(value).digest('hex');
const binaryBytes = await readFile(binary);
if (!/^\d+\.\d+\.\d+$/.test(manifest.version) || digest(binaryBytes) !== manifest.nativeTestBinarySha256) throw new Error('Native test binary identity differs from the explicit manifest.');
const actualBinary = await realpath(binary);
if (actualBinary !== binary) throw new Error('Binary must be the exact preserved workspace path.');
const directory = join(root, '.local', `origin-smoke-${randomUUID()}`);
const dataDirectory = join(directory, 'data');
const cacheDirectory = join(directory, 'webview');
const target = join(directory, 'target');
await mkdir(target, { recursive: true });
await writeFile(join(target, 'README.txt'), 'Owned benign origin-boundary test fixture.\n');
if (await realpath(directory) !== directory) throw new Error('Fixture directory must not redirect outside the workspace.');

const result = { passed: false, directory, binary, manifestPath, artifact: { version: manifest.version, sha256: digest(binaryBytes), bytes: binaryBytes.length }, records: [],
  limits: ['One preserved Windows debug binary, one loopback HTTP origin and two benign domain commands; no universal WebView isolation claim.',
    'CDP controls navigation for this test; it is not an attacker capability assumed by the product.',
    'The actual runtime-injected bridge is used; no invoke key or function is copied from the trusted origin.',
    'Forced IPC fetch rejection tests Tauri\'s real postMessage fallback, not a mocked permission response.',
    'Startup can inspect WSL registration metadata; the isolated environment has no owned distro, and no WSL executable/tool/maintenance is requested.',
    'No real provider credential, provider API, personal profile, secondary-window app command, screen reader or hostile target code is tested.'] };
const diagnostics = [], requests = [], responses = [];
let child, browser, page, server, failure;
const owned = path => {
  const candidate = resolve(path), sub = relative(directory, candidate);
  if (!sub || isAbsolute(sub) || sub === '..' || sub.startsWith(`..${sep}`)) throw new Error('Artifact escaped the owned fixture.');
  return candidate;
};
function snapshotDatabase() {
  const core = new DatabaseSync(owned(join(dataDirectory, 'openkoda.sqlite3')), { readOnly: true });
  const ai = new DatabaseSync(owned(join(dataDirectory, 'assistant.sqlite3')), { readOnly: true });
  try {
    const json = core.prepare('SELECT json FROM app_state WHERE id=1').get().json;
    const state = JSON.parse(json);
    return { coreSha256: digest(json), projects: state.projects.length, assessments: state.assessments.length,
      findings: state.findings.length, projectNames: state.projects.map(p => p.name),
      providerRequests: ai.prepare('SELECT count(*) AS count FROM requests').get().count,
      vaultEntries: ai.prepare('SELECT count(*) AS count FROM vault').get().count };
  } finally { core.close(); ai.close(); }
}
async function unusedPort() {
  const socket = createPortServer();
  await new Promise((ok, bad) => { socket.once('error', bad); socket.listen(0, '127.0.0.1', ok); });
  const port = socket.address().port;
  await new Promise((ok, bad) => socket.close(error => error ? bad(error) : ok()));
  return port;
}
async function record(id, task) {
  const entry = { id, status: 'running', startedAt: new Date().toISOString() }; result.records.push(entry);
  try { entry.detail = await task(); entry.status = 'passed'; }
  catch (error) { entry.status = 'failed'; entry.error = String(error); throw error; }
  finally { entry.endedAt = new Date().toISOString(); }
}

// This function is served as a normal script from the adversarial origin.
// It only uses the bridge injected into that document by the actual runtime.
function adversarialProbe(configuration) {
  const state = { done: false, origin: location.origin, attempts: [], fetches: [], forcedFetchRejections: 0 };
  window.originProbe = state;
  const internal = window.__TAURI_INTERNALS__;
  state.bridge = { internalKeys: internal ? Object.getOwnPropertyNames(internal).sort() : [],
    invoke: typeof internal?.invoke, transformCallback: typeof internal?.transformCallback,
    postMessage: typeof internal?.postMessage, rawIpcPostMessage: typeof window.ipc?.postMessage,
    webviewPostMessage: typeof window.chrome?.webview?.postMessage };
  const originalFetch = window.fetch.bind(window);
  let forceFallback = false;
  window.fetch = async (input, options) => {
    const url = new URL(typeof input === 'string' ? input : input.url, location.href);
    if (url.hostname !== 'ipc.localhost') return originalFetch(input, options);
    const item = { url: url.toString(), method: options?.method, headers: Array.from(new Headers(options?.headers).keys()) };
    state.fetches.push(item);
    if (forceFallback) { state.forcedFetchRejections++; item.forcedRejection = true; throw new TypeError('Owned test forces Tauri postMessage fallback'); }
    const response = await originalFetch(input, options);
    item.status = response.status; item.tauriResponse = response.headers.get('Tauri-Response');
    item.body = (await response.clone().text()).slice(0, 4096);
    return response;
  };
  const operations = [
    { action: 'snapshot', payload: {} },
    { action: 'project.create', payload: { name: configuration.name, kind: 'code', target: configuration.target, description: 'Benign synthetic write from an unauthorized origin.' } },
  ];
  async function attempt(operation, transport) {
    let timer;
    const entry = { action: operation.action, transport, status: 'running', startedAt: new Date().toISOString() };
    state.attempts.push(entry);
    try {
      const value = await Promise.race([
        internal.invoke('request', { request: operation }),
        new Promise((_, reject) => { timer = setTimeout(() => reject(new Error('IPC_RESULT_TIMEOUT_NOT_A_PERMISSION_DENIAL')), 12000); }),
      ]);
      entry.status = 'unexpectedly-allowed';
      entry.resultShape = value && typeof value === 'object' ? Object.keys(value) : typeof value;
    } catch (error) {
      entry.error = String(error);
      entry.status = /request.*not allowed|not allowed.*request|Command request not allowed by ACL/is.test(entry.error) ? 'explicitly-denied' : 'inconclusive';
    } finally { clearTimeout(timer); entry.endedAt = new Date().toISOString(); }
  }
  (async () => {
    if (typeof internal?.invoke !== 'function') {
      state.limitation = 'Runtime did not expose its authenticated invoke API in this document; absence alone is not a demonstrated ACL rejection.';
      return;
    }
    for (const operation of operations) await attempt(operation, 'runtime-default');
    forceFallback = true;
    for (const operation of operations) await attempt(operation, 'runtime-postmessage-fallback');
  })().catch(error => { state.error = String(error); }).finally(() => {
    state.done = true;
    document.querySelector('#result').textContent = JSON.stringify(state, null, 2);
  });
}

try {
  const attackerName = `UNAUTHORIZED_ORIGIN_${randomUUID()}`;
  const script = `(${adversarialProbe.toString()})(${JSON.stringify({ target, name: attackerName })});`;
  const html = '<!doctype html><html lang="en"><meta charset="utf-8"><title>Owned OpenKoda origin probe</title><h1>Untrusted loopback origin</h1><p>Only benign synthetic requests are attempted. No real data or provider.</p><pre id="result">Probe pending</pre><script src="/probe.js"></script></html>';
  await writeFile(join(directory, 'adversary.html'), html);
  await writeFile(join(directory, 'probe.js'), script);
  server = createHttpServer((request, response) => {
    requests.push({ method: request.method, url: request.url, host: request.headers.host });
    response.setHeader('Cache-Control', 'no-store');
    response.setHeader('Content-Security-Policy', "default-src 'none'; script-src 'self'; connect-src http://ipc.localhost ipc:; style-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'");
    if (request.method !== 'GET') { response.writeHead(405); response.end(); return; }
    if (request.url === '/') { response.setHeader('Content-Type', 'text/html; charset=utf-8'); response.end(html); }
    else if (request.url === '/probe.js') { response.setHeader('Content-Type', 'application/javascript; charset=utf-8'); response.end(script); }
    else { response.writeHead(404); response.end(); }
  });
  await new Promise((ok, bad) => { server.once('error', bad); server.listen(0, '127.0.0.1', ok); });
  const adversarialUrl = `http://127.0.0.1:${server.address().port}/`;
  result.adversarialOrigin = new URL(adversarialUrl).origin;
  const port = await unusedPort();
  child = spawn(binary, [], { cwd: dirname(binary), windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'],
    env: { ...process.env, OPENKODA_DATA_DIR: dataDirectory, WEBVIEW2_USER_DATA_FOLDER: cacheDirectory,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-address=127.0.0.1 --remote-debugging-port=${port}` } });
  result.ownedPid = child.pid;
  child.stdout.on('data', bytes => diagnostics.push(`stdout: ${bytes}`));
  child.stderr.on('data', bytes => diagnostics.push(`stderr: ${bytes}`));
  child.on('error', error => { failure = error; });
  await expect.poll(async () => {
    if (failure) throw failure;
    if (child.exitCode !== null) throw new Error(`Owned app exited: ${child.exitCode}`);
    try { return (await fetch(`http://127.0.0.1:${port}/json/version`, { signal: AbortSignal.timeout(1000) })).ok; } catch { return false; }
  }, { timeout: 45000, message: 'Owned preserved WebView CDP becomes ready' }).toBe(true);
  browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  const context = browser.contexts()[0];
  await expect.poll(() => context.pages().length, { timeout: 15000 }).toBeGreaterThan(0);
  page = context.pages()[0]; page.setDefaultTimeout(15000); page.setDefaultNavigationTimeout(30000);
  page.on('console', message => diagnostics.push(`console ${message.type()}: ${message.text()}`));
  page.on('pageerror', error => diagnostics.push(`pageerror: ${error.message}`));
  await context.tracing.start({ screenshots: true, snapshots: true, sources: true });
  await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
  await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
  const trustedUrl = page.url(); result.trustedOrigin = new URL(trustedUrl).origin;
  expect(result.trustedOrigin).not.toBe(result.adversarialOrigin);
  await context.route('**/*', async route => {
    const url = new URL(route.request().url());
    if ([result.trustedOrigin, result.adversarialOrigin, 'http://ipc.localhost', 'ipc://localhost'].includes(url.origin)) await route.continue();
    else { diagnostics.push(`blocked unrelated URL: ${url.origin}`); await route.abort('blockedbyclient'); }
  });
  page.on('response', response => { if (response.url().includes('ipc.localhost')) responses.push({ url: response.url(), status: response.status() }); });
  const localRequest = (action, payload = {}) => page.evaluate(({ action, payload }) => window.__TAURI_INTERNALS__.invoke('request', { request: { action, payload } }), { action, payload });
  await record('trusted-positive-control', async () => {
    expect((await localRequest('snapshot')).projects).toHaveLength(0);
    const created = await localRequest('project.create', { name: 'Trusted origin baseline', kind: 'code', target, description: 'Owned positive control.' });
    expect(created.name).toBe('Trusted origin baseline');
    const baseline = snapshotDatabase(); expect(baseline.projects).toBe(1); expect(baseline.providerRequests).toBe(0); expect(baseline.vaultEntries).toBe(0);
    result.before = baseline;
    await writeFile(join(directory, 'before.json'), JSON.stringify(baseline, null, 2));
    await page.screenshot({ path: join(directory, 'trusted-before.png'), fullPage: true });
    return { command: 'request', actions: ['snapshot', 'project.create'], projectCreated: true, origin: result.trustedOrigin };
  });
  await record('untrusted-actual-bridge', async () => {
    await page.goto(adversarialUrl, { waitUntil: 'domcontentloaded' });
    expect(new URL(page.url()).origin).toBe(result.adversarialOrigin);
    await page.waitForFunction(() => window.originProbe?.done === true, undefined, { timeout: 60000 });
    const probe = await page.evaluate(() => window.originProbe);
    result.probe = probe;
    await writeFile(join(directory, 'probe-result.json'), JSON.stringify(probe, null, 2));
    await page.screenshot({ path: join(directory, 'untrusted-result.png'), fullPage: true });
    expect(probe.bridge.invoke, 'Do not certify an ACL denial merely because a bridge was absent').toBe('function');
    expect(probe.attempts).toHaveLength(4);
    expect(probe.attempts.map(attempt => attempt.status)).toEqual(Array(4).fill('explicitly-denied'));
    expect(probe.forcedFetchRejections, 'Actual runtime fallback was exercised').toBeGreaterThan(0);
    expect(probe.fetches.some(item => item.tauriResponse === 'error'), 'Custom protocol delivered a real error response').toBe(true);
    const after = snapshotDatabase(); result.afterAdversarial = after;
    expect(after).toEqual(result.before); expect(after.projectNames).not.toContain(attackerName);
    return { origin: probe.origin, authenticatedBridgePresent: true, explicitDenials: 4,
      customProtocolErrorObserved: true, runtimePostMessageFallbackObserved: true, databaseUnchanged: true };
  });
  await record('trusted-return-and-database-preservation', async () => {
    await page.goto(trustedUrl, { waitUntil: 'domcontentloaded' });
    await page.waitForFunction(() => typeof window.__TAURI_INTERNALS__?.invoke === 'function');
    await expect(page.locator('#main-content')).toHaveAttribute('aria-busy', 'false', { timeout: 45000 });
    expect((await localRequest('snapshot')).projects).toHaveLength(1);
    result.after = snapshotDatabase(); expect(result.after).toEqual(result.before);
    await page.screenshot({ path: join(directory, 'trusted-after.png'), fullPage: true });
    return { trustedBridgeStillFunctional: true, coreUnchanged: true, providerRequests: 0, vaultEntries: 0 };
  });
  result.passed = true;
} catch (error) { failure = error; result.error = String(error); }
finally {
  if (browser) {
    await browser.contexts()[0]?.tracing.stop({ path: join(directory, 'trace.zip') }).catch(error => diagnostics.push(`trace: ${error}`));
    await browser.close().catch(error => diagnostics.push(`CDP close: ${error}`));
  }
  if (child && child.exitCode === null && child.signalCode === null) {
    child.kill();
    await expect.poll(() => child.exitCode !== null || child.signalCode !== null, { timeout: 10000 }).toBe(true).catch(error => { failure ??= error; result.passed = false; result.cleanupError = String(error); });
  }
  result.ownedProcessClosed = !child || child.exitCode !== null || child.signalCode !== null;
  if (server?.listening) await new Promise((ok, bad) => { server.close(error => error ? bad(error) : ok()); server.closeAllConnections(); });
  result.ownedServerClosed = !server?.listening;
  if (result.before && result.ownedProcessClosed) {
    try { result.afterClose = snapshotDatabase(); expect(result.afterClose).toEqual(result.before); }
    catch (error) { failure ??= error; result.passed = false; result.databasePreservationError = String(error); }
  }
  result.generatedAt = new Date().toISOString();
  await writeFile(join(directory, 'diagnostics.log'), diagnostics.join('\n'));
  await writeFile(join(directory, 'http-requests.json'), JSON.stringify(requests, null, 2));
  await writeFile(join(directory, 'ipc-responses.json'), JSON.stringify(responses, null, 2));
  await writeFile(join(directory, 'result.json'), JSON.stringify(result, null, 2));
  console.log(JSON.stringify({ passed: result.passed, directory, records: result.records.map(({ id, status }) => ({ id, status })), error: result.error,
    ownedProcessClosed: result.ownedProcessClosed, ownedServerClosed: result.ownedServerClosed }, null, 2));
}
if (failure || !result.passed) process.exitCode = 1;
