import test from 'node:test';
import assert from 'node:assert/strict';
import {addUsageOnce, emptyTokenUsage, isCurrentThreadLoad, tokenUsageTotals} from '../src/components/tokenUsage.ts';
import {estimateMessageTokens} from '../src/components/stepLifecycle.ts';

 test('estimates multilingual text and image attachments conservatively with overhead', () => {
  assert.ok(estimateMessageTokens('你好こんにちは مرحبا') > Math.ceil('你好こんにちは مرحبا'.length / 4));
  assert.ok(estimateMessageTokens('tiny', ['![image](data:image/png;base64,AAAA)']) > estimateMessageTokens('tiny'));
});

 test('estimator includes fixed message overhead even for tiny text', () => {
  assert.ok(estimateMessageTokens('x') > estimateMessageTokens(''));
  assert.ok(estimateMessageTokens('') > 0);
});

 test('preserves a recent message suffix during compaction', () => {
  assert.deepEqual(compactionWindow([0, 1, 2, 3, 4, 5], 2), {summarize: [0, 1, 2, 3], retain: [4, 5]});
});

import {compactionWindow} from '../src/components/tokenUsage.ts';



test('restores reported usage from persisted thread records after reload', () => {
  const restored = tokenUsageTotals([
    {source: 'provider_reported', usage: {input_tokens: 12, output_tokens: 5, total_tokens: 17, cached_input_tokens: 2, reasoning_tokens: 1}},
  ]);
  assert.deepEqual(restored, {input: 12, output: 5, total: 17, cached: 2, reasoning: 1});
});

test('isolates totals across threads, new chat, and legacy threads', () => {
  const a = tokenUsageTotals([{source: 'provider_reported', usage: {input_tokens: 10}}]);
  const b = tokenUsageTotals([{source: 'provider_reported', usage: {input_tokens: 3}}]);
  assert.equal(a.input, 10);
  assert.equal(b.input, 3);
  assert.deepEqual(emptyTokenUsage(), {input: null, output: null, total: null, cached: null, reasoning: null});
  assert.deepEqual(tokenUsageTotals(), emptyTokenUsage());
});

test('preserves missing provider fields as unknown and accepts actual zero', () => {
  assert.deepEqual(tokenUsageTotals([{source: 'provider_reported', usage: {input_tokens: 0, reasoning_tokens: 4}}]), {
    input: 0, output: null, total: null, cached: null, reasoning: 4,
  });
});

test('excludes estimates and deduplicates streaming usage updates by request ID', () => {
  const seen = new Set<string>();
  const initial = tokenUsageTotals([{source: 'estimated', usage: {input_tokens: 1000}}]);
  const first = addUsageOnce(initial, seen, 'request-1', {input_tokens: 7, output_tokens: 2});
  const duplicate = addUsageOnce(first, seen, 'request-1', {input_tokens: 7, output_tokens: 2});
  assert.deepEqual(first, {input: 7, output: 2, total: null, cached: null, reasoning: null});
  assert.equal(duplicate, first);
});

test('rejects stale async loads when selected thread changed or was deleted', () => {
  assert.equal(isCurrentThreadLoad(2, 1), false);
  assert.equal(isCurrentThreadLoad(3, 2), false);
  assert.equal(isCurrentThreadLoad(4, 4), true);
});
