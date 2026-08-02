import { describe, expect, it } from 'vitest';
import { renderMarkdown } from './markdown';

describe('renderMarkdown', () => {
  it('renders heading', () => {
    expect(renderMarkdown('# hi')).toBe('<h1>hi</h1>');
    expect(renderMarkdown('### h3')).toBe('<h3>h3</h3>');
  });
  it('renders paragraph and escapes html', () => {
    expect(renderMarkdown('a<b>c')).toBe('<p>a&lt;b&gt;c</p>');
  });
  it('renders fenced code with language and does not escape markdown inside', () => {
    const html = renderMarkdown('```ts\nconst a = 1\n```');
    expect(html).toContain('<pre data-lang="ts"><code>const a = 1');
  });
  it('escapes html inside code fence', () => {
    const html = renderMarkdown('```\n<div>\n```');
    expect(html).toContain('&lt;div&gt;');
  });
  it('renders inline code', () => {
    expect(renderMarkdown('use `x` here')).toBe('<p>use <code>x</code> here</p>');
  });
  it('renders unordered list', () => {
    const html = renderMarkdown('- a\n- b');
    expect(html).toBe('<ul><li>a</li><li>b</li></ul>');
  });
  it('renders ordered list', () => {
    const html = renderMarkdown('1. a\n2. b');
    expect(html).toBe('<ol><li>a</li><li>b</li></ol>');
  });
  it('renders blockquote', () => {
    expect(renderMarkdown('> hi')).toBe('<blockquote>hi</blockquote>');
  });
  it('renders link with target blank', () => {
    const html = renderMarkdown('[claude](https://claude.ai)');
    expect(html).toBe('<p><a href="https://claude.ai" target="_blank" rel="noopener">claude</a></p>');
  });
  it('renders bold and italic', () => {
    expect(renderMarkdown('**b** and *i*')).toBe('<p><strong>b</strong> and <em>i</em></p>');
  });
  it('renders hr', () => {
    expect(renderMarkdown('---')).toBe('<hr>');
  });
  it('escapes attribute-like content in text', () => {
    expect(renderMarkdown('a"b')).toBe('<p>a&quot;b</p>');
  });
  it('escapes url quotes in link', () => {
    const html = renderMarkdown('[x](https://a"b)');
    expect(html).toContain('href="https://a&quot;b"');
  });
});
