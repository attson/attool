import { describe, expect, it } from 'vitest';
import {
  base64Decode,
  base64Encode,
  decodeJwt,
  hexDecode,
  hexEncode,
  md5,
  sha1,
  sha256,
  sha512,
  unicodeEscape,
  unicodeUnescape,
  urlDecode,
  urlEncode
} from './codec';

describe('base64', () => {
  it('roundtrips ASCII', () => {
    expect(base64Decode(base64Encode('hello'))).toBe('hello');
  });
  it('roundtrips UTF-8 Chinese', () => {
    const src = '你好，世界 🌍';
    expect(base64Decode(base64Encode(src))).toBe(src);
  });
  it('url-safe strips padding and swaps chars', () => {
    const src = 'subjects?/+';
    const std = base64Encode(src, false);
    const urlSafe = base64Encode(src, true);
    expect(urlSafe).not.toContain('+');
    expect(urlSafe).not.toContain('/');
    expect(urlSafe).not.toContain('=');
    expect(base64Decode(urlSafe, true)).toBe(src);
    // std still roundtrips
    expect(base64Decode(std, false)).toBe(src);
  });
});

describe('url', () => {
  it('encodes and decodes component chars', () => {
    const src = 'a b/c?d=e&f';
    const enc = urlEncode(src);
    expect(enc).toBe('a%20b%2Fc%3Fd%3De%26f');
    expect(urlDecode(enc)).toBe(src);
  });
  it('non-component preserves reserved chars', () => {
    const src = 'https://x.com/a b';
    expect(urlEncode(src, false)).toBe('https://x.com/a%20b');
  });
});

describe('unicode', () => {
  it('escapes non-ascii', () => {
    expect(unicodeEscape('a中b')).toBe('a\\u4e2db');
  });
  it('unescapes', () => {
    expect(unicodeUnescape('a\\u4e2db')).toBe('a中b');
  });
  it('roundtrips emoji via surrogate pair', () => {
    const src = '🌍';
    expect(unicodeUnescape(unicodeEscape(src))).toBe(src);
  });
});

describe('hex', () => {
  it('roundtrips ASCII', () => {
    expect(hexDecode(hexEncode('hi'))).toBe('hi');
    expect(hexEncode('hi')).toBe('6869');
  });
  it('supports separator on encode', () => {
    expect(hexEncode('hi', ' ')).toBe('68 69');
    expect(hexDecode('68 69')).toBe('hi');
  });
  it('tolerates 0x prefix', () => {
    expect(hexDecode('0x6869')).toBe('hi');
  });
  it('rejects odd length', () => {
    expect(() => hexDecode('abc')).toThrow();
  });
  it('rejects non-hex', () => {
    expect(() => hexDecode('gg')).toThrow();
  });
});

describe('hashes', () => {
  it('md5 known vector', () => {
    expect(md5('')).toBe('d41d8cd98f00b204e9800998ecf8427e');
    expect(md5('The quick brown fox jumps over the lazy dog')).toBe('9e107d9d372bb6826bd81d3542a419d6');
  });
  it('sha1 known vector', async () => {
    expect(await sha1('abc')).toBe('a9993e364706816aba3e25717850c26c9cd0d89d');
  });
  it('sha256 known vector', async () => {
    expect(await sha256('abc')).toBe(
      'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad'
    );
  });
  it('sha512 length', async () => {
    expect((await sha512('abc')).length).toBe(128);
  });
});

describe('jwt', () => {
  it('decodes header and payload', () => {
    // { "alg": "HS256", "typ": "JWT" }.{ "sub": "1234567890", "name": "John Doe", "iat": 1516239022 }.<sig>
    const token =
      'eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiaWF0IjoxNTE2MjM5MDIyfQ.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c';
    const decoded = decodeJwt(token);
    expect(decoded.header).toContain('"alg": "HS256"');
    expect(decoded.payload).toContain('"sub": "1234567890"');
    expect(decoded.signatureRaw).toBe('SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c');
  });
  it('rejects malformed', () => {
    expect(() => decodeJwt('not.a.jwt.extra')).toThrow();
    expect(() => decodeJwt('only-one-part')).toThrow();
  });
});
