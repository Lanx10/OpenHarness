import test from 'node:test';
import assert from 'node:assert/strict';
import { upsertStep } from '../src/components/stepLifecycle.ts';
import type { StepEventLike } from '../src/components/StepEvent.ts';
import { cleanMessageText } from '../src/utils/cleanMessageText.ts';

test('upsertStep adds new steps in order', () => {
  const initial: StepEventLike[] = [];
  const step1: StepEventLike = {
    id: 'step-1',
    kind: 'read',
    label: 'Reading src/App.tsx',
    status: 'running',
  };
  const step2: StepEventLike = {
    id: 'step-2',
    kind: 'grep',
    label: 'Grepping "Canvas"',
    status: 'running',
  };

  const next1 = upsertStep(initial, step1);
  assert.equal(next1.length, 1);
  assert.equal(next1[0].id, 'step-1');

  const next2 = upsertStep(next1, step2);
  assert.equal(next2.length, 2);
  assert.equal(next2[1].id, 'step-2');
});

test('upsertStep updates status and detail on existing step by id', () => {
  const initial: StepEventLike[] = [
    { id: 'call_1', kind: 'command', label: 'Running cargo check', status: 'running' },
  ];
  const updated: StepEventLike = {
    id: 'call_1',
    kind: 'command',
    label: 'Running cargo check',
    status: 'done',
    detail: 'exit code 0',
  };

  const result = upsertStep(initial, updated);
  assert.equal(result.length, 1);
  assert.equal(result[0].status, 'done');
  assert.equal(result[0].detail, 'exit code 0');
});

test('cleanMessageText strips tool_call', () => {
  const input = `Here you go.
<​tool_call>
{"name": "read_file", "arguments": {"path": "APP-COMPLETION-AUDIT.md"}}
</​tool_call>`;
  const out = cleanMessageText(input);
  assert.equal(out, 'Here you go.');
});

test('cleanMessageText strips raw tool call objects', () => {
  const capped = 'done ' + JSON.stringify({name:'read_file', arguments:{path:'src/main.rs'}});
  assert.equal(cleanMessageText(capped).trim(), 'done');
});

test('upsertStep handles approval to running transition', () => {
  const initial: StepEventLike[] = [
    { id: 'app_1', kind: 'edit', label: 'Editing src/main.rs', status: 'approval' },
  ];
  const approved: StepEventLike = {
    id: 'app_1',
    kind: 'edit',
    label: 'Editing src/main.rs',
    status: 'running',
  };
  const result = upsertStep(initial, approved);
  assert.equal(result[0].status, 'running');
});
