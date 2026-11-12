import test from 'node:test';
import assert from 'node:assert/strict';
import {createRequestOwner} from '../src/components/conversationRequests.ts';

test('only latest request for the still-active project and thread owns updates', () => {
  const owner = createRequestOwner();
  const first = owner.begin('project-a', 'thread-a');
  const second = owner.begin('project-a', 'thread-b');
  assert.equal(owner.isCurrent(first, 'project-a', 'thread-a'), false);
  assert.equal(owner.isCurrent(second, 'project-a', 'thread-b'), true);
  assert.equal(owner.isCurrent(second, 'project-b', 'thread-b'), false);
});

test('project switch and invalidation revoke pending requests', () => {
  const owner = createRequestOwner();
  const request = owner.begin('project-a', 'thread-a');
  assert.equal(owner.isCurrent(request, 'project-b', 'thread-a'), false);
  owner.invalidate();
  assert.equal(owner.isCurrent(request, 'project-a', 'thread-a'), false);
});
