import test from 'node:test';
import assert from 'node:assert/strict';
import {parseGitStatus, parseFileDiffs} from '../src/components/gitDiff.ts';

test('parses NUL-delimited status filenames verbatim, including spaces, tabs and Unicode', () => {
  const files = parseGitStatus('## main\0 M space name.txt\0?? tab\t雪.txt\0');
  assert.deepEqual(files.map(file => file.path), ['space name.txt', 'tab\t雪.txt']);
  assert.deepEqual(files.map(file => file.type), ['modified', 'untracked']);
});

test('parses NUL-delimited rename source and destination without splitting filenames', () => {
  const files = parseGitStatus('## main\0R  new name.txt\0old name.txt\0 D gone.txt\0');
  assert.deepEqual(files, [
    {path: 'new name.txt', previousPath: 'old name.txt', code: 'R', type: 'renamed'},
    {path: 'gone.txt', previousPath: undefined, code: 'D', type: 'deleted'},
  ]);
});

test('keeps diff chunk keys scoped to their full paths and tolerates malformed chunks', () => {
  const diff = 'diff --git a/space name.txt b/space name.txt\nindex 1..2\n--- a/space name.txt\n+++ b/space name.txt\n@@ -1 +1 @@\n-old\n+new\n';
  const parsed = parseFileDiffs(diff);
  assert.equal(parsed['space name.txt'], diff);
  assert.deepEqual(Object.keys(parseFileDiffs('not a diff\n')), []);
});
