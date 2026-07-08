import { describe, expect, it } from 'vitest';
import {
  fakeChineseIdCard,
  fakeCreditCard,
  generatePassword,
  lorem,
  nanoId,
  pickN,
  rollDice,
  scorePassword,
  ulid,
  uuidV4
} from './generators';

describe('generatePassword', () => {
  it('returns requested length', () => {
    const pw = generatePassword({
      length: 16,
      lowercase: true,
      uppercase: true,
      digits: true,
      symbols: true,
      excludeAmbiguous: false
    });
    expect(pw.length).toBe(16);
  });
  it('excludes ambiguous chars when asked', () => {
    for (let i = 0; i < 20; i++) {
      const pw = generatePassword({
        length: 40,
        lowercase: true,
        uppercase: true,
        digits: true,
        symbols: false,
        excludeAmbiguous: true
      });
      expect(pw).not.toMatch(/[Il1O0oB8Z2S5]/);
    }
  });
  it('returns empty string when no charsets picked', () => {
    expect(generatePassword({
      length: 16,
      lowercase: false,
      uppercase: false,
      digits: false,
      symbols: false,
      excludeAmbiguous: false
    })).toBe('');
  });
});

describe('scorePassword', () => {
  it('scores weak short', () => {
    expect(scorePassword('abc').score).toBeLessThanOrEqual(1);
  });
  it('scores long random high', () => {
    expect(scorePassword('h9J#4qKz@2Xf!7Wn').score).toBeGreaterThanOrEqual(3);
  });
});

describe('uuidV4', () => {
  it('matches shape', () => {
    const id = uuidV4();
    expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/);
  });
});

describe('nanoId', () => {
  it('respects size', () => {
    expect(nanoId(10).length).toBe(10);
    expect(nanoId(21).length).toBe(21);
  });
});

describe('ulid', () => {
  it('length 26', () => {
    expect(ulid().length).toBe(26);
  });
});

describe('lorem', () => {
  it('word count', () => {
    const w = lorem('en', 'word', 5);
    expect(w.split(' ').length).toBe(5);
  });
  it('cn sentence ends with period', () => {
    const s = lorem('cn', 'sentence', 1);
    expect(s.endsWith('。')).toBe(true);
  });
});

describe('fakeChineseIdCard', () => {
  it('is 18 chars and checksum valid', () => {
    for (let i = 0; i < 10; i++) {
      const id = fakeChineseIdCard();
      expect(id.length).toBe(18);
      const weights = [7, 9, 10, 5, 8, 4, 2, 1, 6, 3, 7, 9, 10, 5, 8, 4, 2];
      const codes = ['1', '0', 'X', '9', '8', '7', '6', '5', '4', '3', '2'];
      let sum = 0;
      for (let i = 0; i < 17; i++) sum += Number(id[i]) * weights[i];
      expect(id[17]).toBe(codes[sum % 11]);
    }
  });
});

describe('fakeCreditCard', () => {
  const luhnValid = (num: string) => {
    let sum = 0;
    const rev = [...num].reverse();
    for (let i = 0; i < rev.length; i++) {
      let d = Number(rev[i]);
      if (i % 2 === 1) {
        d *= 2;
        if (d > 9) d -= 9;
      }
      sum += d;
    }
    return sum % 10 === 0;
  };
  it('visa passes Luhn', () => {
    for (let i = 0; i < 10; i++) expect(luhnValid(fakeCreditCard('visa'))).toBe(true);
  });
  it('mastercard passes Luhn', () => {
    for (let i = 0; i < 10; i++) expect(luhnValid(fakeCreditCard('mastercard'))).toBe(true);
  });
  it('amex is 15 digits + Luhn', () => {
    for (let i = 0; i < 10; i++) {
      const n = fakeCreditCard('amex');
      expect(n.length).toBe(15);
      expect(luhnValid(n)).toBe(true);
    }
  });
});

describe('dice / pick', () => {
  it('rollDice range', () => {
    const r = rollDice(20, 6);
    expect(r.length).toBe(20);
    for (const v of r) expect(v).toBeGreaterThanOrEqual(1);
    for (const v of r) expect(v).toBeLessThanOrEqual(6);
  });
  it('pickN unique', () => {
    const picked = pickN([1, 2, 3, 4, 5], 3);
    expect(picked.length).toBe(3);
    expect(new Set(picked).size).toBe(3);
  });
});
