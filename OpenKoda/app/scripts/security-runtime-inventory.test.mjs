import assert from 'node:assert/strict';
import { link, mkdir, mkdtemp, realpath, rm, unlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import test from 'node:test';
import { inventory } from './security-runtime-inventory.mjs';

test('inventory binds sorted paths and contents, including hidden files, and refuses hardlinks', async t => {
  const parent = await realpath(tmpdir());
  const root = await mkdtemp(join(parent, 'openkoda-runtime-inventory-'));
  t.after(async () => {
    assert.equal(dirname(await realpath(root)), parent);
    await rm(root, { recursive: true });
  });
  await mkdir(join(root, 'dir'));
  const source = join(root, 'dir', 'one');
  const hidden = join(root, '.hidden');
  await writeFile(source, 'abc');
  await writeFile(hidden, 'xyz');
  const expected = await inventory(root);
  assert.equal(expected.files, 2);
  assert.equal(expected.bytes, 6);
  assert.deepEqual(await inventory(root), expected);
  await writeFile(hidden, 'xYz');
  assert.notEqual((await inventory(root)).sha256, expected.sha256);
  await unlink(hidden);
  assert.equal((await inventory(root)).files, 1);
  await link(source, join(root, 'alias'));
  await assert.rejects(inventory(root), /linked resource/);
});
