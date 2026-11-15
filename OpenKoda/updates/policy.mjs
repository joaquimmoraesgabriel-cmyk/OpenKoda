export const MAX_DOWNLOAD = 384 * 1024 * 1024;
export const CHANNELS = new Set(['stable', 'early', 'experimental']);
const platformNames = new Set(['windows-x86_64', 'windows-aarch64', 'linux-x86_64', 'linux-aarch64', 'darwin-x86_64', 'darwin-aarch64']);
export class HttpError extends Error {
  constructor(status, code, headers = {}) { super(code); this.status = status; this.code = code; this.headers = headers; }
}
export function requireValue(condition, status = 503, code = 'updates_unavailable') {
  if (!condition) throw new HttpError(status, code);
}
export function parseVersion(value) {
  if (typeof value !== 'string' || value.length > 80) return null;
  const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/.exec(value);
  if (!match) return null;
  const core = match.slice(1, 4).map(Number), pre = match[4]?.split('.') ?? [];
  if (!core.every(Number.isSafeInteger) || pre.some(p => /^\d+$/.test(p) && (p.length > 1 && p[0] === '0' || !Number.isSafeInteger(Number(p))))) return null;
  return { core, pre };
}
export function compareVersions(left, right) {
  const a = parseVersion(left), b = parseVersion(right);
  requireValue(a && b);
  for (let i = 0; i < 3; i++) if (a.core[i] !== b.core[i]) return a.core[i] > b.core[i] ? 1 : -1;
  if (!a.pre.length || !b.pre.length) return a.pre.length ? -1 : b.pre.length ? 1 : 0;
  for (let i = 0; i < Math.max(a.pre.length, b.pre.length); i++) {
    if (a.pre[i] === undefined) return -1;
    if (b.pre[i] === undefined) return 1;
    if (a.pre[i] === b.pre[i]) continue;
    const an = /^\d+$/.test(a.pre[i]), bn = /^\d+$/.test(b.pre[i]);
    if (an && bn) return Number(a.pre[i]) > Number(b.pre[i]) ? 1 : -1;
    if (an !== bn) return an ? -1 : 1;
    return a.pre[i] > b.pre[i] ? 1 : -1;
  }
  return 0;
}
export const validPlatform = (target, arch) => platformNames.has(`${target}-${arch}`);
export const validDigest = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
export const validFilename = value => typeof value === 'string' && /^[A-Za-z0-9][A-Za-z0-9._-]{0,159}$/.test(value) && !value.includes('..');
export const validUuid = value => typeof value === 'string' && /^[a-f0-9]{8}-[a-f0-9]{4}-[1-5][a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/i.test(value);
export async function sha256(data) {
  const bytes = typeof data === 'string' ? new TextEncoder().encode(data) : data;
  return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', bytes)), n => n.toString(16).padStart(2, '0')).join('');
}
export function validateControl(value) {
  requireValue(value && value.schema === 'openkoda.update-channel.v1' && typeof value.enabled === 'boolean');
  requireValue(Number.isInteger(value.rolloutPercent) && value.rolloutPercent >= 0 && value.rolloutPercent <= 100);
  requireValue(typeof value.rolloutSeed === 'string' && /^[A-Za-z0-9_-]{8,80}$/.test(value.rolloutSeed));
  requireValue(value.release === null || validDigest(value.release));
  requireValue(!value.enabled || value.release !== null);
  return value;
}
export function validateRelease(value, channel, now) {
  requireValue(value && value.schema === 'openkoda.update-release.v1' && value.channel === channel && parseVersion(value.version));
  requireValue(channel !== 'stable' || !parseVersion(value.version).pre.length);
  requireValue(typeof value.pub_date === 'string' && /^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(?:\.\d{1,3})?Z$/.test(value.pub_date));
  requireValue(Number.isFinite(Date.parse(value.pub_date)) && Date.parse(value.pub_date) <= now);
  requireValue(typeof value.notes === 'string' && new TextEncoder().encode(value.notes).length <= 8000);
  requireValue(value.platforms && typeof value.platforms === 'object' && !Array.isArray(value.platforms));
  const entries = Object.entries(value.platforms);
  requireValue(entries.length > 0 && entries.length <= platformNames.size);
  for (const [platform, asset] of entries) {
    requireValue(platformNames.has(platform) && asset && validDigest(asset.sha256) && validFilename(asset.filename));
    requireValue(Number.isSafeInteger(asset.size) && asset.size > 0 && asset.size <= MAX_DOWNLOAD);
    requireValue(typeof asset.signature === 'string' && asset.signature.length >= 32 && asset.signature.length <= 8192 && /^[A-Za-z0-9+/=\r\n]+$/.test(asset.signature));
    requireValue(!Object.hasOwn(asset, 'url') && !Object.hasOwn(asset, 'key'));
  }
  return value;
}
export function eligibleAccount(account, channel, now) {
  const plan = account?.plan?.id, subscription = account?.subscription;
  const start = Date.parse(subscription?.periodStart), end = Date.parse(subscription?.periodEnd);
  return subscription?.status === 'active' && Number.isFinite(start) && Number.isFinite(end) && start <= now && now < end
    && (!subscription.mode || subscription.mode === 'production')
    && (channel === 'early' ? ['pro', 'ultra'].includes(plan) : channel === 'experimental' && plan === 'ultra');
}
export async function eligibleRollout(control, identity) {
  if (control.rolloutPercent === 100) return true;
  if (control.rolloutPercent === 0) return false;
  if (!validUuid(identity)) return false;
  const hash = await sha256(`${control.rolloutSeed}:${identity.toLowerCase()}`);
  return parseInt(hash.slice(0, 8), 16) % 10000 < control.rolloutPercent * 100;
}
export function byteRange(header, size) {
  if (!header) return null;
  const match = /^bytes=(\d*)-(\d*)$/.exec(header);
  const invalid = () => { throw new HttpError(416, 'range_not_satisfiable', { 'content-range': `bytes */${size}` }); };
  if (!match || !match[1] && !match[2]) return invalid();
  const first = match[1] ? Number(match[1]) : null, last = match[2] ? Number(match[2]) : null;
  if ([first, last].some(n => n !== null && (!Number.isSafeInteger(n) || n < 0))) return invalid();
  if (first === null) {
    if (!last) return invalid();
    const length = Math.min(last, size); return { offset: size - length, length };
  }
  if (first >= size || last !== null && last < first) return invalid();
  return { offset: first, length: Math.min(last ?? size - 1, size - 1) - first + 1 };
}
