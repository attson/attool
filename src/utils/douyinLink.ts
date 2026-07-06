const DOUYIN_SHORT_URL_RE = /https?:\/\/v\.douyin\.com\/[A-Za-z0-9_-]+\/?/gi;

export function extractDouyinLinks(text: string): string[] {
  if (!text) return [];
  const matches = text.match(DOUYIN_SHORT_URL_RE);
  if (!matches) return [];

  const seen = new Set<string>();
  const result: string[] = [];
  for (const raw of matches) {
    const normalized = normalize(raw);
    if (!seen.has(normalized)) {
      seen.add(normalized);
      result.push(normalized);
    }
  }
  return result;
}

function normalize(url: string): string {
  const withHttps = url.replace(/^http:\/\//i, 'https://');
  const lowercaseHost = withHttps.replace(/^https:\/\/v\.douyin\.com/i, 'https://v.douyin.com');
  return lowercaseHost.replace(/\/*$/, '/');
}
