import test from 'node:test';
import assert from 'node:assert/strict';
import { formatInlineMarkdown, parseMarkdownBlocks, sanitizeUrl } from '../src/utils/markdownParser.ts';

test('escapes HTML and strips javascript links', () => {
  const rendered = formatInlineMarkdown('<img src=x onerror=alert(1)> [bad](javascript:alert(1))');
  assert.ok(!rendered.includes('<img'));
  assert.ok(!rendered.includes('javascript:'));
  assert.ok(rendered.includes('&lt;img'));
});

test('allows safe https and relative links only', () => {
  assert.equal(sanitizeUrl('https://example.com'), 'https://example.com');
  assert.equal(sanitizeUrl('/docs'), '/docs');
  assert.equal(sanitizeUrl('javascript:alert(1)'), null);
  assert.equal(sanitizeUrl('data:text/html,x'), null);
});

test('renders common markdown as safe HTML blocks', () => {
  const html = parseMarkdownBlocks('# Title\n\n- One\n- Two\n\n> Quote\n\n| A | B |\n|---|---|\n| x | y |')[0].content;
  assert.match(html, /<h1/);
  assert.match(html, /<ul/);
  assert.match(html, /<blockquote/);
  assert.match(html, /<table>/);
});

test('preserves fenced code as a code block, escaped only by React text rendering', () => {
  const blocks = parseMarkdownBlocks('```ts\nconst x = "<tag>";\n```');
  assert.deepEqual(blocks, [{type: 'code', language: 'ts', content: 'const x = "<tag>";'}]);
});
