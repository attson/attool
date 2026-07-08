// Encoding / decoding helpers used by the Codec tool.
// All functions are pure; no imports beyond Web platform APIs.

// ---------- Base64 (UTF-8 safe, optionally URL-safe) ----------

export function base64Encode(input: string, urlSafe = false): string {
  const bytes = new TextEncoder().encode(input);
  let binary = '';
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
  const b64 = btoa(binary);
  if (!urlSafe) return b64;
  return b64.replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/g, '');
}

export function base64Decode(input: string, urlSafe = false): string {
  let s = input.trim();
  if (urlSafe) {
    s = s.replace(/-/g, '+').replace(/_/g, '/');
    while (s.length % 4) s += '=';
  }
  const binary = atob(s);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return new TextDecoder('utf-8', { fatal: false }).decode(bytes);
}

// ---------- URL ----------

export function urlEncode(input: string, component = true): string {
  return component ? encodeURIComponent(input) : encodeURI(input);
}

export function urlDecode(input: string, component = true): string {
  return component ? decodeURIComponent(input) : decodeURI(input);
}

// ---------- Unicode escape (\uXXXX or \uXXXX\uXXXX for surrogates) ----------

export function unicodeEscape(input: string): string {
  let out = '';
  for (const ch of input) {
    const code = ch.codePointAt(0)!;
    if (code < 0x80) {
      out += ch;
    } else if (code <= 0xffff) {
      out += '\\u' + code.toString(16).padStart(4, '0');
    } else {
      // Encode as UTF-16 surrogate pair
      const high = 0xd800 + ((code - 0x10000) >> 10);
      const low = 0xdc00 + ((code - 0x10000) & 0x3ff);
      out += '\\u' + high.toString(16).padStart(4, '0');
      out += '\\u' + low.toString(16).padStart(4, '0');
    }
  }
  return out;
}

export function unicodeUnescape(input: string): string {
  return input.replace(/\\u([0-9a-fA-F]{4})/g, (_, hex) => String.fromCharCode(parseInt(hex, 16)));
}

// ---------- Hex ----------

export function hexEncode(input: string, separator = ''): string {
  const bytes = new TextEncoder().encode(input);
  const parts: string[] = [];
  for (let i = 0; i < bytes.length; i++) parts.push(bytes[i].toString(16).padStart(2, '0'));
  return parts.join(separator);
}

export function hexDecode(input: string): string {
  const clean = input.replace(/\s+/g, '').replace(/^0x/i, '');
  if (clean.length % 2 !== 0) throw new Error('Hex 字符串长度必须为偶数');
  if (!/^[0-9a-fA-F]*$/.test(clean)) throw new Error('包含非 hex 字符');
  const bytes = new Uint8Array(clean.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(clean.substr(i * 2, 2), 16);
  }
  return new TextDecoder('utf-8', { fatal: false }).decode(bytes);
}

// ---------- Hashes ----------

async function digest(algo: 'SHA-1' | 'SHA-256' | 'SHA-512', text: string): Promise<string> {
  const bytes = new TextEncoder().encode(text);
  const buf = await crypto.subtle.digest(algo, bytes);
  const arr = new Uint8Array(buf);
  let hex = '';
  for (let i = 0; i < arr.length; i++) hex += arr[i].toString(16).padStart(2, '0');
  return hex;
}

export const sha1 = (text: string) => digest('SHA-1', text);
export const sha256 = (text: string) => digest('SHA-256', text);
export const sha512 = (text: string) => digest('SHA-512', text);

// Pure-JS MD5 (RFC 1321). ~1KB source; kept inline to avoid a runtime dep.
export function md5(input: string): string {
  const bytes = new TextEncoder().encode(input);
  const msg = Array.from(bytes);
  const originalLen = msg.length;

  msg.push(0x80);
  while (msg.length % 64 !== 56) msg.push(0);

  // 64-bit little-endian length in bits. Split hi/lo because JS `>>>` is mod-32.
  const bitLen = originalLen * 8;
  const lo = bitLen >>> 0;
  const hi = Math.floor(bitLen / 0x100000000) >>> 0;
  for (let i = 0; i < 4; i++) msg.push((lo >>> (8 * i)) & 0xff);
  for (let i = 0; i < 4; i++) msg.push((hi >>> (8 * i)) & 0xff);

  const K = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391
  ];
  const s = [
    7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
    5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
    4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
    6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21
  ];

  let a0 = 0x67452301, b0 = 0xefcdab89, c0 = 0x98badcfe, d0 = 0x10325476;

  const rotl = (x: number, n: number) => ((x << n) | (x >>> (32 - n))) >>> 0;

  for (let chunk = 0; chunk < msg.length; chunk += 64) {
    const M = new Array<number>(16);
    for (let i = 0; i < 16; i++) {
      M[i] =
        msg[chunk + i * 4] |
        (msg[chunk + i * 4 + 1] << 8) |
        (msg[chunk + i * 4 + 2] << 16) |
        (msg[chunk + i * 4 + 3] << 24);
      M[i] = M[i] >>> 0;
    }

    let A = a0, B = b0, C = c0, D = d0;
    for (let i = 0; i < 64; i++) {
      let F: number, g: number;
      if (i < 16) { F = (B & C) | (~B & D); g = i; }
      else if (i < 32) { F = (D & B) | (~D & C); g = (5 * i + 1) % 16; }
      else if (i < 48) { F = B ^ C ^ D; g = (3 * i + 5) % 16; }
      else { F = C ^ (B | ~D); g = (7 * i) % 16; }
      F = (F + A + K[i] + M[g]) >>> 0;
      A = D;
      D = C;
      C = B;
      B = (B + rotl(F, s[i])) >>> 0;
    }
    a0 = (a0 + A) >>> 0;
    b0 = (b0 + B) >>> 0;
    c0 = (c0 + C) >>> 0;
    d0 = (d0 + D) >>> 0;
  }

  const toLE = (n: number) => {
    let hex = '';
    for (let i = 0; i < 4; i++) hex += ((n >>> (i * 8)) & 0xff).toString(16).padStart(2, '0');
    return hex;
  };
  return toLE(a0) + toLE(b0) + toLE(c0) + toLE(d0);
}

// ---------- JWT decode (no signature verification) ----------

export interface JwtParts {
  header: string;
  payload: string;
  signature: string;
  headerRaw: string;
  payloadRaw: string;
  signatureRaw: string;
}

export function decodeJwt(input: string): JwtParts {
  const s = input.trim();
  const parts = s.split('.');
  if (parts.length !== 3) throw new Error('JWT 应有 3 段（用点分隔）');
  const [h, p, sig] = parts;

  const decodePart = (raw: string) => {
    if (!raw) return '';
    try {
      const text = base64Decode(raw, true);
      // Try to pretty-print if it's JSON
      try {
        return JSON.stringify(JSON.parse(text), null, 2);
      } catch {
        return text;
      }
    } catch (err) {
      return `<解码失败：${err}>`;
    }
  };

  return {
    header: decodePart(h),
    payload: decodePart(p),
    signature: sig,
    headerRaw: h,
    payloadRaw: p,
    signatureRaw: sig
  };
}
