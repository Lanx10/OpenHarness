import {useMemo, useState} from 'react';
import {parseGitStatus, parseFileDiffs} from './gitDiff';

type GitFileStatus = import('./gitDiff').GitFileStatus;

const MAX_RENDERED_DIFF_LINES = 20_000;

function diffLines(text: string): string[] {
  return text.split('\n', MAX_RENDERED_DIFF_LINES + 1).slice(0, MAX_RENDERED_DIFF_LINES);
}

function classifyDiffLine(line: string): string {
  if (line.startsWith('+++ ') || line.startsWith('--- ')) return 'diff-line diff-file-target';
  if (line.startsWith('+')) return 'diff-line diff-added';
  if (line.startsWith('-')) return 'diff-line diff-removed';
  if (line.startsWith('@@')) return 'diff-line diff-hunk';
  if (line.startsWith('diff --git')) return 'diff-line diff-header';
  return 'diff-line';
}

function isBinaryDiff(text: string): boolean {
  return text.includes('GIT binary patch') || text.includes('Binary files ') || text.includes('\ufffd');
}

function isGitStatusError(text: string): boolean {
  return text.startsWith('Error:') || text.startsWith('Git error:');
}

function isGitDiffError(text: string): boolean {
  return text.startsWith('Error:') || text.startsWith('Git error:');
}

export {parseGitStatus, parseFileDiffs};
export type {GitFileStatus};

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

  const statusError = isGitStatusError(workspaceStatus);
  const diffError = isGitDiffError(gitDiff);
  const changedFiles = useMemo(() => statusError ? [] : parseGitStatus(workspaceStatus), [workspaceStatus, statusError]);
  const fileDiffs = useMemo(() => parseFileDiffs(gitDiff, changedFiles.map(file => file.path)), [gitDiff, changedFiles]);
  const renderedLines = useMemo(() => diffLines(gitDiff), [gitDiff]);
  const truncated = gitDiff.split('\n').length > renderedLines.length;

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

  if (statusError || diffError) {
    return <div className="file-preview-state" role="alert"><p>Git could not load this repository.</p><pre>{statusError ? workspaceStatus : gitDiff}</pre><button type="button" onClick={onRefresh}>Retry</button></div>;
  }

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
          isBinaryDiff(activeDiffContent) ? <div className="file-preview-state"><p>Binary file; text diff is unavailable.</p></div> : <>
            <pre className="git-diff-pre">
              {(selectedFile ? diffLines(activeDiffContent) : renderedLines).map((line, idx) => <span key={idx} className={classifyDiffLine(line)}>{line}{'\n'}</span>)}
            </pre>
            {!selectedFile && truncated && <div className="file-preview-state" role="status">Diff output truncated for display. Refresh or inspect individual files for complete changes.</div>}
          </>
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
