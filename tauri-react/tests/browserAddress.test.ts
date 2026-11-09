import test from 'node:test';
import assert from 'node:assert/strict';
import { normalizeBrowserAddress } from '../src/utils/browserAddress.ts';

test('normalizeBrowserAddress returns default for empty input', () => {
  assert.equal(normalizeBrowserAddress(''), 'http://localhost:5173');
});

test('normalizeBrowserAddress prefixes localhost', () => {
  assert.equal(normalizeBrowserAddress('localhost:3000'), 'http://localhost:3000');
  assert.equal(normalizeBrowserAddress('localhost:3000/path'), 'http://localhost:3000/path');
  assert.equal(normalizeBrowserAddress('127.0.0.1:8080'), 'http://127.0.0.1:8080');
});

test('normalizeBrowserAddress supports domain and URL forms', () => {
  assert.equal(normalizeBrowserAddress('https://openrouter.ai'), 'https://openrouter.ai');
  assert.equal(normalizeBrowserAddress('example.com/docs'), 'https://example.com/docs');
});

test('normalizeBrowserAddress converts plain terms to search URL', () => {
  assert.equal(normalizeBrowserAddress('react hooks'), 'https://duckduckgo.com/?q=react%20hooks');
});
