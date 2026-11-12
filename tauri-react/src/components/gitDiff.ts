export type GitFileStatus = {
  path: string;
  previousPath?: string;
  code: string;
  type: 'modified' | 'added' | 'deleted' | 'untracked' | 'renamed' | 'other';
};

export type GitDiffFile = {path: string; content: string; binary: boolean};
export type GitChanges = {branch: string; files: GitFileStatus[]; diffs: GitDiffFile[]; truncated: boolean};

export function toGitChanges(status: string, diff: string): GitChanges {
  const files = parseGitStatus(status);
  const chunks = parseFileDiffs(diff, files.map(file => file.path));
  return {
    branch: status.split('\0', 1)[0].replace(/^##\s*/, '').split('\n', 1)[0],
    files,
    diffs: Object.entries(chunks).map(([path, content]) => ({path, content, binary: content.includes('GIT binary patch') || content.includes('Binary files ')})),
    truncated: diff.includes('[diff truncated]'),
  };
}

export function parseGitStatus(statusText: string): GitFileStatus[] {
  if (statusText.includes('\0')) {
    const entries = statusText.split('\0');
    const files: GitFileStatus[] = [];
    for (let i = 0; i < entries.length; i++) {
      const entry = entries[i];
      if (!entry || entry.startsWith('##')) continue;
      const code = entry.slice(0, 2);
      let path = entry.slice(3);
      let previousPath: string | undefined;
      if (code.includes('R') || code.includes('C')) previousPath = entries[++i];
      if (!path) continue;
      const type = code === '??' ? 'untracked' : code.includes('R') || code.includes('C') ? 'renamed' : code.includes('D') ? 'deleted' : code.includes('A') ? 'added' : code.includes('M') ? 'modified' : 'other';
      files.push({path, previousPath, code: code.trim() || 'M', type});
    }
    return files;
  }
  return statusText.split('\n').flatMap(line => {
    if (!line || line.startsWith('##')) return [];
    const code = line.slice(0, 2);
    const path = line.slice(3);
    if (!path) return [];
    const type = code === '??' ? 'untracked' : code.includes('R') ? 'renamed' : code.includes('D') ? 'deleted' : code.includes('A') ? 'added' : code.includes('M') ? 'modified' : 'other';
    return [{path: path.replace(/^(?:\"((?:[^\"\\]|\\.)*)\"|(.+))$/, (_, quoted: string, plain: string) => quoted ? decodeGitQuotedPath(quoted) : plain), code: code.trim() || 'M', type}];
  });
}

function decodeGitQuotedPath(path: string): string {
  return path.replace(/\\([\\\"abfnrtv]|[0-7]{1,3})/g, (_, escaped: string) => {
    const chars: Record<string, string> = {a: '\x07', b: '\b', f: '\f', n: '\n', r: '\r', t: '\t', v: '\v', '\\': '\\', '"': '"'};
    return chars[escaped] ?? String.fromCharCode(parseInt(escaped, 8));
  });
}

export function parseFileDiffs(diffText: string, paths: string[] = []): Record<string, string> {
  const result: Record<string, string> = Object.create(null);
  if (!diffText) return result;
  const chunks = diffText.split(/(?=^diff --git )/m);
  for (const chunk of chunks) {
    if (!chunk.startsWith('diff --git ')) continue;
    const header = chunk.split('\n', 1)[0].slice('diff --git '.length);
    const path = paths.find(candidate => header.endsWith(` b/${candidate}`) || header.endsWith(` b/\"${candidate}\"`));
    if (path) result[path] = chunk;
    else {
      const match = /^a\/(.+) b\/(.+)$/.exec(header);
      if (match && match[1] === match[2]) result[match[2]] = chunk;
      else {
        const quoted = /^\"a\/(.*)\" \"b\/(.*)\"$/.exec(header);
        if (quoted) result[decodeGitQuotedPath(quoted[2])] = chunk;
      }
    }
  }
  return result;
}
