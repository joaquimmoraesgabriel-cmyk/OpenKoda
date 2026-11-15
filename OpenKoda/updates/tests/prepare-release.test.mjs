import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, readFile, rm, readdir } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { prepareRelease, assertNativeVerificationOutput } from '../scripts/prepare-release.mjs';

test('native verifier must actually run its single signature test', () => {
  assert.throws(() => assertNativeVerificationOutput('test result: ok. 0 passed; 0 failed; 0 ignored; 4 filtered out;'), /did not run/);
  assert.throws(() => assertNativeVerificationOutput('test updater_verify_packaged_candidate ... ignored\ntest result: ok. 0 passed; 0 failed; 1 ignored;'), /did not run/);
  assertNativeVerificationOutput('running 1 test\ntest updater_verify_packaged_candidate ... ok\ntest result: ok. 1 passed; 0 failed; 0 ignored; 3 filtered out;');
});

async function fixture(t) {
  const root = await mkdtemp(join(tmpdir(), 'koda-release-')); t.after(() => rm(root, { recursive: true, force: true }));
  const releaseRoot = join(root, 'release'); await mkdir(join(releaseRoot, 'bundle', 'nsis'), { recursive: true });
  const path = join(releaseRoot, 'bundle', 'nsis', 'OpenKoda_1.1.2_x64-setup.exe');
  const payload = Buffer.from('fixture signed installer'), sha256 = createHash('sha256').update(payload).digest('hex');
  await writeFile(path, payload); await writeFile(path + '.sig', Buffer.from('fixture signature content; native verification injected only by tests').toString('base64'));
  const receiptPath = join(root, 'receipt.json'), output = join(root, 'prepared');
  const receipt = { profile: 'release', version: '1.1.2', updaterSigned: true, isolatedUpdateTest: false, releaseRoot,
    files: [{ path, bytes: payload.length, sha256, updaterSignature: path + '.sig', authenticode: 'NotSigned' }] };
  const save = () => writeFile(receiptPath, JSON.stringify(receipt)); await save();
  return { receipt, save, path, output, options: { receiptPath, output, channel: 'stable', now: new Date('2026-11-14T21:00:00Z') } };
}
test('prepares immutable hashes and a paused control only after signature verification', async t => {
  const f = await fixture(t); let verified = false;
  const result = await prepareRelease({ ...f.options, verifySignature: async ({ path, version }) => {
    assert.equal(path, f.path); assert.equal(version, '1.1.2'); verified = true;
  } });
  assert.equal(verified, true); assert.equal(result.activated, false); assert.equal(result.authenticode, 'NotSigned');
  const plan = JSON.parse(await readFile(result.planPath, 'utf8'));
  assert.equal(plan.uploads[0].key, `artifacts/${result.artifactSha256}/OpenKoda_1.1.2_x64-setup.exe`);
  const release = await readFile(plan.uploads[1].source);
  assert.equal(createHash('sha256').update(release).digest('hex'), result.releaseSha256);
  const control = JSON.parse(await readFile(plan.uploads[2].source, 'utf8'));
  assert.equal(control.enabled, false); assert.equal(control.rolloutPercent, 0);
  assert.equal(control.release, result.releaseSha256);
});
test('corrupt installer is rejected before native verification or publication output', async t => {
  const f = await fixture(t); await writeFile(f.path, 'changed');
  await assert.rejects(prepareRelease({ ...f.options, verifySignature: async () => assert.fail('must not reach verification') }), /differs/);
  await assert.rejects(readdir(f.output), { code: 'ENOENT' });
});
test('failed signature and version verification produces no publication output', async t => {
  const f = await fixture(t);
  await assert.rejects(prepareRelease({ ...f.options, verifySignature: async () => { throw Error('invalid signature'); } }), /invalid signature/);
  await assert.rejects(readdir(f.output), { code: 'ENOENT' });
});
test('replacement during verification is rejected', async t => {
  const f = await fixture(t);
  await assert.rejects(prepareRelease({ ...f.options, verifySignature: async () => writeFile(f.path, 'replacement') }), /changed during/);
});
test('test harness and unsigned artifacts cannot become public releases', async t => {
  const f = await fixture(t); f.receipt.isolatedUpdateTest = true; await f.save();
  await assert.rejects(prepareRelease({ ...f.options, verifySignature: async () => {} }), /production-profile/);
  f.receipt.isolatedUpdateTest = false; f.receipt.updaterSigned = false; await f.save();
  await assert.rejects(prepareRelease({ ...f.options, verifySignature: async () => {} }), /production-profile/);
});
test('unknown channels and receipt path substitutions fail closed', async t => {
  const f = await fixture(t);
  await assert.rejects(prepareRelease({ ...f.options, channel: 'ultra', verifySignature: async () => {} }), /Choose/);
  f.receipt.files[0].updaterSignature = join(f.output, 'other.sig'); await f.save();
  await assert.rejects(prepareRelease({ ...f.options, verifySignature: async () => {} }), /does not match/);
});
