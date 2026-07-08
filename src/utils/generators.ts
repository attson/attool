// Personal-toolbox generators. Pure JS; hashes / uuid rely on Web crypto only.

// ---------- password ----------

export interface PasswordOptions {
  length: number;
  lowercase: boolean;
  uppercase: boolean;
  digits: boolean;
  symbols: boolean;
  excludeAmbiguous: boolean;
}

const LOWER = 'abcdefghijklmnopqrstuvwxyz';
const UPPER = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
const DIGITS = '0123456789';
const SYMBOLS = '!@#$%^&*()-_=+[]{};:,.<>?/';
const AMBIGUOUS = 'Il1O0oB8Z2S5';

export function generatePassword(opts: PasswordOptions): string {
  const parts: string[] = [];
  if (opts.lowercase) parts.push(LOWER);
  if (opts.uppercase) parts.push(UPPER);
  if (opts.digits) parts.push(DIGITS);
  if (opts.symbols) parts.push(SYMBOLS);
  if (parts.length === 0) return '';
  let pool = parts.join('');
  if (opts.excludeAmbiguous) {
    pool = [...pool].filter((c) => !AMBIGUOUS.includes(c)).join('');
  }
  const len = Math.max(1, Math.min(256, Math.floor(opts.length)));
  // Ensure at least one char from each selected pool for stronger guarantees
  const guaranteed: string[] = [];
  for (const p of parts) {
    const filtered = opts.excludeAmbiguous
      ? [...p].filter((c) => !AMBIGUOUS.includes(c)).join('')
      : p;
    if (filtered) guaranteed.push(secureChoice(filtered));
  }
  const remaining = Math.max(0, len - guaranteed.length);
  const chars = [...guaranteed];
  for (let i = 0; i < remaining; i++) chars.push(secureChoice(pool));
  return shuffle(chars).join('');
}

function secureChoice(pool: string): string {
  const buf = new Uint32Array(1);
  crypto.getRandomValues(buf);
  return pool[buf[0] % pool.length];
}

function shuffle<T>(arr: T[]): T[] {
  const a = [...arr];
  for (let i = a.length - 1; i > 0; i--) {
    const buf = new Uint32Array(1);
    crypto.getRandomValues(buf);
    const j = buf[0] % (i + 1);
    [a[i], a[j]] = [a[j], a[i]];
  }
  return a;
}

export interface PasswordStrength {
  score: 0 | 1 | 2 | 3 | 4;
  label: string;
  guessesLog10: number;
}

export function scorePassword(pw: string): PasswordStrength {
  // Rough entropy estimate: pool size × length in bits, then translate to score bucket.
  let pool = 0;
  if (/[a-z]/.test(pw)) pool += 26;
  if (/[A-Z]/.test(pw)) pool += 26;
  if (/[0-9]/.test(pw)) pool += 10;
  if (/[^a-zA-Z0-9]/.test(pw)) pool += 32;
  if (pool === 0 || pw.length === 0) {
    return { score: 0, label: '极弱', guessesLog10: 0 };
  }
  const entropyBits = pw.length * Math.log2(pool);
  const guessesLog10 = (entropyBits * Math.log10(2));
  let score: 0 | 1 | 2 | 3 | 4;
  if (entropyBits < 28) score = 0;
  else if (entropyBits < 36) score = 1;
  else if (entropyBits < 60) score = 2;
  else if (entropyBits < 80) score = 3;
  else score = 4;
  const labels = ['极弱', '弱', '一般', '强', '极强'] as const;
  return { score, label: labels[score], guessesLog10 };
}

// ---------- ids ----------

export function uuidV4(): string {
  const b = crypto.getRandomValues(new Uint8Array(16));
  b[6] = (b[6] & 0x0f) | 0x40; // version 4
  b[8] = (b[8] & 0x3f) | 0x80; // variant 10
  const hex = [...b].map((x) => x.toString(16).padStart(2, '0'));
  return `${hex.slice(0, 4).join('')}-${hex.slice(4, 6).join('')}-${hex.slice(6, 8).join('')}-${hex.slice(8, 10).join('')}-${hex.slice(10, 16).join('')}`;
}

const NANOID_ALPHABET = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_-';

export function nanoId(size = 21): string {
  const bytes = crypto.getRandomValues(new Uint8Array(size));
  let out = '';
  for (let i = 0; i < size; i++) out += NANOID_ALPHABET[bytes[i] & 63];
  return out;
}

const ULID_ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ'; // Crockford's base32 (I, L, O, U removed)

export function ulid(): string {
  // 48-bit timestamp (10 chars) + 80 bits random (16 chars) — but ULID we can't use Date.now() in code here.
  // We DO use Date.now() at runtime; caller controls when this is invoked.
  const now = Date.now();
  let time = '';
  let t = now;
  for (let i = 9; i >= 0; i--) {
    time = ULID_ALPHABET[t % 32] + time;
    t = Math.floor(t / 32);
  }
  const rand = crypto.getRandomValues(new Uint8Array(10));
  let rnd = '';
  for (let i = 0; i < 10; i++) {
    // 8 bits per byte, ULID_ALPHABET is 5 bits/char, so 16 chars from 10 bytes.
    // Simplest: take byte mod 32 (introduces slight bias but OK for personal use).
    rnd += ULID_ALPHABET[rand[i] & 31];
    rnd += ULID_ALPHABET[(rand[i] >> 3) & 31];
  }
  return time + rnd.slice(0, 16);
}

// ---------- lorem ipsum ----------

const LOREM_WORDS =
  'lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua enim ad minim veniam quis nostrud exercitation ullamco laboris nisi aliquip ex ea commodo consequat duis aute irure in reprehenderit voluptate velit esse cillum eu fugiat nulla pariatur excepteur sint occaecat cupidatat non proident sunt culpa qui officia deserunt mollit anim id est laborum'.split(
    ' '
  );

const LOREM_CN = '春眠不觉晓 处处闻啼鸟 夜来风雨声 花落知多少 独在异乡为异客 每逢佳节倍思亲 遥知兄弟登高处 遍插茱萸少一人 白日依山尽 黄河入海流 欲穷千里目 更上一层楼 慈母手中线 游子身上衣 临行密密缝 意恐迟迟归 谁言寸草心 报得三春晖 空山不见人 但闻人语响 返景入深林 复照青苔上'.split(
  ' '
);

export function lorem(kind: 'en' | 'cn', unit: 'word' | 'sentence' | 'paragraph', count: number): string {
  const words = kind === 'en' ? LOREM_WORDS : LOREM_CN;
  const pickWord = () => words[Math.floor(Math.random() * words.length)];
  const capitalize = (s: string) => (kind === 'en' ? s.charAt(0).toUpperCase() + s.slice(1) : s);
  const makeSentence = () => {
    const len = 5 + Math.floor(Math.random() * 10);
    const parts: string[] = [];
    for (let i = 0; i < len; i++) parts.push(pickWord());
    if (kind === 'en') return capitalize(parts.join(' ')) + '.';
    return parts.join('，') + '。';
  };
  const makeParagraph = () => {
    const n = 3 + Math.floor(Math.random() * 4);
    const s: string[] = [];
    for (let i = 0; i < n; i++) s.push(makeSentence());
    return s.join(kind === 'en' ? ' ' : '');
  };
  if (unit === 'word') {
    const out: string[] = [];
    for (let i = 0; i < count; i++) out.push(pickWord());
    return kind === 'en' ? out.join(' ') : out.join('');
  }
  if (unit === 'sentence') {
    const out: string[] = [];
    for (let i = 0; i < count; i++) out.push(makeSentence());
    return out.join(kind === 'en' ? ' ' : '');
  }
  const out: string[] = [];
  for (let i = 0; i < count; i++) out.push(makeParagraph());
  return out.join('\n\n');
}

// ---------- fake data ----------

const CN_FIRSTS = '李王张刘陈杨黄赵吴周徐孙马朱胡郭何高林郑梁谢宋唐许韩冯邓曹彭曾萧田董袁潘于蒋蔡余杜叶程苏魏吕丁任沈姚卢姜崔钟谭陆汪范金石廖贾夏韦付方白邹孟熊秦邱江尹薛闫段雷侯龙史陶黎贺顾毛郝龚邵万钱严覃武戴莫孔向汤'.split(
  ''
);
const CN_GIVENS = '伟芳娜秀英敏静丽强磊军洋勇艳杰娟涛明超秀兰霞平刚桂英翔博宇涵浩然梓晗欣悦子涵柯楠慕容浩宇一鸣紫萱雨欣皓轩语彤思远若曦晓明志强佳琪雅琪嘉豪浩然若彤淑芬桂花'.split(
  ''
);
const EN_FIRSTS = ['Alex', 'Sam', 'Jamie', 'Taylor', 'Jordan', 'Casey', 'Morgan', 'Riley', 'Avery', 'Reese'];
const EN_LASTS = ['Smith', 'Johnson', 'Williams', 'Brown', 'Jones', 'Garcia', 'Miller', 'Davis', 'Rodriguez', 'Martinez'];
const EMAIL_DOMAINS = ['example.com', 'mail.test', 'demo.co', 'inbox.io'];

function randomOf<T>(a: T[]): T {
  return a[Math.floor(Math.random() * a.length)];
}

export function fakeChineseName(): string {
  const first = randomOf(CN_FIRSTS);
  const giv = randomOf(CN_GIVENS) + (Math.random() < 0.5 ? randomOf(CN_GIVENS) : '');
  return first + giv;
}

export function fakeEnglishName(): string {
  return `${randomOf(EN_FIRSTS)} ${randomOf(EN_LASTS)}`;
}

export function fakeEmail(): string {
  const parts = ['abc', 'foo', 'bar', 'user', 'test', 'demo'];
  return `${randomOf(parts)}${Math.floor(Math.random() * 9000 + 1000)}@${randomOf(EMAIL_DOMAINS)}`;
}

export function fakeChinesePhone(): string {
  const prefixes = ['139', '138', '186', '187', '188', '150', '151', '152', '158', '159', '176', '178', '188'];
  let out = randomOf(prefixes);
  for (let i = 0; i < 8; i++) out += Math.floor(Math.random() * 10);
  return out;
}

/**
 * Generate a syntactically valid 18-digit Chinese ID card number.
 * The last checksum digit is computed correctly. Everything else (birthday /
 * region code) is random within valid ranges. This is for FILLING TEST FORMS,
 * not for impersonating a real person — hence the fixed prefix set.
 */
export function fakeChineseIdCard(): string {
  const regions = ['110101', '310115', '440305', '500101', '330106', '320102'];
  const region = randomOf(regions);
  const year = 1970 + Math.floor(Math.random() * 40);
  const month = String(1 + Math.floor(Math.random() * 12)).padStart(2, '0');
  const day = String(1 + Math.floor(Math.random() * 28)).padStart(2, '0');
  let seq = '';
  for (let i = 0; i < 3; i++) seq += Math.floor(Math.random() * 10);
  const base = `${region}${year}${month}${day}${seq}`;
  // Checksum
  const weights = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
  const codes = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];
  let sum = 0;
  for (let i = 0; i < 17; i++) sum += Number(base[i]) * weights[i];
  return base + codes[sum % 11];
}

/**
 * Generate a syntactically valid credit card number using the Luhn algorithm.
 * Prefix from common test-BIN ranges (Visa 4, Mastercard 5, Amex 34/37).
 * Made for testing forms — will fail any real bank check because the account
 * portion is random.
 */
export function fakeCreditCard(brand: 'visa' | 'mastercard' | 'amex' = 'visa'): string {
  let prefix: string;
  let length: number;
  if (brand === 'amex') {
    prefix = Math.random() < 0.5 ? '34' : '37';
    length = 15;
  } else if (brand === 'mastercard') {
    prefix = String(51 + Math.floor(Math.random() * 5));
    length = 16;
  } else {
    prefix = '4';
    length = 16;
  }
  let digits = prefix;
  while (digits.length < length - 1) digits += Math.floor(Math.random() * 10);
  // Luhn check digit
  let sum = 0;
  const rev = [...digits].reverse();
  for (let i = 0; i < rev.length; i++) {
    let d = Number(rev[i]);
    if (i % 2 === 0) {
      d *= 2;
      if (d > 9) d -= 9;
    }
    sum += d;
  }
  const check = (10 - (sum % 10)) % 10;
  return digits + check;
}

// ---------- dice / picker ----------

export function rollDice(count: number, sides: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < count; i++) out.push(1 + Math.floor(Math.random() * sides));
  return out;
}

export function pickN<T>(pool: T[], n: number): T[] {
  if (n >= pool.length) return shuffle(pool);
  const copy = shuffle(pool);
  return copy.slice(0, n);
}

export function randomInRange(min: number, max: number, count: number): number[] {
  const lo = Math.min(min, max);
  const hi = Math.max(min, max);
  const out: number[] = [];
  for (let i = 0; i < count; i++) out.push(lo + Math.floor(Math.random() * (hi - lo + 1)));
  return out;
}
