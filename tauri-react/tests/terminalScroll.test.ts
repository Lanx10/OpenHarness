import test from 'node:test';
import assert from 'node:assert/strict';
import {isNearBottom} from '../src/components/conversationScroll.ts';

test('terminal near-bottom detection keeps a reader detached from new output', () => {
  assert.equal(isNearBottom({scrollTop: 800, clientHeight: 200, scrollHeight: 1000}), true);
  assert.equal(isNearBottom({scrollTop: 500, clientHeight: 200, scrollHeight: 1000}), false);
});
