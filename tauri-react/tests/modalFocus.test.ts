import test from 'node:test';
import assert from 'node:assert/strict';
import {containTabKey, focusableElements} from '../src/components/modalFocus.ts';

function element(hidden = false, rects = 1) {
  return {hidden, getAttribute: () => null, getClientRects: () => Array(rects), focus() { focused = this; }, contains: () => true};
}
let focused: ReturnType<typeof element> | null;

test('filters hidden and disabled controls from focus order', () => {
  const visible = element();
  const hidden = element(true);
  const noLayout = element(false, 0);
  assert.deepEqual(focusableElements({querySelectorAll: () => [visible, hidden, noLayout]} as unknown as ParentNode), [visible]);
});

test('wraps forward and reverse Tab at dialog boundaries', () => {
  const first = element();
  const last = element();
  const documentStub = {activeElement: last};
  const dialog = {
    querySelectorAll: () => [first, last],
    ownerDocument: documentStub,
    contains: () => true,
    focus() {},
  } as unknown as HTMLElement;
  let prevented = false;
  containTabKey({key: 'Tab', shiftKey: false, preventDefault: () => { prevented = true; }}, dialog);
  assert.equal(prevented, true);
  assert.equal(focused, first);
  (dialog.ownerDocument as Document).activeElement = first;
  prevented = false;
  containTabKey({key: 'Tab', shiftKey: true, preventDefault: () => { prevented = true; }}, dialog);
  assert.equal(prevented, true);
  assert.equal(focused, last);
});
