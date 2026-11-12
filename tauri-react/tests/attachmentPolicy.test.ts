import test from 'node:test';
import assert from 'node:assert/strict';
import {inspectAttachment, MAX_ATTACHMENTS, MAX_TOTAL_BYTES} from '../src/components/attachmentPolicy.ts';

test('validates text as UTF-8 and rejects binary masquerading as text', () => {
  assert.deepEqual(inspectAttachment('notes.md', 'application/octet-stream', new TextEncoder().encode('# hello')), {kind: 'text', mime: 'text/plain'});
  assert.ok('error' in inspectAttachment('notes.txt', 'text/plain', new Uint8Array([0xff, 0x00])));
});

test('recognizes image signatures regardless of MIME; rejects corruption and SVG', () => {
  assert.deepEqual(inspectAttachment('photo.png', 'application/octet-stream', new Uint8Array([0x89, 0x50, 0x4e, 0x47])), {kind: 'image', mime: 'image/png'});
  assert.ok('error' in inspectAttachment('photo.jpg', 'image/jpeg', new Uint8Array([0, 1, 2])));
  assert.ok('error' in inspectAttachment('x.svg', 'image/svg+xml', new Uint8Array([60, 115, 118, 103])));
});

test('exports per-message attachment limits', () => {
  assert.equal(MAX_ATTACHMENTS, 10);
  assert.equal(MAX_TOTAL_BYTES, 12 * 1024 * 1024);
});
