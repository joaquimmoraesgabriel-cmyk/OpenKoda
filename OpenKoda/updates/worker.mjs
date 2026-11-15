import { HttpError, requireValue, CHANNELS, parseVersion, compareVersions, validPlatform, validDigest,
  validFilename, validateControl, validateRelease, eligibleAccount, eligibleRollout, byteRange, sha256 } from './policy.mjs';

const HEADERS = { 'cache-control': 'no-store, private', 'x-content-type-options': 'nosniff',
  'referrer-policy': 'no-referrer', 'content-security-policy': "default-src 'none'; sandbox",
  'strict-transport-security': 'max-age=31536000', vary: 'Authorization, X-Koda-Experimental, X-Koda-Installation-Id' };
function response(body, status = 200, extra = {}, head = false) {
  return new Response(head || status === 204 || status === 304 ? null : body,
    { status, headers: { ...HEADERS, ...extra } });
}
function json(body, status = 200, extra = {}, head = false) {
  return response(JSON.stringify(body), status, { 'content-type': 'application/json; charset=utf-8', ...extra }, head);
}
async function boundedBytes(body, limit) {
  requireValue(body && typeof body.getReader === 'function');
  const reader = body.getReader(), chunks = []; let total = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read(); if (done) break;
      total += value.byteLength; requireValue(total <= limit);
      chunks.push(value);
    }
  } catch (error) { await reader.cancel().catch(() => {}); throw error; }
  finally { reader.releaseLock(); }
  const bytes = new Uint8Array(total); let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  return bytes;
}
function parseJson(bytes) {
  try { return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes)); }
  catch { throw new HttpError(503, 'updates_unavailable'); }
}
async function readObject(bucket, key, limit, digest) {
  const object = await bucket.get(key);
  if (!object) return null;
  if (!(Number.isSafeInteger(object.size) && object.size >= 0 && object.size <= limit && object.body)) {
    await object.body?.cancel().catch(() => {}); throw new HttpError(503, 'updates_unavailable');
  }
  const bytes = await boundedBytes(object.body, limit);
  requireValue(bytes.byteLength === object.size && (!digest || await sha256(bytes) === digest));
  return parseJson(bytes);
}
function configuredOrigin(value, supabase = false) {
  let url; try { url = new URL(value); } catch { throw new HttpError(503, 'updates_unavailable'); }
  requireValue(url.protocol === 'https:' && !url.username && !url.password && !url.port && url.pathname === '/' && !url.search && !url.hash);
  if (supabase) requireValue(/^[a-z0-9]{20}\.supabase\.co$/.test(url.hostname));
  else requireValue(/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/.test(url.hostname) && url.hostname.includes('.') && !/^[\d.]+$/.test(url.hostname));
  return url.origin;
}
async function authorize(request, env, channel, fetcher, clock) {
  if (channel === 'stable') return;
  const auth = request.headers.get('authorization') ?? '';
  requireValue(auth.length < 16384 && /^Bearer [A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/.test(auth), 401, 'authentication_required');
  if (channel === 'experimental') requireValue(request.headers.get('x-koda-experimental') === 'true', 403, 'channel_not_available');
  const origin = configuredOrigin(env.SUPABASE_URL, true);
  requireValue(typeof env.SUPABASE_PUBLISHABLE_KEY === 'string' && env.SUPABASE_PUBLISHABLE_KEY.length > 20);
  let remote;
  try {
    // This existing Edge endpoint verifies the token with Auth.getUser and
    // computes financial entitlements in SQL; user metadata is never authority.
    remote = await fetcher(`${origin}/functions/v1/openkoda`, {
      method: 'POST', redirect: 'error', signal: AbortSignal.timeout(8000),
      headers: { authorization: auth, apikey: env.SUPABASE_PUBLISHABLE_KEY, 'content-type': 'application/json' },
      body: JSON.stringify({ action: 'account' }),
    });
  } catch { throw new HttpError(503, 'account_unavailable'); }
  if (remote.status === 401 || remote.status === 403) { await remote.body?.cancel(); throw new HttpError(remote.status, remote.status === 401 ? 'authentication_required' : 'channel_not_available'); }
  if (!remote.ok) { await remote.body?.cancel(); throw new HttpError(503, 'account_unavailable'); }
  const declared = remote.headers.get('content-length');
  if (declared && (!/^\d+$/.test(declared) || Number(declared) > 65536)) {
    await remote.body?.cancel(); throw new HttpError(503, 'updates_unavailable');
  }
  const account = parseJson(await boundedBytes(remote.body, 65536));
  requireValue(eligibleAccount(account, channel, clock()), 403, 'channel_not_available');
}
async function currentRelease(env, channel, now) {
  requireValue(env.UPDATES_BUCKET && typeof env.UPDATES_BUCKET.get === 'function');
  const raw = await readObject(env.UPDATES_BUCKET, `channels/${channel}.json`, 8192);
  if (!raw) return null;
  const control = validateControl(raw);
  if (!control.enabled || !control.rolloutPercent || !control.release) return { control, release: null };
  const rawRelease = await readObject(env.UPDATES_BUCKET, `releases/${control.release}.json`, 65536, control.release);
  requireValue(rawRelease);
  return { control, release: validateRelease(rawRelease, channel, now) };
}
function parseRoute(url) {
  // Reject alternate encodings, path tricks, arbitrary R2 keys and every query.
  requireValue(url.pathname.length <= 700 && !url.search && !/%|\\|\/\//.test(url.pathname), 400, 'invalid_request');
  const p = url.pathname.split('/');
  const isManifest = p[2] === 'updates';
  requireValue(p[1] === 'v1' && (isManifest && p.length === 7 || p[2] === 'downloads' && p.length === 9), 404, 'not_found');
  const channel = p[3], target = isManifest ? p[4] : p[5], arch = isManifest ? p[5] : p[6], version = isManifest ? p[6] : p[4];
  requireValue(CHANNELS.has(channel) && validPlatform(target, arch) && parseVersion(version), 400, 'invalid_request');
  if (!isManifest) requireValue(validDigest(p[7]) && validFilename(p[8]), 400, 'invalid_request');
  return { isManifest, channel, target, arch, version, digest: p[7], filename: p[8] };
}
async function rateLimit(request, env) {
  // Production deploy requires a distributed rate-limit binding, never an
  // isolate-local counter. Missing/broken configuration fails closed.
  requireValue(env.REQUEST_LIMITER && typeof env.REQUEST_LIMITER.limit === 'function');
  const ip = request.headers.get('cf-connecting-ip');
  requireValue(ip && ip.length <= 64 && /^[a-fA-F0-9:.]+$/.test(ip), 400, 'invalid_request');
  const result = await env.REQUEST_LIMITER.limit({ key: `updates:${await sha256(ip)}` });
  requireValue(result?.success === true, 429, 'rate_limited');
}
export function createHandler({ fetcher = fetch, now = () => Date.now() } = {}) {
  return async function handle(request, env) {
    const head = request.method === 'HEAD';
    try {
      const url = new URL(request.url), origin = configuredOrigin(env.PUBLIC_ORIGIN);
      requireValue(url.origin === origin, 421, 'host_not_allowed');
      requireValue(request.method === 'GET' || head, 405, 'method_not_allowed');
      if (url.pathname === '/health' && !url.search) return json({ service: 'openkoda-updates', status: 'ok', updatesEnabled: env.UPDATES_ENABLED === 'true' }, 200, {}, head);
      const route = parseRoute(url), { isManifest, channel, target, arch, version } = route;
      if (env.UPDATES_ENABLED !== 'true') return isManifest ? response(null, 204, { 'x-koda-updates-paused': 'true' }) : json({ error: 'updates_paused' }, 503, { 'retry-after': '3600' }, head);
      await rateLimit(request, env);
      await authorize(request, env, channel, fetcher, now);
      const current = await currentRelease(env, channel, now());
      if (!current?.release) return isManifest ? response(null, 204, { 'x-koda-updates-paused': 'true' }) : json({ error: 'release_not_available' }, 410, {}, head);
      const { control, release } = current;
      if (!await eligibleRollout(control, request.headers.get('x-koda-installation-id'))) return isManifest ? response(null, 204) : json({ error: 'release_not_available' }, 403, {}, head);
      const asset = release.platforms[`${target}-${arch}`];
      if (!asset) return isManifest ? response(null, 204) : json({ error: 'not_found' }, 404, {}, head);
      if (isManifest) {
        if (compareVersions(release.version, version) <= 0) return response(null, 204);
        const download = `${origin}/v1/downloads/${channel}/${release.version}/${target}/${arch}/${asset.sha256}/${asset.filename}`;
        return json({ version: release.version, notes: release.notes, pub_date: release.pub_date, url: download,
          signature: asset.signature, rolloutPercent: 100 }, 200, {}, head);
      }
      requireValue(version === release.version && route.digest === asset.sha256 && route.filename === asset.filename, 410, 'release_not_available');
      const key = `artifacts/${asset.sha256}/${asset.filename}`, meta = await env.UPDATES_BUCKET.head(key);
      requireValue(meta && meta.size === asset.size && meta.customMetadata?.sha256 === asset.sha256 && typeof meta.etag === 'string' && typeof meta.httpEtag === 'string');
      const headers = { 'content-type': 'application/octet-stream', 'content-disposition': `attachment; filename="${asset.filename}"`,
        'accept-ranges': 'bytes', etag: meta.httpEtag, 'content-length': String(meta.size) };
      if (request.headers.get('if-none-match') === meta.httpEtag || request.headers.get('if-none-match') === '*') return response(null, 304, { etag: meta.httpEtag });
      let range = null;
      if (!head && (!request.headers.has('if-range') || request.headers.get('if-range') === meta.httpEtag)) range = byteRange(request.headers.get('range'), meta.size);
      if (range) { headers['content-range'] = `bytes ${range.offset}-${range.offset + range.length - 1}/${meta.size}`; headers['content-length'] = String(range.length); }
      if (head) return response(null, 200, headers, true);
      // Pin the read to the object checked above; stream without buffering the
      // installer. R2 immutable locks + Tauri's signature protect its contents.
      const object = await env.UPDATES_BUCKET.get(key, { onlyIf: { etagMatches: meta.etag }, ...(range ? { range } : {}) });
      requireValue(object?.body && object.size === meta.size && object.etag === meta.etag, 409, 'release_changed');
      return response(object.body, range ? 206 : 200, headers);
    } catch (error) {
      const known = error instanceof HttpError, status = known ? error.status : 503;
      // Never log URLs, query strings, credentials, user IDs or upstream bodies.
      if (status >= 500) console.error(JSON.stringify({ event: 'update_service_error', code: known ? error.code : 'unexpected_failure' }));
      return json({ error: known ? error.code : 'updates_unavailable' }, status,
        { ...(status === 405 ? { allow: 'GET, HEAD' } : {}), ...(status === 429 || status === 503 ? { 'retry-after': status === 429 ? '60' : '300' } : {}), ...(known ? error.headers : {}) }, head);
    }
  };
}
export default { fetch: createHandler() };
