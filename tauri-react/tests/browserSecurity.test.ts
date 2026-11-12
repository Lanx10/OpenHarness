import test from 'node:test';
import assert from 'node:assert/strict';
import {normalizeBrowserAddress} from '../src/utils/browserAddress.ts';

test('normalizes explicit HTTP and HTTPS URLs and rejects unsupported protocols', () => {
  assert.equal(normalizeBrowserAddress(' HTTPS://example.com/path '), 'https://example.com/path');
  assert.equal(normalizeBrowserAddress('http://localhost:3000/'), 'http://localhost:3000/');
  for (const value of ['file:///etc/passwd', 'javascript:alert(1)', 'data:text/html,hi', 'ftp://example.com', 'about:blank']) {
    assert.throws(() => normalizeBrowserAddress(value), /Only HTTP and HTTPS/);
  }
});

test('rejects malformed URLs and embedded credentials', () => {
  for (const value of ['http://', 'https://exa mple.com', 'https://user:pass@example.com']) {
    assert.throws(() => normalizeBrowserAddress(value));
  }
});

test('preserves explicit localhost/private HTTP workflows and domain defaults', () => {
  assert.equal(normalizeBrowserAddress('localhost:5173'), 'http://localhost:5173');
  assert.equal(normalizeBrowserAddress('192.168.1.25:8080'), 'http://192.168.1.25:8080');
  assert.equal(normalizeBrowserAddress('example.com/docs'), 'https://example.com/docs');
});

test('keeps search terms as search and uses default for empty input', () => {
  assert.equal(normalizeBrowserAddress('react hooks'), 'https://duckduckgo.com/?q=react%20hooks');
  assert.equal(normalizeBrowserAddress(''), 'http://localhost:5173');
});
