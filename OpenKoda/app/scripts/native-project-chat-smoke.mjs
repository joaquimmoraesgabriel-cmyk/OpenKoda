import assert from 'node:assert/strict';
import { createHash, randomUUID } from 'node:crypto';
import { execFile, spawn } from 'node:child_process';
import { once } from 'node:events';
import { access, mkdir, readFile, writeFile } from 'node:fs/promises';
import { createServer } from 'node:net';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { chromium, expect } from '@playwright/test';

const appRoot = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const version = JSON.parse(await readFile(join(appRoot, 'package.json'), 'utf8')).version;
const binary = resolve(appRoot, process.env.OPENKODA_SMOKE_BINARY || 'src-tauri/target/debug/openkoda.exe');
const [flag, expectedHash] = process.argv.slice(2);
assert.equal(flag, '--expected-sha256');
assert.match(expectedHash ?? '', /^[a-f0-9]{64}$/);
assert.equal(basename(binary), 'openkoda.exe');
assert.equal(basename(dirname(binary)), 'debug');
const sha256 = createHash('sha256').update(await readFile(binary)).digest('hex');
assert.equal(sha256, expectedHash);
const metadata = await promisify(execFile)('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command',
  '(Get-Item -LiteralPath $env:OPENKODA_SMOKE_BINARY).VersionInfo.ProductVersion'], {
  env: { ...process.env, OPENKODA_SMOKE_BINARY: binary }, windowsHide: true,
});
assert.equal(metadata.stdout.trim().split('+')[0], version, 'Version of executable must match this checkout');

const folder = join(appRoot, '.local', 'native-project-chat-' + randomUUID());
const data = join(folder, 'data');
await mkdir(data, { recursive: true });
async function listen(server) {
  server.listen(0, '127.0.0.1');
  await once(server, 'listening');
  return server.address().port;
}

// Exercise the normal project entry with real TCP and SQLite, entirely on loopback.
let connections = 0;
const target = createServer(socket => { connections++; socket.end(); });
const targetPort = await listen(target);
const reservation = createServer();
const port = await listen(reservation);
await new Promise(done => reservation.close(done));
const child = spawn(binary, [], {
  cwd: dirname(binary), stdio: 'ignore', windowsHide: true,
  env: { ...process.env, OPENKODA_DATA_DIR: data, WEBVIEW2_USER_DATA_FOLDER: join(folder, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: '--remote-debugging-address=127.0.0.1 --remote-debugging-port=' + port },
});
let browser;
const result = { passed: false, version, sha256, checks: [] };
try {
  const cdp = 'http://127.0.0.1:' + port;
  await expect.poll(async () => {
    try { return (await fetch(cdp + '/json/version')).ok; } catch { return false; }
  }, { timeout: 45000 }).toBe(true);
  browser = await chromium.connectOverCDP(cdp);
  const context = browser.contexts()[0];
  await expect.poll(() => context.pages().length, { timeout: 30000 }).toBe(1);
  const page = context.pages()[0];
  page.setDefaultTimeout(15000);
  await page.waitForFunction(() => location.hostname === 'tauri.localhost' && !!window.__TAURI_INTERNALS__?.invoke);
  await access(join(data, 'openkoda.sqlite3'));
  const snapshot = () => page.evaluate(() => window.__TAURI_INTERNALS__.invoke('request', { request: { action: 'snapshot', payload: {} } }));
  assert.equal((await snapshot()).projects.length, 0);
  await expect(page.getByRole('heading', { name: 'Seu primeiro projeto' })).toBeVisible();
  await page.screenshot({ path: join(folder, '01-inicio.png') });
  await page.getByText('Rede', { exact: true }).click();
  await expect(page.getByRole('radio', { name: /Rede/ })).toBeChecked();
  await page.getByRole('button', { name: 'Criar projeto', exact: true }).click();
  await page.getByLabel('IP ou rede CIDR').fill('127.0.0.1');
  await page.getByRole('switch', { name: 'Opções avançadas' }).check();
  await page.getByLabel('Portas TCP').fill(String(targetPort));
  await page.getByRole('button', { name: 'Criar projeto', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Mensagem da conversa' });
  await expect(input).toBeVisible({ timeout: 30000 });
  await expect.poll(async () => {
    const state = await snapshot();
    const plan = state.plans.find(p => p.adapter === 'network.tcp-connect.v1' &&
      p.resolvedAddresses.includes('127.0.0.1:' + targetPort));
    return !!plan && state.executions.some(e => e.planId === plan.id && e.status === 'completed' && !!e.evidenceId);
  }, { timeout: 45000 }).toBe(true);
  assert.equal(connections, 1);
  result.checks.push('Criação na tela inicial, execução TCP única e evidência persistida');
  await page.screenshot({ path: join(folder, '02-chat.png') });

  const tree = page.getByRole('complementary', { name: 'Projetos e conversas' });
  const firstChat = await tree.locator('button[aria-current="page"]').innerText();
  const draft = 'Rascunho temporário da conversa inicial';
  await input.fill(draft);
  await tree.getByRole('button', { name: 'Criar nova conversa em 127.0.0.1' }).click();
  await expect(tree.getByRole('button', { name: 'Conversa 2', exact: true })).toHaveAttribute('aria-current', 'page');
  await expect(input).toHaveValue('');
  await tree.getByRole('button', { name: firstChat, exact: true }).click();
  await expect(input).toHaveValue(draft);
  const state = await snapshot();
  assert.equal(state.projects.length, 1);
  assert.equal(state.assessments.length, 2);
  assert.equal(state.executions.length, 1);
  result.checks.push('Duas conversas, rascunho individual e uma única execução');

  const overview = page.getByRole('button', { name: 'Visão geral do projeto' });
  await overview.click();
  await page.getByRole('button', { name: 'Mostrar ao lado da conversa' }).click();
  await expect(input).toBeVisible();
  await page.screenshot({ path: join(folder, '03-chat-painel.png') });
  await page.getByRole('button', { name: 'Fechar painel' }).click();
  await expect(overview).toBeFocused();
  result.checks.push('Painel dividido junto ao chat e retorno do foco');
  result.passed = true;
} finally {
  try {
    await writeFile(join(folder, 'result.json'), JSON.stringify(result, null, 2));
    console.log(JSON.stringify({ folder, ...result }));
  } finally {
    child.kill();
    target.close();
    await browser?.close().catch(() => {});
  }
}
