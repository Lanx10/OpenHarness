import test from 'node:test';
import assert from 'node:assert/strict';
import {classifyProviderEndpoint} from '../src/components/Settings/providerEndpoint.ts';

test('classifies loopback and remote HTTP(S) endpoints accurately', () => {
  for (const url of ['http://localhost', 'http://127.0.0.1', 'http://[::1]']) assert.equal(classifyProviderEndpoint(url).kind, 'local_http');
  assert.equal(classifyProviderEndpoint('https://localhost').kind, 'local_https');
  assert.equal(classifyProviderEndpoint('https://api.example.com').kind, 'remote_https');
  assert.equal(classifyProviderEndpoint('http://api.example.com').kind, 'remote_http');
  assert.equal(classifyProviderEndpoint('http://192.0.2.1').kind, 'remote_http');
});

test('rejects malformed, unsupported and credential-bearing endpoints', () => {
  for (const url of ['', 'file:///tmp/foo', 'not a URL', 'https://example.com:bad', 'https://user:pass@example.com']) {
    assert.equal(classifyProviderEndpoint(url).kind, 'invalid', url);
  }
});
