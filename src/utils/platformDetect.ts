export type VideoPlatform = 'douyin' | 'xhs' | 'bilibili' | 'youtube';

interface PlatformSpec {
  id: VideoPlatform;
  name: string;
  urlPattern: RegExp;
  extractPattern: RegExp;
}

const PLATFORMS: PlatformSpec[] = [
  {
    id: 'douyin',
    name: '抖音',
    urlPattern: /^https?:\/\/(v\.douyin\.com|www\.douyin\.com|www\.iesdouyin\.com)\//i,
    extractPattern: /https?:\/\/v\.douyin\.com\/[A-Za-z0-9_-]+\/?/gi
  },
  {
    id: 'xhs',
    name: '小红书',
    urlPattern: /^https?:\/\/(www\.xiaohongshu\.com|xhslink\.com)\//i,
    extractPattern: /https?:\/\/(?:www\.xiaohongshu\.com|xhslink\.com)\/[A-Za-z0-9/_?=&%.-]+/gi
  },
  {
    id: 'bilibili',
    name: 'B 站',
    urlPattern: /^https?:\/\/(www\.bilibili\.com|b23\.tv|m\.bilibili\.com)\//i,
    extractPattern: /https?:\/\/(?:www\.bilibili\.com|b23\.tv|m\.bilibili\.com)\/[A-Za-z0-9/_?=&%.-]+/gi
  },
  {
    id: 'youtube',
    name: 'YouTube',
    urlPattern: /^https?:\/\/(www\.youtube\.com|youtube\.com|youtu\.be|m\.youtube\.com)\//i,
    extractPattern: /https?:\/\/(?:www\.youtube\.com|youtube\.com|youtu\.be|m\.youtube\.com)\/[A-Za-z0-9/_?=&%.-]+/gi
  }
];

export function detectPlatform(url: string): VideoPlatform | null {
  const trimmed = url.trim();
  for (const p of PLATFORMS) {
    if (p.urlPattern.test(trimmed)) return p.id;
  }
  return null;
}

export function platformName(id: VideoPlatform): string {
  return PLATFORMS.find((p) => p.id === id)?.name ?? id;
}

export interface ExtractedLink {
  url: string;
  platform: VideoPlatform;
}

/**
 * Extract all recognized video links from arbitrary text (share copy).
 * Preserves discovery order and dedupes by (platform, normalized URL).
 */
export function extractLinks(text: string): ExtractedLink[] {
  if (!text) return [];
  const seen = new Set<string>();
  const results: ExtractedLink[] = [];
  for (const p of PLATFORMS) {
    const matches = text.match(p.extractPattern);
    if (!matches) continue;
    for (const raw of matches) {
      const normalized = normalize(raw, p.id);
      const key = `${p.id}::${normalized}`;
      if (seen.has(key)) continue;
      seen.add(key);
      results.push({ url: normalized, platform: p.id });
    }
  }
  return results;
}

function normalize(url: string, platform: VideoPlatform): string {
  const withHttps = url.replace(/^http:\/\//i, 'https://');
  if (platform === 'douyin') {
    return withHttps
      .replace(/^https:\/\/v\.douyin\.com/i, 'https://v.douyin.com')
      .replace(/\/*$/, '/');
  }
  return withHttps.replace(/\/+$/, '');
}
