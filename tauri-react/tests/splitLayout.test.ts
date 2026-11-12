import assert from 'node:assert/strict';
import test from 'node:test';
import {getSplitLayout, MIN_CONVERSATION_WIDTH, MIN_SECONDARY_PANEL_WIDTH, PANEL_GAP} from '../src/components/splitLayout.ts';

test('uses a single panel below the minimum split width', () => {
  for (const width of [0, 480, 768, MIN_CONVERSATION_WIDTH + 2 * MIN_SECONDARY_PANEL_WIDTH + PANEL_GAP - 1]) {
    assert.equal(getSplitLayout(width, true, 420, 420).enabled, false);
  }
});

test('keeps both panels and conversation above their minimums at minimum split width', () => {
  const width = MIN_CONVERSATION_WIDTH + 2 * MIN_SECONDARY_PANEL_WIDTH + PANEL_GAP;
  const layout = getSplitLayout(width, true, 420, 420);
  assert.equal(layout.enabled, true);
  assert.ok(layout.filesWidth >= MIN_SECONDARY_PANEL_WIDTH);
  assert.ok(layout.gitWidth >= MIN_SECONDARY_PANEL_WIDTH);
  assert.ok(width - layout.sidePanelWidth >= MIN_CONVERSATION_WIDTH);
});

test('scales panels proportionally when space is constrained and fits saved widths otherwise', () => {
  const tight = getSplitLayout(1100, true, 500, 500);
  assert.equal(tight.enabled, true);
  assert.ok(tight.filesWidth >= MIN_SECONDARY_PANEL_WIDTH);
  assert.ok(tight.gitWidth >= MIN_SECONDARY_PANEL_WIDTH);
  assert.ok(1100 - tight.sidePanelWidth >= MIN_CONVERSATION_WIDTH);
  const large = getSplitLayout(1800, true, 420, 420);
  assert.deepEqual(large, {enabled: true, filesWidth: 420, gitWidth: 420, sidePanelWidth: 840});
});

test('preserves the active single panel when split is disabled or cannot fit', () => {
  assert.deepEqual(getSplitLayout(1000, false, 420, 460), {enabled: false, filesWidth: 420, gitWidth: 460, sidePanelWidth: 0});
  assert.deepEqual(getSplitLayout(1000, true, 420, 460), {enabled: false, filesWidth: 420, gitWidth: 460, sidePanelWidth: 0});
});
