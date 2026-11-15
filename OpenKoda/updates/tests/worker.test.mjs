import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createHandler } from '../worker.mjs';
import { sha256, MAX_DOWNLOAD, compareVersions, parseVersion, eligibleRollout } from '../policy.mjs';

const NOW = Date.parse('2026-11-14T12:00:00Z'), ORIGIN = 'https://openkoda-updates.example.workers.dev';
const TOKEN = 'eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJmaXh0dXJlIn0.fixture_signature';
const ID = '2d18c935-685d-45a5-b6fb-48f0f92d1213';
const encoded = value => new TextEncoder().encode(typeof value === 'string' ? value : JSON.stringify(value));
class Bucket {
  objects = new Map(); calls = []; changed = false;
  set(key, value, metadata = {}) {
    const bytes = value instanceof Uint8Array ? value : encoded(value);
    this.objects.set(key, { key, bytes, size: bytes.length, etag: 'fixture-etag', httpEtag: '"fixture-etag"', customMetadata: metadata });
  }
  async head(key) { this.calls.push(['head', key]); const row = this.objects.get(key); return row ? { ...row } : null; }
  async get(key, options) {
    this.calls.push(['get', key, options]); const row = this.objects.get(key); if (!row) return null;
    if (options?.onlyIf && (this.changed || options.onlyIf.etagMatches !== row.etag)) return { ...row };
    const bytes = options?.range ? row.bytes.slice(options.range.offset, options.range.offset + options.range.length) : row.bytes;
    return { ...row, body: new Response(bytes).body };
  }
}
function account(plan = 'pro', changes = {}) {
  return { plan: plan ? { id: plan } : null, subscription: plan ? { status: 'active', periodStart: '2026-11-01T00:00:00Z', periodEnd: '2026-12-01T00:00:00Z' } : null, ...changes };
}
async function fixture({ channel = 'stable', plan = 'pro', version = '1.1.2', accountBody, upstreamStatus = 200, percent = 100 } = {}) {
  const bucket = new Bucket(), bytes = encoded('signed installer fixture 0123456789'), digest = await sha256(bytes);
  const asset = { filename: `OpenKoda_${version}_x64-setup.exe`, size: bytes.length, sha256: digest, signature: 'a'.repeat(64) };
  const release = { schema: 'openkoda.update-release.v1', channel, version, pub_date: '2026-11-14T10:00:00Z', notes: 'Atualização de teste.', platforms: { 'windows-x86_64': asset } };
  const releaseDigest = await sha256(encoded(release));
  const control = { schema: 'openkoda.update-channel.v1', enabled: true, release: releaseDigest, rolloutPercent: percent, rolloutSeed: 'test-release-20261114' };
  bucket.set(`channels/${channel}.json`, control); bucket.set(`releases/${releaseDigest}.json`, release);
  bucket.set(`artifacts/${digest}/${asset.filename}`, bytes, { sha256: digest });
  const fetched = [], limited = [], upstream = { body: accountBody ?? account(plan), status: upstreamStatus };
  const env = { PUBLIC_ORIGIN: ORIGIN, SUPABASE_URL: 'https://ynjcyvnrfxsrijrwpaye.supabase.co',
    SUPABASE_PUBLISHABLE_KEY: 'sb_publishable_test_fixture_only', UPDATES_ENABLED: 'true', UPDATES_BUCKET: bucket,
    REQUEST_LIMITER: { async limit(input) { limited.push(input); return { success: true }; } } };
  const handle = createHandler({ now: () => NOW, fetcher: async (url, options) => { fetched.push({ url, options }); return new Response(JSON.stringify(upstream.body), { status: upstream.status }); } });
  const manifestPath = `/v1/updates/${channel}/windows/x86_64/1.1.1`;
  const downloadPath = `/v1/downloads/${channel}/${version}/windows/x86_64/${digest}/${asset.filename}`;
  function request(path = manifestPath, options = {}) {
    const headers = { 'cf-connecting-ip': '192.0.2.10', 'x-koda-installation-id': ID,
      ...(channel !== 'stable' ? { authorization: `Bearer ${TOKEN}` } : {}),
      ...(channel === 'experimental' ? { 'x-koda-experimental': 'true' } : {}), ...options.headers };
    for (const key of Object.keys(headers)) if (headers[key] === null) delete headers[key];
    return handle(new Request(path.startsWith('https:') ? path : ORIGIN + path, { ...options, headers }), env);
  }
  async function publish(replacement) {
    const hash = await sha256(encoded(replacement)); bucket.set(`releases/${hash}.json`, replacement);
    control.release = hash; bucket.set(`channels/${channel}.json`, control);
  }
  return { bucket, bytes, asset, release, control, env, fetched, limited, upstream, request, publish, manifestPath, downloadPath };
}

test('anonymous stable returns a same-origin dynamic Tauri manifest with unchanged signature', async () => {
  const f = await fixture(), result = await f.request(), body = await result.json();
  assert.equal(result.status, 200); assert.equal(body.version, '1.1.2'); assert.equal(body.signature, f.asset.signature);
  assert.equal(body.url, ORIGIN + f.downloadPath); assert.equal(body.rolloutPercent, 100);
  assert.equal(f.fetched.length, 0); assert.equal(result.headers.get('cache-control'), 'no-store, private');
});
test('equal/newer versions and build metadata never produce a downgrade', async () => {
  const f = await fixture();
  for (const version of ['1.1.2', '1.1.2+build', '1.1.3', '2.0.0']) assert.equal((await f.request(f.manifestPath.replace('1.1.1', version))).status, 204);
  assert.equal((await f.request(f.manifestPath.replace('1.1.1', '1.1.2-beta.1'))).status, 200);
});
test('semantic version parsing orders prereleases and rejects malformed/oversized numeric versions', () => {
  assert.equal(compareVersions('1.2.0-beta.2', '1.2.0-beta.10'), -1);
  assert.equal(compareVersions('1.2.0-alpha', '1.2.0-1'), 1);
  assert.equal(compareVersions('1.2.0', '1.2.0-rc.1'), 1);
  for (const value of ['01.2.0', '1.2', 'v1.2.0', '1.2.0-01', '9007199254740992.0.0', '../1.2.0']) assert.equal(parseVersion(value), null);
});
for (const channel of ['early', 'experimental']) for (const plan of [null, 'plus', 'pro', 'ultra']) {
  test(`${channel} checks real financial plan ${plan ?? 'free'} on the server`, async () => {
    const f = await fixture({ channel, plan }), allowed = plan === 'ultra' || channel === 'early' && plan === 'pro';
    const result = await f.request(undefined, { headers: { 'x-plan': 'ultra', 'x-user-id': 'attacker', 'x-subscription': 'active' } });
    assert.equal(result.status, allowed ? 200 : 403); assert.equal(f.fetched.length, 1);
    const call = f.fetched[0]; assert.equal(call.url, f.env.SUPABASE_URL + '/functions/v1/openkoda');
    assert.deepEqual(JSON.parse(call.options.body), { action: 'account' }); assert.equal(call.options.redirect, 'error');
    assert.equal(call.options.headers.authorization, `Bearer ${TOKEN}`); assert.equal(call.options.headers['x-plan'], undefined);
  });
}
test('simulated ultra and JWT/user metadata entitlements cannot unlock private channels', async () => {
  const f = await fixture({ channel: 'experimental', accountBody: { ...account(null), simulation: { status: 'active', plan: 'ultra' }, entitlements: { hostedInference: true }, user_metadata: { plan: 'ultra' } } });
  assert.equal((await f.request()).status, 403);
});
test('subscription expiry, future start, non-production modes and inactive states deny', async () => {
  const f = await fixture({ channel: 'early' });
  for (const patch of [{ periodEnd: new Date(NOW).toISOString() }, { periodStart: new Date(NOW + 1).toISOString() }, { status: 'expired' }, { mode: 'internal' }, { periodEnd: 'invalid' }]) {
    f.upstream.body = account('ultra'); Object.assign(f.upstream.body.subscription, patch);
    assert.equal((await f.request()).status, 403);
  }
});
test('experimental requires explicit opt-in, which alone cannot upgrade a pro account', async () => {
  const f = await fixture({ channel: 'experimental', plan: 'ultra' });
  assert.equal((await f.request(undefined, { headers: { 'x-koda-experimental': null } })).status, 403);
  assert.equal(f.fetched.length, 0);
  assert.equal((await f.request(undefined, { headers: { 'x-koda-experimental': 'false' } })).status, 403);
});
test('absent, malformed, revoked and expired authentication never exposes releases', async () => {
  const f = await fixture({ channel: 'early' });
  for (const auth of [null, 'Bearer forged', 'Bearer ' + 'x'.repeat(17000)]) assert.equal((await f.request(undefined, { headers: { authorization: auth } })).status, 401);
  assert.equal(f.bucket.calls.length, 0);
  f.upstream.status = 401; assert.equal((await f.request()).status, 401);
  assert.equal(f.bucket.calls.length, 0);
});
test('downloads reauthenticate and reevaluate plan after check; there is no entitlement cache', async () => {
  const f = await fixture({ channel: 'early' }); assert.equal((await f.request()).status, 200);
  f.upstream.body = account('plus'); assert.equal((await f.request(f.downloadPath)).status, 403);
  f.upstream.body = account('pro'); assert.equal((await f.request(f.downloadPath)).status, 200);
  f.upstream.status = 401; assert.equal((await f.request(f.downloadPath, { method: 'HEAD' })).status, 401);
  assert.equal(f.fetched.length, 4);
});
test('parallel users never share account results or release authorization', async () => {
  const f = await fixture({ channel: 'early' });
  const shared = createHandler({ now: () => NOW, fetcher: async (_url, options) => {
    await new Promise(resolve => setTimeout(resolve, options.headers.authorization.endsWith('paid') ? 10 : 1));
    return Response.json(account(options.headers.authorization.endsWith('paid') ? 'pro' : 'plus'));
  } });
  const requests = Array.from({ length: 12 }, (_, i) => new Request(ORIGIN + f.downloadPath, {
    headers: { 'cf-connecting-ip': '192.0.2.10', authorization: 'Bearer head.body.' + (i % 2 ? 'paid' : 'free') },
  }));
  const results = await Promise.all(requests.map(request => shared(request, f.env)));
  assert.deepEqual(results.map(result => result.status), Array.from({ length: 12 }, (_, i) => i % 2 ? 200 : 403));
});
test('account service failures and oversized upstream JSON fail closed', async () => {
  const f = await fixture({ channel: 'early', upstreamStatus: 503 });
  assert.equal((await f.request()).status, 503); assert.equal(f.bucket.calls.length, 0);
  f.upstream.status = 200; f.upstream.body = { padding: 'x'.repeat(66000), ...account('ultra') };
  assert.equal((await f.request()).status, 503); assert.equal(f.bucket.calls.length, 0);
});
test('global emergency pause works even before R2/rate limit setup and cannot trigger manifest fallback', async () => {
  const f = await fixture(); f.env.UPDATES_ENABLED = 'false'; delete f.env.UPDATES_BUCKET; delete f.env.REQUEST_LIMITER;
  const result = await f.request(); assert.equal(result.status, 204); assert.equal(result.headers.get('x-koda-updates-paused'), 'true');
  assert.equal((await f.request(f.downloadPath)).status, 503); assert.equal(f.fetched.length, 0);
});
test('missing and disabled channels return no update; malformed control fails closed', async () => {
  const f = await fixture(); f.bucket.objects.delete('channels/stable.json'); assert.equal((await f.request()).status, 204);
  f.bucket.set('channels/stable.json', { ...f.control, enabled: false }); assert.equal((await f.request()).status, 204);
  f.bucket.set('channels/stable.json', { ...f.control, rolloutPercent: 101 }); assert.equal((await f.request()).status, 503);
  f.bucket.set('channels/stable.json', { ...f.control, release: '../../other' }); assert.equal((await f.request()).status, 503);
});
test('rollout is deterministic, monotonic, requires a valid installation ID for partial cohorts', async () => {
  const f = await fixture({ percent: 50 }); const expected = await eligibleRollout(f.control, ID);
  assert.equal((await f.request()).status, expected ? 200 : 204);
  assert.equal((await f.request()).status, expected ? 200 : 204);
  assert.equal((await f.request(undefined, { headers: { 'x-koda-installation-id': null } })).status, 204);
  let admitted = false;
  for (let percent = 0; percent <= 100; percent++) { const next = await eligibleRollout({ ...f.control, rolloutPercent: percent }, ID); if (admitted) assert.ok(next); admitted ||= next; }
});
test('tampering immutable manifest content, channel or digest is detected', async () => {
  const f = await fixture(); f.bucket.set(`releases/${f.control.release}.json`, { ...f.release, notes: 'tampered' }); assert.equal((await f.request()).status, 503);
  await f.publish({ ...f.release, channel: 'experimental' }); assert.equal((await f.request()).status, 503);
});
test('untrusted URL/key metadata, oversized artifacts, invalid dates/signatures and stable prerelease fail closed', async () => {
  const f = await fixture();
  for (const patch of [{ size: MAX_DOWNLOAD + 1 }, { size: -1 }, { filename: '../other.exe' }, { url: 'https://evil.example/download' }, { key: 'secrets' }, { signature: 'not a signature' }]) {
    await f.publish({ ...f.release, platforms: { 'windows-x86_64': { ...f.asset, ...patch } } }); assert.equal((await f.request()).status, 503);
  }
  await f.publish({ ...f.release, version: '1.2.0-beta' }); assert.equal((await f.request()).status, 503);
  await f.publish({ ...f.release, pub_date: '2099-01-01T00:00:00Z' }); assert.equal((await f.request()).status, 503);
});
test('oversized R2 metadata and unsupported release platforms fail closed', async () => {
  const f = await fixture(); await f.publish({ ...f.release, notes: 'x'.repeat(9000) }); assert.equal((await f.request()).status, 503);
  await f.publish({ ...f.release, platforms: { 'web-x86_64': f.asset } }); assert.equal((await f.request()).status, 503);
  f.bucket.set('channels/stable.json', ' '.repeat(8193)); assert.equal((await f.request()).status, 503);
});
test('full download streams exactly the pinned artifact with safe headers', async () => {
  const f = await fixture(), result = await f.request(f.downloadPath);
  assert.equal(result.status, 200); assert.deepEqual(new Uint8Array(await result.arrayBuffer()), f.bytes);
  assert.equal(result.headers.get('content-length'), String(f.bytes.length)); assert.equal(result.headers.get('accept-ranges'), 'bytes');
  assert.equal(result.headers.get('content-type'), 'application/octet-stream'); assert.equal(result.headers.get('cache-control'), 'no-store, private');
  const get = f.bucket.calls.find(call => call[0] === 'get' && call[1].startsWith('artifacts/'));
  assert.deepEqual(get[2].onlyIf, { etagMatches: 'fixture-etag' });
});
test('HEAD reads metadata without installer body and ignores Range as HTTP specifies', async () => {
  const f = await fixture(), result = await f.request(f.downloadPath, { method: 'HEAD', headers: { range: 'bytes=0-3' } });
  assert.equal(result.status, 200); assert.equal(await result.text(), ''); assert.equal(result.headers.get('content-length'), String(f.bytes.length));
  assert.equal(f.bucket.calls.some(call => call[0] === 'get' && call[1].startsWith('artifacts/')), false);
});
for (const [header, offset, length] of [['bytes=0-3', 0, 4], ['bytes=5-', 5, 30], ['bytes=-4', 31, 4], ['bytes=0-999999', 0, 35]]) {
  test(`single range ${header} returns 206 and exact bytes`, async () => {
    const f = await fixture(), result = await f.request(f.downloadPath, { headers: { range: header } });
    assert.equal(result.status, 206); assert.deepEqual(new Uint8Array(await result.arrayBuffer()), f.bytes.slice(offset, offset + length));
    assert.equal(result.headers.get('content-range'), `bytes ${offset}-${offset + length - 1}/${f.bytes.length}`);
  });
}
test('invalid, multiple, zero suffix and unsatisfiable ranges return 416 after authorization', async () => {
  const f = await fixture({ channel: 'early' });
  for (const range of ['bytes=1-2,5-6', 'bytes=-0', 'bytes=1000-', 'bytes=5-1', 'bytes=9007199254740992-', 'not-a-range']) {
    const result = await f.request(f.downloadPath, { headers: { range } }); assert.equal(result.status, 416); assert.equal(result.headers.get('content-range'), `bytes */${f.bytes.length}`);
  }
  f.upstream.body = account('plus'); const denied = await f.request(f.downloadPath, { headers: { range: 'bytes=1000-' } });
  assert.equal(denied.status, 403); assert.equal(denied.headers.get('content-range'), null);
});
test('If-Range mismatch sends full content; ETag 304 still requires authorization', async () => {
  const f = await fixture({ channel: 'early' });
  assert.equal((await f.request(f.downloadPath, { headers: { range: 'bytes=0-2', 'if-range': '"old"' } })).status, 200);
  assert.equal((await f.request(f.downloadPath, { headers: { range: 'bytes=0-2', 'if-range': '"fixture-etag"' } })).status, 206);
  assert.equal((await f.request(f.downloadPath, { headers: { 'if-none-match': '"fixture-etag"' } })).status, 304);
  f.upstream.status = 401; assert.equal((await f.request(f.downloadPath, { headers: { 'if-none-match': '*' } })).status, 401);
});
test('object substitution between head and get, metadata mismatch and retired release cannot serve installer', async () => {
  const f = await fixture(); f.bucket.changed = true; assert.equal((await f.request(f.downloadPath)).status, 409);
  f.bucket.changed = false; f.bucket.objects.get(`artifacts/${f.asset.sha256}/${f.asset.filename}`).customMetadata.sha256 = '0'.repeat(64);
  assert.equal((await f.request(f.downloadPath)).status, 503);
  await f.publish({ ...f.release, version: '1.1.3' }); assert.equal((await f.request(f.downloadPath)).status, 410);
});
test('unsupported platform has no update, routes cannot traverse or override plan via query', async () => {
  const f = await fixture(); assert.equal((await f.request('/v1/updates/stable/linux/aarch64/1.1.1')).status, 204);
  for (const path of ['/v1/updates/stable/windows/x86_64/1.1.1?plan=ultra', '/v1/updates/experimental/windows/x86_64/1.1.1?optIn=true', '/v1/updates/stable/windows%2fx86_64/1.1.1', '/v1/updates/stable/web/x86_64/1.1.1', '/v1/downloads/stable/1.1.2/windows/x86_64/'+ 'a'.repeat(64)+'/bad%00.exe']) assert.ok([400, 404].includes((await f.request(path)).status));
});
test('host, configured origin and methods are constrained; health reveals no bindings/secrets', async () => {
  const f = await fixture(); assert.equal((await f.request('https://evil.example'+f.manifestPath)).status, 421);
  assert.equal((await f.request(undefined, { method: 'POST' })).status, 405);
  const health = await (await f.request('/health')).json(); assert.deepEqual(health, { service: 'openkoda-updates', status: 'ok', updatesEnabled: true });
  f.env.PUBLIC_ORIGIN = 'http://127.0.0.1:1234'; assert.equal((await f.request()).status, 503);
});
test('rate limit is distributed by a hashed Cloudflare IP, with missing binding failing closed', async () => {
  const f = await fixture(); await f.request(); assert.match(f.limited[0].key, /^updates:[a-f0-9]{64}$/);
  f.env.REQUEST_LIMITER.limit = async () => ({ success: false }); const result = await f.request(); assert.equal(result.status, 429); assert.equal(result.headers.get('retry-after'), '60');
  delete f.env.REQUEST_LIMITER; assert.equal((await f.request()).status, 503);
});
