import test from 'node:test';
import assert from 'node:assert/strict';
import {parseSavedModel, reconcileModelSelection} from '../src/components/providerSelection.ts';

test('ignores malformed or invalid saved preferences', () => {
  assert.equal(parseSavedModel('{'), null);
  assert.equal(parseSavedModel('{"id":3,"provider":"P"}'), null);
});

test('keeps missing saved models unavailable, not selectable', () => {
  const models = [{id: 'live', provider: 'P'}];
  const result = reconcileModelSelection(null, models, {id: 'gone', provider: 'P'});
  assert.equal(result.selected, null);
  assert.equal(result.unavailable, true);
  assert.deepEqual(result.models, models);
});

test('preserves a current valid selection and selects first when no preference exists', () => {
  const models = [{id: 'a', provider: 'P'}, {id: 'b', provider: 'P'}];
  assert.equal(reconcileModelSelection(models[1], models, null).selected, models[1]);
  assert.equal(reconcileModelSelection(null, models, null).selected, models[0]);
});

test('clears removed selections without choosing a different model silently', () => {
  const result = reconcileModelSelection({id: 'gone', provider: 'P'}, [{id: 'a', provider: 'Q'}], null);
  assert.equal(result.selected, null);
  assert.equal(result.unavailable, true);
});
