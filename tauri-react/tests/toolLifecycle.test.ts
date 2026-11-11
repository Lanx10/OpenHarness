import test from 'node:test';
import assert from 'node:assert/strict';
import { readQueuedPrompts, upsertStep, markPromptActive, estimateMessageTokens, type QueuedPrompt } from '../src/components/stepLifecycle.ts';
import type { StepEventLike } from '../src/components/StepEvent.ts';
import { cleanMessageText } from '../src/utils/cleanMessageText.ts';

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

test('message token estimate includes attachments and is empty-safe', () => {
  assert.equal(estimateMessageTokens('', []), 0);
  assert.equal(estimateMessageTokens('1234', []), 1);
  assert.equal(estimateMessageTokens('1234', ['5678']), 3);
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
