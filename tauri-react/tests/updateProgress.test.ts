import test from 'node:test';
import assert from 'node:assert/strict';
import {createDownloadProgress} from '../src/components/Settings/updateProgress.ts';

test('accumulates chunk lengths and clamps percentages', () => {
  const progress = createDownloadProgress();
  assert.deepEqual(progress.add(40, 100), {downloaded: 40, total: 100, percent: 40});
  assert.deepEqual(progress.add(80), {downloaded: 120, total: 100, percent: 100});
});

test('unknown or invalid sizes remain indeterminate', () => {
  const progress = createDownloadProgress();
  assert.deepEqual(progress.add(120), {downloaded: 120, total: null, percent: null});
  assert.deepEqual(progress.add(-5, -1), {downloaded: 120, total: null, percent: null});
});
