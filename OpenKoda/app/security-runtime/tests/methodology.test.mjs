import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';

const root = new URL('../', import.meta.url);
const upstream = 'node_modules/@openai/codex-security/';
const originals = {
  'attack-path-facts.md': [
    '_bundled_plugin/skills/attack-path-analysis/references/attack-path-facts.md',
    '1979ea81991ba1c095c1846d4b8e09429edc6295912f87a6ebb7fa7e32651a1e',
  ],
  'LICENSE': ['LICENSE', 'd17f227e4df5da1600391338865ce0f3055211760a36688f816941d58232d8dc'],
};

test('embedded review guidance and license match the pinned official SDK', async () => {
  const metadata = JSON.parse(await readFile(new URL(upstream + 'package.json', root), 'utf8'));
  assert.equal(metadata.version, '0.10.0');
  assert.equal(metadata.license, 'Apache-2.0');
  for (const [name, [source, digest]] of Object.entries(originals)) {
    const [copy, original] = await Promise.all([
      readFile(new URL('methodology/' + name, root)),
      readFile(new URL(upstream + source, root)),
    ]);
    assert.equal(createHash('sha256').update(copy).digest('hex'), digest);
    assert.deepEqual(copy, original);
  }
});
