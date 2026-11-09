import {useMemo, useState} from 'react';

type GitFileStatus = {
  path: string;
  code: string;
  type: 'modified' | 'added' | 'deleted' | 'untracked' | 'renamed' | 'other';
};

function parseGitStatus(statusText: string): GitFileStatus[] {
  return statusText.split('\n').flatMap((rawLine) => {
    const line = rawLine.trimEnd();
    if (!line || line.startsWith('##')) return [];
    const code = line.slice(0, 2).trim();
    let path = line.slice(2).trim();
    if (path.startsWith('"') && path.endsWith('"')) {
      path = path.slice(1, -1);
    }
    if (!path) return [];

    let type: GitFileStatus['type'] = 'other';
    if (code === '??') type = 'untracked';
    else if (code.includes('M')) type = 'modified';
    else if (code.includes('A')) type = 'added';
    else if (code.includes('D')) type = 'deleted';
    else if (code.includes('R')) {
      type = 'renamed';
      const target = (path.split(' -> ').at(-1) || path).trim();
      path = target.startsWith('"') && target.endsWith('"') ? target.slice(1, -1) : target;
    }

    return [{path, code: code || 'M', type}];
  });
}

function parseFileDiffs(diffText: string): Record<string, string> {
  const result: Record<string, string> = {};
  if (!diffText || !diffText.trim()) return result;

  const chunks = diffText.split(/^diff --git /m);
  for (const chunk of chunks) {
    if (!chunk.trim()) continue;
    const fullChunk = 'diff --git ' + chunk;
    const firstLine = chunk.split('\n')[0] || '';
    const match = firstLine.match(/a\/(.+?)\s+b\/(.+)/);
    if (match && match[2]) {
      result[match[2].trim()] = fullChunk;
    } else {
      const bMatch = firstLine.match(/b\/(.+)/);
      if (bMatch) {
        result[bMatch[1].trim()] = fullChunk;
      }
    }
  }
  return result;
}

export function GitDiffIcon({width = 16, height = 16}: {width?: number; height?: number}) {
  return (
    <svg viewBox="0 0 16 16" width={width} height={height} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <circle cx="4" cy="4" r="2" />
      <circle cx="4" cy="12" r="2" />
      <circle cx="12" cy="7" r="2" />
      <path d="M4 6v4M4 7.5a4.5 4.5 0 0 1 4.5-1.5H10" />
    </svg>
  );
}

export function WorkspaceGitDiff({
  projectId,
  workspaceStatus,
  gitDiff,
  onRefresh,
}: {
  projectId: string | null;
  workspaceStatus: string;
  gitDiff: string;
  onRefresh: () => void;
}) {
  const [selectedFile, setSelectedFile] = useState<string | null>(null);

  const changedFiles = useMemo(() => parseGitStatus(workspaceStatus), [workspaceStatus]);
  const fileDiffs = useMemo(() => parseFileDiffs(gitDiff), [gitDiff]);

  if (!projectId) {
    return (
      <div className="workspace-files-empty">
        <GitDiffIcon width={32} height={32} />
        <p>Select a project to view Git diff.</p>
      </div>
    );
  }

  const selectedInfo = changedFiles.find((f) => f.path === selectedFile);
  const activeDiffContent = selectedFile ? fileDiffs[selectedFile] : gitDiff;
  const isClean = !gitDiff.trim() && changedFiles.length === 0;

  if (isClean) {
    return (
      <div className="workspace-files-empty">
        <GitDiffIcon width={32} height={32} />
        <p>Working tree clean. No uncommitted changes.</p>
        <button type="button" className="quick-pill" onClick={onRefresh} style={{marginTop: 8}}>
          Check status
        </button>
      </div>
    );
  }

  return (
    <div className="workspace-files git-diff-view">
      <div className="workspace-file-list">
        <div className="file-list-heading" style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center'}}>
          <span>CHANGES ({changedFiles.length})</span>
          <button type="button" onClick={onRefresh} title="Refresh Git status" style={{background: 'transparent', border: 0, color: 'var(--muted)', cursor: 'pointer', fontSize: 10}}>↻</button>
        </div>
        <button
          type="button"
          className={`file-entry ${selectedFile === null ? 'selected' : ''}`}
          onClick={() => setSelectedFile(null)}
        >
          <span className="file-entry-icon" aria-hidden="true">✱</span>
          <span className="file-entry-name">All changes</span>
        </button>
        {changedFiles.map((file) => {
          const isSelected = selectedFile === file.path;
          return (
            <button
              key={file.path}
              type="button"
              className={`file-entry git-file-entry ${isSelected ? 'selected' : ''}`}
              onClick={() => setSelectedFile(file.path)}
              title={`${file.code} ${file.path}`}
            >
              <span className={`git-status-badge-inline git-status-${file.type}`}>
                {file.code}
              </span>
              <span className="file-entry-name">{file.path}</span>
            </button>
          );
        })}
      </div>
      <div className="workspace-file-content">
        <div className="file-preview-heading">
          {selectedFile ? selectedFile : 'All changes in working tree'}
        </div>
        {activeDiffContent ? (
          <pre className="git-diff-pre">
            {activeDiffContent.split('\n').map((line, idx) => {
              let lineClass = 'diff-line';
              if (line.startsWith('+++ ') || line.startsWith('--- ')) {
                lineClass = 'diff-line diff-file-target';
              } else if (line.startsWith('+')) {
                lineClass = 'diff-line diff-added';
              } else if (line.startsWith('-')) {
                lineClass = 'diff-line diff-removed';
              } else if (line.startsWith('@@')) {
                lineClass = 'diff-line diff-hunk';
              } else if (line.startsWith('diff --git')) {
                lineClass = 'diff-line diff-header';
              }
              return (
                <span key={idx} className={lineClass}>
                  {line}
                  {'\n'}
                </span>
              );
            })}
          </pre>
        ) : (
          <div className="file-preview-state">
            {selectedInfo?.type === 'untracked' ? (
              <>
                <GitDiffIcon width={24} height={24} />
                <p>Untracked file: <code>{selectedFile}</code></p>
                <span style={{fontSize: 11, color: 'var(--muted)'}}>This file is newly created and not yet committed to Git.</span>
              </>
            ) : (
              <p>No diff content to display for {selectedFile || 'current selection'}.</p>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
