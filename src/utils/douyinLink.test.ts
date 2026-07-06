import { describe, expect, it } from 'vitest';
import { extractDouyinLinks } from './douyinLink';

describe('extractDouyinLinks', () => {
  it('returns empty array for empty string', () => {
    expect(extractDouyinLinks('')).toEqual([]);
  });

  it('returns empty array for whitespace-only input', () => {
    expect(extractDouyinLinks('   \n\t  ')).toEqual([]);
  });

  it('returns empty array when no douyin link is present', () => {
    expect(extractDouyinLinks('hello world, no link here')).toEqual([]);
  });

  it('extracts a single link from a real-world share caption', () => {
    const text = '9.99 复制打开抖音，看看【标题】xxxx https://v.douyin.com/iRnKtwr8/ 复制此链接';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/iRnKtwr8/']);
  });

  it('extracts multiple distinct links in the order they appear', () => {
    const text = 'first https://v.douyin.com/aaa111/ second https://v.douyin.com/bbb222/';
    expect(extractDouyinLinks(text)).toEqual([
      'https://v.douyin.com/aaa111/',
      'https://v.douyin.com/bbb222/'
    ]);
  });

  it('deduplicates the same link when it appears twice, once with and once without trailing slash', () => {
    const text = 'a https://v.douyin.com/abc123/ b https://v.douyin.com/abc123 c';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/abc123/']);
  });

  it('lowercases the host but preserves case in the path (ids are case-sensitive)', () => {
    const text = 'HTTPS://V.DOUYIN.COM/AbC123';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/AbC123/']);
  });

  it('upgrades http to https', () => {
    const text = 'http://v.douyin.com/xyz789';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/xyz789/']);
  });

  it('extracts a link that is immediately adjacent to CJK characters', () => {
    const text = '看看【标题】https://v.douyin.com/abc/复制此链接';
    expect(extractDouyinLinks(text)).toEqual(['https://v.douyin.com/abc/']);
  });
});
