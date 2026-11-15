import { createHash, randomUUID } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { lstat, readFile, writeFile, mkdir, realpath } from 'node:fs/promises';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { resolve, dirname, join, basename, isAbsolute } from 'node:path';
import { fileURLToPath } from 'node:url';
import { MAX_DOWNLOAD, CHANNELS, validateRelease, sha256 } from '../policy.mjs';

const execute = promisify(execFile);
const appRoot = fileURLToPath(new URL('../../app/', import.meta.url));
const assert = (ok, message) => { if (!ok) throw new Error(message); };
async function plainFile(path, max) {
  const stat = await lstat(path);
  assert(stat.isFile() && !stat.isSymbolicLink() && stat.size > 0 && stat.size <= max, 'Expected a bounded regular file.');
  return stat;
}
async function hashFile(path) {
  const hash = createHash('sha256'); let size = 0;
  for await (const chunk of createReadStream(path)) {
    size += chunk.length; assert(size <= MAX_DOWNLOAD, 'Installer exceeds the update limit.'); hash.update(chunk);
  }
  return { size, sha256: hash.digest('hex') };
}
async function verifyNative({ path, version, cargoConfig }) {
  // Verify with the same production key and version-bound Tauri implementation
  // used by the app. This needs no private key and makes no API call.
  try {
    const result = await execute('cargo', ['test', ...(cargoConfig ? ['--config', cargoConfig] : []), '--manifest-path', join(appRoot, 'src-tauri/Cargo.toml'), '--locked', '--offline',
      '--test', 'updater_signatures', 'updater_verify_packaged_candidate', '--', '--ignored', '--exact'], {
      cwd: appRoot, windowsHide: true, timeout: 15 * 60 * 1000, maxBuffer: 4 * 1024 * 1024,
      env: { ...process.env, OPENKODA_VERIFY_UPDATE_FILE: path, OPENKODA_VERIFY_UPDATE_VERSION: version },
    });
    assertNativeVerificationOutput(result.stdout);
  } catch { throw new Error('Production updater signature/version verification failed; no release was prepared.'); }
}
export function assertNativeVerificationOutput(stdout) {
  assert(/^test updater_verify_packaged_candidate \.\.\. ok\r?$/m.test(stdout)
    && /test result: ok\. 1 passed; 0 failed; 0 ignored;/.test(stdout), 'Native verification did not run exactly one passing signature test.');
}
async function writeImmutable(path, bytes) {
  try { await writeFile(path, bytes, { flag: 'wx' }); }
  catch (error) {
    if (error.code !== 'EEXIST') throw error;
    assert((await readFile(path)).equals(Buffer.from(bytes)), 'Refusing to overwrite an existing publication file.');
  }
}

export async function prepareRelease({ receiptPath, channel, output, notesFile, cargoConfig, now = new Date(), verifySignature = verifyNative }) {
  assert(CHANNELS.has(channel), 'Choose stable, early or experimental.');
  if (cargoConfig) { cargoConfig = resolve(cargoConfig); await plainFile(cargoConfig, 64 * 1024); }
  await plainFile(receiptPath, 1024 * 1024);
  const receipt = JSON.parse((await readFile(receiptPath, 'utf8')).replace(/^\uFEFF/, ''));
  assert(receipt.profile === 'release' && receipt.updaterSigned === true && receipt.isolatedUpdateTest === false,
    'A signed production-profile installer is required.');
  assert(Array.isArray(receipt.files) && receipt.files.length === 1 && typeof receipt.version === 'string', 'Expected one installer.');
  const file = receipt.files[0];
  assert(typeof file.path === 'string' && isAbsolute(file.path) && typeof receipt.releaseRoot === 'string' && isAbsolute(receipt.releaseRoot), 'Invalid package paths.');
  const filename = `OpenKoda_${receipt.version}_x64-setup.exe`;
  assert(basename(file.path) === filename && file.updaterSignature === file.path + '.sig', 'Installer/signature name does not match the version.');
  const installPath = resolve(file.path), releaseRoot = await realpath(receipt.releaseRoot);
  const expectedDirectory = join(releaseRoot, 'bundle', 'nsis');
  assert(await realpath(dirname(installPath)) === expectedDirectory, 'Installer must belong to the packaging receipt directory.');
  await plainFile(installPath, MAX_DOWNLOAD); await plainFile(file.updaterSignature, 8192);
  const actual = await hashFile(installPath);
  assert(actual.sha256 === file.sha256 && actual.size === file.bytes, 'Installer differs from the packaging receipt.');
  const signature = (await readFile(file.updaterSignature, 'utf8')).trim();
  let notes = 'Melhorias no aplicativo e preparação dos canais de atualização.';
  if (notesFile) { await plainFile(notesFile, 8000); notes = (await readFile(notesFile, 'utf8')).trim(); }
  const release = { schema: 'openkoda.update-release.v1', channel, version: receipt.version,
    pub_date: now.toISOString(), notes,
    platforms: { 'windows-x86_64': { filename, ...actual, signature } } };
  validateRelease(release, channel, now.getTime());
  await verifySignature({ path: installPath, version: receipt.version, cargoConfig });
  // Catch a replacement during verification; native installation still verifies
  // the signature again even after transport and publishing checks have passed.
  const final = await hashFile(installPath);
  assert(final.sha256 === actual.sha256 && final.size === actual.size && (await readFile(file.updaterSignature, 'utf8')).trim() === signature,
    'Installer or signature changed during verification.');
  const bytes = JSON.stringify(release, null, 2) + '\n', digest = await sha256(bytes);
  const out = resolve(output); await mkdir(out, { recursive: true });
  const control = { schema: 'openkoda.update-channel.v1', enabled: false, release: digest,
    rolloutPercent: 0, rolloutSeed: randomUUID() };
  await mkdir(join(out, 'releases'), { recursive: true });
  const releaseFile = join(out, 'releases', digest + '.json');
  await writeImmutable(releaseFile, bytes);
  const controlFile = join(out, `${channel}-${digest}.paused.json`);
  // Never overwrite a staged control or turn a channel on in a build script.
  let prior;
  try { prior = JSON.parse(await readFile(controlFile, 'utf8')); } catch (e) { if (e.code !== 'ENOENT') throw e; }
  if (prior) {
    assert(prior.schema === control.schema && prior.release === digest && prior.enabled === false && prior.rolloutPercent === 0,
      'Existing staged control differs; use a new staging directory.');
  } else await writeImmutable(controlFile, JSON.stringify(control, null, 2) + '\n');
  const plan = { schema: 'openkoda.update-upload.v1', channel, version: receipt.version, updaterSignatureVerified: true,
    authenticode: file.authenticode ?? 'Unknown', activated: false,
    uploads: [
      { source: installPath, key: `artifacts/${actual.sha256}/${filename}`, sha256: actual.sha256, bytes: actual.size, customMetadata: { sha256: actual.sha256 } },
      { source: releaseFile, key: `releases/${digest}.json`, sha256: digest, bytes: Buffer.byteLength(bytes) },
      { source: controlFile, key: `channels/${channel}.json`, activate: false },
    ] };
  const planPath = join(out, `upload-${channel}-${digest}.json`);
  await writeImmutable(planPath, JSON.stringify(plan, null, 2) + '\n');
  return { version: receipt.version, channel, releaseSha256: digest, artifactSha256: actual.sha256, bytes: actual.size,
    authenticode: plan.authenticode, updaterSignatureVerified: true, activated: false, planPath };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const args = process.argv.slice(2), options = new Map();
  try {
    assert(args.length % 2 === 0, 'Provide --receipt, --channel, --output and optional --notes-file or --cargo-config.');
    for (let i = 0; i < args.length; i += 2) {
      assert(['--receipt', '--channel', '--output', '--notes-file', '--cargo-config'].includes(args[i]) && !options.has(args[i]), 'Unknown or duplicate option.');
      options.set(args[i], args[i + 1]);
    }
    assert(options.has('--receipt') && options.has('--channel') && options.has('--output'), 'Provide --receipt, --channel and --output.');
    console.log(JSON.stringify(await prepareRelease({ receiptPath: resolve(options.get('--receipt')), channel: options.get('--channel'),
      output: resolve(options.get('--output')), notesFile: options.has('--notes-file') ? resolve(options.get('--notes-file')) : undefined,
      cargoConfig: options.has('--cargo-config') ? resolve(options.get('--cargo-config')) : undefined })));
  } catch (error) { console.error(error.message); process.exitCode = 1; }
}
