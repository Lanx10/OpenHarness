import test from 'node:test';
import assert from 'node:assert/strict';
import { readQueuedPrompts, upsertStep, markPromptActive, estimateMessageTokens, removeQueuedPrompt, nextQueuedPrompt, queuedPromptById, queuedPromptsForThread, persistQueuedPrompts, type QueuedPrompt } from '../src/components/stepLifecycle.ts';
import type { StepEventLike } from '../src/components/StepEvent.ts';
import { cleanMessageText } from '../src/utils/cleanMessageText.ts';
import { withFailureNotice } from '../src/components/stepLifecycle.ts';

 test('failure notice preserves partial assistant output and is idempotent', () => {
  const partial = 'First paragraph.\n\nSecond paragraph';
  const failed = withFailureNotice(partial, 'Provider request failed');
  assert.ok(failed.startsWith(partial));
  assert.ok(failed.includes('Provider request failed'));
  assert.equal(withFailureNotice(failed, 'Provider request failed'), failed);
});

 test('failure notice remains visible when no assistant output arrived', () => {
  assert.equal(withFailureNotice('', 'Provider request failed'), 'Provider request failed');
});


test('restores queued prompts with attachments, model context and selected skills', () => {
  const prompt: QueuedPrompt = {
    id: 'queued-1',
    text: 'Inspect this image',
    attachments: ['![diagram](data:image/png;base64,aGVsbG8=)'],
    modelId: 'vision-model',
    provider: 'OpenRouter',
    supportsVision: true,
    modelContextTokens: 128_000,
    modelOutputTokens: 4_096,
    projectId: 'project-1',
    threadId: 'thread-1',
    selectedSkills: ['review'],
  };
  const storage = {getItem: () => JSON.stringify([prompt])};

  assert.deepEqual(readQueuedPrompts(storage, 'queue'), [prompt]);
});

test('queued prompts stay isolated to their originating thread', () => {
  const prompt = (id: string, threadId: string, text: string): QueuedPrompt => ({
    id,
    text,
    attachments: [],
    modelId: 'model',
    provider: 'OpenRouter',
    supportsVision: false,
    modelContextTokens: 128_000,
    modelOutputTokens: 4_096,
    projectId: 'project-1',
    threadId,
    selectedSkills: [],
  });
  const threadOne = prompt('one', 'thread-1', 'first thread');
  const threadTwo = prompt('two', 'thread-2', 'second thread');

  assert.deepEqual(queuedPromptsForThread([threadOne, threadTwo], 'thread-2'), [threadTwo]);
  assert.deepEqual(queuedPromptsForThread([threadOne], null), []);
});

test('removing a queued prompt only removes its exact queue item', () => {
  const first = {id: 'active:first', text: 'first'} as QueuedPrompt;
  const second = {id: 'second', text: 'second'} as QueuedPrompt;
  assert.deepEqual(removeQueuedPrompt([first, second], 'first'), [second]);
  assert.deepEqual(removeQueuedPrompt([first, second], 'missing'), [first, second]);
});

test('queue drain skips already-active and missing entries', () => {
  const active = {id: 'active:running', text: 'running'} as QueuedPrompt;
  const pending = {id: 'pending', text: 'pending'} as QueuedPrompt;
  assert.equal(nextQueuedPrompt([active, pending])?.id, 'pending');
  assert.equal(nextQueuedPrompt([active]), undefined);
  assert.equal(nextQueuedPrompt([active, {...pending, threadId: 'thread-b'}].filter(prompt => prompt.threadId === 'thread-a')), undefined);
  assert.equal(nextQueuedPrompt([{...active, threadId: 'thread-a'}, {...pending, threadId: 'thread-a'}])?.id, pending.id);
});

test('queued prompt lookup resolves active marker to its owner prompt', () => {
  const prompt = {id: 'active:prompt-1', text: 'continue'} as QueuedPrompt;
  assert.equal(queuedPromptById([prompt], 'prompt-1')?.id, prompt.id);
});

test('persisting an oversized queue clears the stale stored queue', () => {
  const prompt = {id: 'large', text: 'x'} as QueuedPrompt;
  let stored: string | null = JSON.stringify([prompt]);
  const storage = {
    setItem: (_key: string, value: string) => { stored = value; },
    removeItem: () => { stored = null; },
  };

  assert.equal(persistQueuedPrompts(storage, 'queue', [prompt], 2), false);
  assert.equal(stored, null);
});

test('message token estimate includes attachments and message overhead', () => {
  assert.equal(estimateMessageTokens('', []), 0);
  assert.equal(estimateMessageTokens('1234', []), 10);
  assert.equal(estimateMessageTokens('1234', ['5678']), 11);
});

test('active prompt markers keep queued prompt data and are idempotent', () => {
  const prompt: QueuedPrompt = {
    id: 'prompt-1',
    text: 'Continue the build',
    attachments: [],
    modelId: 'model',
    provider: 'OpenRouter',
    supportsVision: false,
    modelContextTokens: 128_000,
    modelOutputTokens: 4_096,
    selectedSkills: [],
  };

  const active = markPromptActive(prompt);
  assert.equal(active.id, 'active:prompt-1');
  assert.equal(markPromptActive(active).id, active.id);
  assert.equal(active.text, prompt.text);
  assert.deepEqual(markPromptActive(active), active);
});

test('drops malformed queued prompt records without throwing', () => {
  assert.deepEqual(readQueuedPrompts({getItem: () => '[{"id":"broken"}]'}, 'queue'), []);
  assert.deepEqual(readQueuedPrompts({getItem: () => JSON.stringify([{id:'invalid-limit',text:'x',attachments:[],modelId:'m',provider:'p',supportsVision:false,modelContextTokens:0,modelOutputTokens:1,projectId:null,threadId:null,selectedSkills:[]}])}, 'queue'), []);
  assert.deepEqual(readQueuedPrompts({getItem: () => 'not-json'}, 'queue'), []);
});

test('learning candidates persist in a thread until explicit approval', () => {
  const candidate = {summary:'Keep test command', memory:['Run npm test after UI changes'], skill_name:null, skill_content:null, memory_saved:false, skill_saved:false};
  const thread = {id:'t1', project_id:'p1', learning_candidates:[candidate]};
  assert.deepEqual(thread.learning_candidates[0], candidate);
  assert.equal(thread.learning_candidates[0].memory_saved, false);
});

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

test('cleanMessageText strips direct patch_file calls', () => {
  const raw = 'Updated. ' + JSON.stringify({name:'patch_file',arguments:{path:'src/main.rs',old_text:'old',new_text:'new'}});
  assert.equal(cleanMessageText(raw), 'Updated.');
  const fenced = 'Updated. ```json ' + JSON.stringify({name:'patch_file',arguments:{path:'src/main.rs',old_text:'old',new_text:'new'}}) + '```';
  assert.equal(cleanMessageText(fenced), 'Updated.');
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
