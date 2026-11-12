import test from 'node:test';
import assert from 'node:assert/strict';
import {isNearBottom} from '../src/components/conversationScroll.ts';

test('detects near-bottom position with a tolerance and clamps at the top', () => {
  assert.equal(isNearBottom({scrollTop: 728, clientHeight: 200, scrollHeight: 1000}), true);
  assert.equal(isNearBottom({scrollTop: 727, clientHeight: 200, scrollHeight: 1000}), false);
  assert.equal(isNearBottom({scrollTop: 0, clientHeight: 200, scrollHeight: 1000}), false);
});
