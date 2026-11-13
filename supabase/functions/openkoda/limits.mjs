export class ApiError extends Error {
  constructor(status, code, detail = {}) {
    super(code);
    this.status = status;
    this.code = code;
    this.detail = detail;
  }
}

export function requireValue(condition, code = 'invalid_input', status = 400) {
  if (!condition) throw new ApiError(status, code);
}

export const byteLength = value => new TextEncoder().encode(value).byteLength;
export const uuid = value => typeof value === 'string' && /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value);

export function textField(value, max) {
  requireValue(typeof value === 'string' && value.trim() && byteLength(value) <= max && !value.includes('\0'));
  return value;
}

export function fields(input, allowed) {
  requireValue(input && typeof input === 'object' && !Array.isArray(input));
  requireValue(Object.keys(input).every(key => allowed.includes(key)));
}

export async function readJson(message, maximum, status = 400) {
  const declared = Number(message.headers.get('content-length') ?? 0);
  requireValue(declared <= maximum, 'body_too_large', status === 400 ? 413 : status);
  const reader = message.body?.getReader();
  requireValue(reader, 'missing_body', status);
  const decoder = new TextDecoder('utf-8',{fatal:true});
  let bytes = 0, text = '';
  try {
    while (true) {
      const {done,value} = await reader.read();
      if (done) break;
      bytes += value.byteLength;
      if (bytes > maximum) {
        await reader.cancel();
        throw new ApiError(status === 400 ? 413 : status, 'body_too_large');
      }
      text += decoder.decode(value,{stream:true});
    }
    return JSON.parse(text + decoder.decode());
  } catch (error) {
    if (error instanceof ApiError) throw error;
    throw new ApiError(status, 'invalid_json');
  } finally {
    reader.releaseLock();
  }
}

export async function fingerprint(value) {
  const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(value));
  return Array.from(new Uint8Array(digest), b => b.toString(16).padStart(2,'0')).join('');
}

export async function ipFingerprint(request, secret) {
  // Hosted Supabase is behind Cloudflare. The edge-set header is a single IP.
  // Do not accept a client-provided X-Forwarded-For chain as a fallback.
  const ip = request.headers.get('cf-connecting-ip');
  requireValue(typeof secret === 'string' && secret.length >= 32 && ip && /^[0-9a-f:.]{2,45}$/i.test(ip),
    'network_identity_unavailable', 503);
  const key = await crypto.subtle.importKey('raw', new TextEncoder().encode(secret), {name:'HMAC',hash:'SHA-256'}, false, ['sign']);
  const signature = await crypto.subtle.sign('HMAC', key, new TextEncoder().encode(ip));
  return Array.from(new Uint8Array(signature), b => b.toString(16).padStart(2,'0')).join('');
}
