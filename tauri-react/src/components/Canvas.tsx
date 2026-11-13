import {useEffect, useLayoutEffect, useRef, useState} from 'react';
import {isNearBottom} from './conversationScroll';
import {createRequestOwner} from './conversationRequests';
import {CHAT_PANEL_GAP, getSplitLayout} from './splitLayout';
import {containTabKey} from './modalFocus';
import {withFailureNotice} from './stepLifecycle';
import { SafeMarkdown } from './SafeMarkdown';
import { MessageCrumbs } from './MessageCrumbs';
import { readQueuedPrompts, upsertStep, markPromptActive, estimateMessageTokens, removeQueuedPrompt, nextQueuedPrompt, queuedPromptById, queuedPromptsForThread, persistQueuedPrompts, type QueuedPrompt } from './stepLifecycle';
import type { Message } from '../types/message';
import {addUsageOnce, emptyTokenUsage, isCurrentThreadLoad, tokenUsageTotals, type ProviderUsageLike} from './tokenUsage';
import { cleanMessageText } from '../utils/cleanMessageText';
import {isTauri} from '@tauri-apps/api/core';
import {PlusIcon, ComputerDesktopIcon, ClipboardDocumentIcon, ArrowPathIcon, TrashIcon, ChevronLeftIcon, ChevronRightIcon, DocumentTextIcon, SquaresPlusIcon, ChatBubbleLeftIcon} from '@heroicons/react/20/solid';
import {Composer} from './Composer';
import {WorkspaceGitDiff, GitDiffIcon} from './WorkspaceGitDiff';
import {BrowserSidePanel} from './BrowserSidePanel';
import {
  apiChatStream,
  apiCompactThread,
  apiListProviders,
  type ProviderInfo,
  apiWorkspaceListDir,
  apiWorkspaceReadFile,
  type FileEntryLike,
  apiCreateThread,
  apiCancelChatStream,
  apiListThreads,
  apiGetThread,
  apiDeleteThread,
  apiGetProjects,
  apiAppendMessage,
  apiUpdateThreadLearning,
  apiDismissThreadLearning,
  apiEditMessage,
  apiDeleteMessage,
  apiRespondToToolApproval,
  apiGitStatus,
  apiGitDiff,
  apiTerminalCreate,
  apiTerminalKill,
  apiTerminalWrite,
  apiListenTerminalOutput,
  apiListenTerminalExit,
  type ChatThreadLike,
  type ChatMessageLike,
  type ApprovalRequestLike,
} from '../services/tauriBridge';



function toast(message: string) {
  const notice = document.getElementById('notice');
  if (notice) {
    notice.textContent = message;
    window.setTimeout(() => { if (notice.textContent === message) notice.textContent = ''; }, 2500);
  }
}


export function Hero({style, className}: {style?: React.CSSProperties; className?: string}) {
  const [localDetail, setLocalDetail] = useState('Open Settings → Providers to add OpenRouter, Google AI Studio, or Ollama.');
  return (
    <section className={`hero ${className || ''}`} aria-hidden="false" style={style}>
      <div className="hero-mark" aria-hidden="true">
        <img src="/appicon/openharness-desktop-mark-only-512-v2.svg" alt="OpenHarness" width={38} height={38} style={{display: 'block'}} />
      </div>
      <h1 data-od-id="hero-heading">What can I help build?</h1>
      <p className="hero-note">Start with a task, or pick up where you left off.</p>
      <div className="quick-actions">
        <button className="quick-pill" id="quick-project" type="button" data-od-id="quick-project" onClick={() => window.dispatchEvent(new Event('openharness:new-project'))}>
          <PlusIcon width={10} height={10} aria-hidden="true" /> New project
        </button>
        <button type="button" className="quick-pill local" onClick={() => { setLocalDetail('Use Providers to configure Ollama, OpenRouter, or Google AI Studio.'); window.dispatchEvent(new CustomEvent('openharness:open-settings', {detail: {section: 'Providers'}})); }} title={localDetail}>
          <ComputerDesktopIcon width={11} height={11} aria-hidden="true" /> Providers
        </button>
      </div>
    </section>
  );
}

function WorkspaceFiles({projectId}: {projectId: string | null}) {
  const [files, setFiles] = useState<FileEntryLike[]>([]);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [content, setContent] = useState('');
  const [error, setError] = useState('');
  const [directory, setDirectory] = useState('');
  const requestId = useRef(0);

  useEffect(() => {
    let cancelled = false;
    if (!projectId) {
      setFiles([]); setSelectedPath(null); setContent(''); setDirectory('');
      setError('Select a project to browse files.');
      return () => { cancelled = true; };
    }
    void apiWorkspaceListDir(projectId, directory || undefined).then(items => {
      if (!cancelled) { setFiles(items); setSelectedPath(null); setContent(''); setError(''); }
    }).catch(error => { if (!cancelled) setError(String(error)); });
    return () => { cancelled = true; };
  }, [projectId, directory]);

  const openFile = async (path: string) => {
    if (!projectId) return;
    const currentRequest = ++requestId.current;
    setSelectedPath(path); setContent(''); setError('');
    try {
      const text = await apiWorkspaceReadFile(projectId, path);
      if (currentRequest === requestId.current) setContent(text);
    } catch (error) {
      if (currentRequest === requestId.current) setError(String(error));
    }
  };

  const openDirectory = (path: string) => { setDirectory(path); setSelectedPath(null); setContent(''); setError(''); };
  const parent = directory.replace(/[\\/][^\\/]+$/, '');

  if (!projectId) {
    return (
      <div className="workspace-files-empty">
        <DocumentTextIcon width={32} height={32} />
        <p>Select a project to browse files.</p>
      </div>
    );
  }

  return <div className="workspace-files">
    <div className="workspace-file-list">
      <div className="file-list-heading">EXPLORER</div>
      {directory && <button type="button" className="file-entry file-parent" onClick={() => openDirectory(parent)}>‹ <span>Up one level</span></button>}
      {!files.length && !error && <p className="file-empty">This folder is empty.</p>}
      {files.map(file => <button key={file.path} type="button" className={`file-entry ${file.is_dir ? 'is-directory' : 'is-file'} ${selectedPath === file.path ? 'selected' : ''}`} onClick={() => file.is_dir ? openDirectory(file.path) : void openFile(file.path)}><span className="file-entry-icon" aria-hidden="true">{file.is_dir ? '▸' : '·'}</span><span className="file-entry-name">{file.name}</span></button>)}
    </div>
    <div className="workspace-file-content">{selectedPath ? <><div className="file-preview-heading">{selectedPath}</div>{error ? <p className="file-preview-state">{error}</p> : content ? <pre>{content}</pre> : <p className="file-preview-state">Loading preview…</p>}</> : <div className="file-preview-state">{error ? <p>{error}</p> : <><DocumentTextIcon width={22} height={22}/><span>Select a file to preview</span></>}</div>}</div>
  </div>;
}

function renderAttachment(item: string, index: number) {
  const imgMatch = item.match(/^!\[([^\]]*)\]\((data:image\/[^)]+|https?:\/\/[^)]+)\)$/);
  if (imgMatch) {
    const [, alt, src] = imgMatch;
    return (
      <figure key={index} className="message-attachment-image-wrap">
        <img src={src} alt={alt || 'Attached image'} className="message-attachment-image" loading="lazy" />
        {alt && <figcaption>{alt}</figcaption>}
      </figure>
    );
  }
  const fileTitle = item.startsWith('[') && item.includes(']') ? item.slice(1, item.indexOf(']')) : item;
  return <span className="attachment-pill" key={index}>{fileTitle}</span>;
}

type MessageProps = {message: Message; onCopy: () => void; onDelete: () => void; onRegenerate: () => void; onSaveEdit: (text: string) => void};

function MessageView({message, onCopy, onDelete, onRegenerate, onSaveEdit}: MessageProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(message.text);
  useEffect(() => setDraft(message.text), [message.text]);

  if (message.role === 'user') {
    return (
      <div className="chatgpt-user-row">
        <div className="chatgpt-user-group">
          <article className="chatgpt-user-bubble">
            {message.attachments?.length ? (
              <div className="message-attachments">
                {message.attachments.map((file, index) => renderAttachment(file, index))}
              </div>
            ) : null}
            <div className="message-content">
              {editing ? (
                <textarea value={draft} onChange={e => setDraft(e.target.value)} aria-label="Edit message" />
              ) : (
                <p>{message.text}</p>
              )}
            </div>
            {(message.status === 'queued' || message.status === 'cancelled' || message.status === 'error') && <span className={`message-state message-state-${message.status}`}>{message.status === 'queued' ? 'Queued' : message.status === 'cancelled' ? 'Stopped' : 'Failed'}</span>}
          </article>
          <div className="chatgpt-user-actions">
            <button type="button" onClick={onCopy} aria-label="Copy" title="Copy"><ClipboardDocumentIcon width={13}/></button>
            <button type="button" onClick={() => { if (editing) { onSaveEdit(draft); setEditing(false); } else setEditing(true); }} aria-label={editing ? 'Save edit' : 'Edit'} title="Edit message">{editing ? 'Save' : 'Edit'}</button>
            <button type="button" onClick={onDelete} aria-label="Delete" title="Delete message"><TrashIcon width={13}/></button>
          </div>
        </div>
      </div>
    );
  }

  const isThinking = message.status === 'thinking';
  const hasText = Boolean(message.text.trim());
  const showBody = hasText || editing || isThinking;

  return (
    <div className={`chatgpt-assistant-row${isThinking ? ' is-thinking' : ''}${message.status === 'error' ? ' is-error' : ''}`}>
      <div className="chatgpt-assistant-avatar" aria-hidden="true">
        <span className="chatgpt-avatar-mark">OH</span>
      </div>
      <div className="chatgpt-assistant-main">
        <MessageCrumbs steps={message.steps} thinking={isThinking && (!message.steps || message.steps.every(step => step.status !== 'running'))} />
        {showBody && (
          <article className="chatgpt-assistant-body">
            {message.attachments?.length ? (
              <div className="message-attachments">
                {message.attachments.map((file, index) => renderAttachment(file, index))}
              </div>
            ) : null}
            <div className="message-content">
              {editing ? (
                <textarea value={draft} onChange={e => setDraft(e.target.value)} aria-label="Edit message" />
              ) : (
                <SafeMarkdown content={cleanMessageText(message.text)} />
              )}
            </div>
          </article>
        )}
        {hasText && (
          <div className="chatgpt-assistant-actions">
            <button type="button" onClick={onCopy} aria-label="Copy" title="Copy"><ClipboardDocumentIcon width={13}/></button>
            <button type="button" onClick={onRegenerate} aria-label="Regenerate" title="Regenerate"><ArrowPathIcon width={13}/></button>
            <button type="button" onClick={() => { if (editing) { onSaveEdit(draft); setEditing(false); } else setEditing(true); }} aria-label={editing ? 'Save edit' : 'Edit'} title="Edit message">{editing ? 'Save' : 'Edit'}</button>
            <button type="button" onClick={onDelete} aria-label="Delete" title="Delete message"><TrashIcon width={13}/></button>
          </div>
        )}
      </div>
    </div>
  );
}

type TerminalShell = 'powershell' | 'cmd' | 'zsh' | 'bash' | 'sh' | 'fish';
type TerminalTab = {id: string; label: string; shell: TerminalShell; output: string; status: 'running'|'exited'};

function LearningProposalCard({candidate, onApprove, onDismiss, saving}: {candidate: NonNullable<ChatThreadLike['learning_candidates']>[number]; onApprove: () => void; onDismiss: () => void; saving: boolean}) {
  return <section className="learning-proposal" aria-label="Project learning proposal">
    <div className="learning-proposal-head"><strong>Project learning proposal</strong><span>Nothing saved yet</span></div>
    <p>{candidate.summary}</p>
    {candidate.memory.length > 0 && <div><strong>Memory notes</strong><ul>{candidate.memory.map((item, index) => <li key={`${index}:${item}`}>{item}</li>)}</ul></div>}
    {candidate.skill_name && <div><strong>Project skill: {candidate.skill_name}</strong><p>{candidate.skill_description}</p><pre>{candidate.skill_content}</pre></div>}
    <div className="learning-proposal-actions"><button type="button" disabled={saving} onClick={onApprove}>{saving ? 'Saving…' : 'Approve and save to project'}</button><button type="button" disabled={saving} onClick={onDismiss}>Dismiss</button></div>
    <small>Approval writes only to this project’s AGENTS.md and/or .agents/skills/ directory.</small>
  </section>;
}

export function Canvas() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [threads, setThreads] = useState<ChatThreadLike[]>([]);
  const [threadId, setThreadId] = useState<string | null>(null);
  const threadIdRef = useRef<string | null>(null);
  const updateThreadId = (id: string | null) => {
    threadIdRef.current = id;
    setThreadId(id);
  };
  const [tokenLimits, setTokenLimits] = useState({context: 128_000, output: 4_096});
  const [tokenUsage, setTokenUsage] = useState(emptyTokenUsage);
  const seenUsageRequestIds = useRef(new Set<string>());
  const threadLoadRequest = useRef(0);
  const requestOwner = useRef(createRequestOwner());
  const [threadLoadError, setThreadLoadError] = useState('');
  const activeThread = threads.find(thread => thread.id === threadId) ?? null;
  const uncompactedMessages = activeThread?.messages.slice(activeThread.compacted_message_count ?? 0) ?? [];
  const estimatedContextTokens = uncompactedMessages.reduce((total, message) => total + estimateMessageTokens(message.text, message.attachments ?? []), 0)
    + estimateMessageTokens(activeThread?.compacted_summary ?? '');
  const contextPercent = Math.min(100, (estimatedContextTokens / Math.max(1, tokenLimits.context)) * 100);
  const nearContextLimit = contextPercent >= 85;
  const [compacting, setCompacting] = useState(false);
  const compactingRef = useRef(false);
  const compactedThreadsRef = useRef(new Set<string>());
  const [threadsCollapsed, setThreadsCollapsed] = useState(() => {
    try { return localStorage.getItem('openharness:threads-collapsed') === 'true'; } catch { return false; }
  });
  const [threadsWidth, setThreadsWidth] = useState(() => {
    try {
      const saved = Number(localStorage.getItem('openharness:threads-width'));
      return Number.isFinite(saved) && saved >= 160 && saved <= 320 ? saved : 206;
    } catch {
      return 206;
    }
  });
  const [projectId, setProjectId] = useState<string | null>(null);
  const [selectedSkillNames, setSelectedSkillNames] = useState<string[]>(() => {
    try {
      const names: unknown = JSON.parse(localStorage.getItem('openharness:global-skills') || '[]');
      return Array.isArray(names) ? names.filter((name): name is string => typeof name === 'string') : [];
    } catch { return []; }
  });
  const projectIdRef = useRef<string | null>(null);
  const projectSelectionMade = useRef(false);
  const updateProjectId = (id: string | null) => {
    projectIdRef.current = id;
    setProjectId(id);
  };
  const [projectName, setProjectName] = useState('');

  const toggleThreads = () => {
    setThreadsCollapsed(prev => {
      const next = !prev;
      try { localStorage.setItem('openharness:threads-collapsed', String(next)); } catch {}
      return next;
    });
  };
  const [stopHandle, setStopHandle] = useState<(() => void) | null>(null);
  const [runInProgress, setRunInProgress] = useState(false);
  const stopHandleRef = useRef(stopHandle);
  const activeRunRef = useRef(false);
  const runPromptRef = useRef<(detail: Omit<QueuedPrompt, 'id'>) => Promise<void>>(async () => {});
  const queueRef = useRef<(id: string) => void>(() => {});
  stopHandleRef.current = stopHandle;
  const [queuedPrompts, setQueuedPrompts] = useState<QueuedPrompt[]>(() => {
    try { return readQueuedPrompts(localStorage, 'openharness:queued-prompts'); } catch { return []; }
  });
  const queuedPromptsRef = useRef(queuedPrompts);
  const processingQueueRef = useRef(false);
  const updateQueuedPrompts = (update: (prompts: QueuedPrompt[]) => QueuedPrompt[]) => {
    const next = update(queuedPromptsRef.current);
    queuedPromptsRef.current = next;
    setQueuedPrompts(next);
    if (!persistQueuedPrompts(localStorage, 'openharness:queued-prompts', next)) {
      toast('Queue is too large to save. Remove a large attachment before it is lost.');
    }
  };
  const visibleQueuedPrompts = queuedPromptsForThread(queuedPrompts, threadId);
  const setQueueForThread = (ownerThreadId: string | null, update: (prompts: QueuedPrompt[]) => QueuedPrompt[]) => {
    updateQueuedPrompts(previous => {
      const owned = queuedPromptsForThread(previous, ownerThreadId);
      const otherThreads = previous.filter(prompt => prompt.threadId !== ownerThreadId);
      return [...otherThreads, ...update(owned)];
    });
  };
  const [approval, setApproval] = useState<ApprovalRequestLike | null>(null);
  const approvalDialogRef = useRef<HTMLDivElement>(null);
  const canvasLayoutRef = useRef<HTMLDivElement>(null);
  const approvalOpenerRef = useRef<HTMLElement | null>(null);
  const previousApprovalRef = useRef(false);
  const inertBackgroundRef = useRef<Array<[HTMLElement, boolean]>>([]);
  const [approvalBusy, setApprovalBusy] = useState(false);
  useEffect(() => {
    const opening = Boolean(approval);
    const layout = canvasLayoutRef.current;
    const backdrop = approvalDialogRef.current?.parentElement;
    if (opening && !previousApprovalRef.current) {
      approvalOpenerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
      inertBackgroundRef.current = layout
        ? Array.from(layout.children).filter((child): child is HTMLElement => child instanceof HTMLElement && child !== backdrop).map(element => [element, element.inert])
        : [];
      inertBackgroundRef.current.forEach(([element]) => { element.inert = true; });
    }
    if (opening) window.requestAnimationFrame(() => approvalDialogRef.current?.querySelector<HTMLButtonElement>('button:not([disabled])')?.focus());
    else if (previousApprovalRef.current) {
      inertBackgroundRef.current.forEach(([element, wasInert]) => { element.inert = wasInert; });
      inertBackgroundRef.current = [];
      const opener = approvalOpenerRef.current;
      if (opener?.isConnected) opener.focus();
      else document.querySelector<HTMLElement>('[aria-label="Approve once"]')?.focus();
      approvalOpenerRef.current = null;
    }
    previousApprovalRef.current = opening;
  }, [approval]);
  const handleApprovalKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    containTabKey(event.nativeEvent, event.currentTarget);
    if (event.key === 'Escape') event.preventDefault();
  };
  const [learningCandidates, setLearningCandidates] = useState<NonNullable<ChatThreadLike['learning_candidates']>>([]);
  const [learningBusy, setLearningBusy] = useState(false);
  const [workspaceStatus, setWorkspaceStatus] = useState('');
  const [gitDiff, setGitDiff] = useState('');
  const [activeTool, setActiveTool] = useState<'terminal'|'browser'|'files'|'git'|null>(null);
  const layoutElementRef = useRef<HTMLDivElement | null>(null);
  const [layoutWidth, setLayoutWidth] = useState(0);
  useEffect(() => {
    const element = layoutElementRef.current;
    if (!element) return;
    const observer = new ResizeObserver(entries => setLayoutWidth(entries[0]?.contentRect.width ?? 0));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const [terminalHeight, setTerminalHeight] = useState(() => {
    try { const saved = Number(localStorage.getItem('openharness:terminal-height')); return Number.isFinite(saved) && saved >= 140 && saved <= 600 ? saved : 240; }
    catch { return 240; }
  });
  const [filePanelWidth, setFilePanelWidth] = useState(() => {
    try { const saved = Number(localStorage.getItem('openharness:file-panel-width')); return Number.isFinite(saved) && saved >= 280 && saved <= 900 ? saved : 420; }
    catch { return 420; }
  });
  const [gitPanelWidth, setGitPanelWidth] = useState(() => {
    try { const saved = Number(localStorage.getItem('openharness:git-panel-width')); return Number.isFinite(saved) && saved >= 280 && saved <= 900 ? saved : 420; }
    catch { return 420; }
  });
   const [splitPanels, setSplitPanels] = useState(false);
   const [browserPanelWidth, setBrowserPanelWidth] = useState(() => {
     try { const saved = Number(localStorage.getItem('openharness:browser-panel-width')); return Number.isFinite(saved) && saved >= 340 && saved <= 1100 ? saved : 460; }
     catch { return 460; }
   });
  const [terminalTabs, setTerminalTabs] = useState<TerminalTab[]>([]);
  const terminalTabsRef = useRef<TerminalTab[]>([]);
  terminalTabsRef.current = terminalTabs;
  const [activeTerminalId, setActiveTerminalId] = useState<string | null>(null);
  const [terminalCommand, setTerminalCommand] = useState('');
  const [terminalBusy, setTerminalBusy] = useState(false);
  const terminalCreationPending = useRef(false);
  const nextTerminalNumber = useRef(1);
  const terminalOutputRef = useRef<HTMLPreElement | null>(null);
  const terminalScrollRef = useRef(new Map<string, {scrollTop: number; followLatest: boolean}>());
  const terminalScrollFrameRef = useRef<number | null>(null);
  const [terminalHasNewOutput, setTerminalHasNewOutput] = useState(false);
  const terminalInputRef = useRef<HTMLInputElement | null>(null);
  const scheduleTerminalFollow = () => {
    if (terminalScrollFrameRef.current !== null) return;
    terminalScrollFrameRef.current = window.requestAnimationFrame(() => {
      terminalScrollFrameRef.current = null;
      const element = terminalOutputRef.current;
      const id = activeTerminalId;
      if (!element || !id) return;
      const state = terminalScrollRef.current.get(id);
      if (state?.followLatest) element.scrollTop = element.scrollHeight;
      else setTerminalHasNewOutput(true);
    });
  };
  const onTerminalScroll = (element: HTMLPreElement) => {
    const id = activeTerminalId;
    if (!id) return;
    const followLatest = isNearBottom(element);
    terminalScrollRef.current.set(id, {scrollTop: element.scrollTop, followLatest});
    setTerminalHasNewOutput(!followLatest);
  };
  const jumpTerminalToLatest = () => {
    const element = terminalOutputRef.current;
    if (!element || !activeTerminalId) return;
    terminalScrollRef.current.set(activeTerminalId, {scrollTop: element.scrollHeight, followLatest: true});
    element.scrollTo({top: element.scrollHeight, behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth'});
    setTerminalHasNewOutput(false);
  };
  useEffect(() => () => {
    if (terminalScrollFrameRef.current !== null) window.cancelAnimationFrame(terminalScrollFrameRef.current);
  }, []);
  useEffect(() => {
    scheduleTerminalFollow();
  }, [terminalTabs, activeTerminalId, terminalHeight]);
  useLayoutEffect(() => {
    const element = terminalOutputRef.current;
    if (!element || !activeTerminalId) return;
    const state = terminalScrollRef.current.get(activeTerminalId) ?? {scrollTop: element.scrollHeight, followLatest: true};
    terminalScrollRef.current.set(activeTerminalId, state);
    element.scrollTop = state.followLatest ? element.scrollHeight : Math.min(state.scrollTop, element.scrollHeight);
    setTerminalHasNewOutput(!state.followLatest && element.scrollHeight > state.scrollTop + element.clientHeight);
  }, [activeTerminalId]);
  const conversationRef = useRef<HTMLElement | null>(null);
  const followConversationRef = useRef(true);
  const [showJumpToLatest, setShowJumpToLatest] = useState(false);

  useEffect(() => {
    try { localStorage.setItem('openharness:terminal-height', String(terminalHeight)); } catch {}
  }, [terminalHeight]);
  useEffect(() => {
    try { localStorage.setItem('openharness:file-panel-width', String(filePanelWidth)); } catch {}
  }, [filePanelWidth]);
  useEffect(() => {
    try { localStorage.setItem('openharness:git-panel-width', String(gitPanelWidth)); } catch {}
  }, [gitPanelWidth]);
  useEffect(() => { try { localStorage.setItem('openharness:browser-panel-width', String(browserPanelWidth)); } catch {} }, [browserPanelWidth]);
  useEffect(() => { try { localStorage.setItem('openharness:threads-width', String(threadsWidth)); } catch {} }, [threadsWidth]);



  useEffect(() => {
    const activePrompt = queuedPrompts.find(prompt => prompt.id.startsWith('active:'));
    const owner = activePrompt?.threadId ?? threadId;
    const owned = queuedPromptsForThread(queuedPrompts, owner);
    if (!owned.length || processingQueueRef.current || activeRunRef.current || stopHandleRef.current) return;
    const firstQueued = nextQueuedPrompt(owned);
    if (firstQueued) queueRef.current(firstQueued.id);
  }, [threadId, queuedPrompts]);

  const workspaceRequest = useRef(0);
  const refreshWorkspaceGit = async (currentProjectId: string | null) => {
    const request = ++workspaceRequest.current;
    if (!currentProjectId) {
      setWorkspaceStatus('');
      setGitDiff('');
      return;
    }
    try {
      const status = await apiGitStatus(currentProjectId);
      if (request === workspaceRequest.current && projectIdRef.current === currentProjectId) setWorkspaceStatus(status);
    } catch {
      if (request === workspaceRequest.current && projectIdRef.current === currentProjectId) setWorkspaceStatus('');
    }
    try {
      const diff = (await apiGitDiff(currentProjectId)).trim();
      if (request === workspaceRequest.current && projectIdRef.current === currentProjectId) setGitDiff(diff);
    } catch {
      if (request === workspaceRequest.current && projectIdRef.current === currentProjectId) setGitDiff('');
    }
  };

  const loadThreadMessages = (thread: ChatThreadLike) => {
    updateThreadId(thread.id);
    seenUsageRequestIds.current = new Set((thread.token_usage ?? []).map(record => record.requestId));
    setTokenUsage(tokenUsageTotals(thread.token_usage));
    setLearningCandidates(thread.learning_candidates ?? []);
    setMessages(thread.messages.map((message, index) => ({
      id: message.id || `${thread.id}:${index}`,
      role: message.role === 'assistant' ? 'assistant' : 'user',
      text: message.text,
      status: message.outcome === 'failed' || message.outcome === 'request_limit_exhausted' ? 'error' : message.outcome === 'cancelled' ? 'cancelled' : message.outcome === 'interrupted' ? 'interrupted' : 'done',
      attachments: message.attachments,
    })));
  };

  const threadListRequest = useRef(0);
  const refreshThreads = async (nextProjectId: string | null) => {
    const request = ++threadListRequest.current;
    try {
      const items = await apiListThreads(nextProjectId ?? undefined);
      if (request !== threadListRequest.current || projectIdRef.current !== nextProjectId) return [];
      setThreads(items);
      window.dispatchEvent(new CustomEvent('openharness:threads-updated', {detail: items}));
      return items;
    } catch {
      if (request !== threadListRequest.current || projectIdRef.current !== nextProjectId) return [];
      setThreads([]);
      window.dispatchEvent(new CustomEvent('openharness:threads-updated', {detail: []}));
      return [];
    }
  };

  useEffect(() => {
    let active = true;
    void apiGetProjects().then(items => {
      if (active && !projectSelectionMade.current && !projectIdRef.current && items.length) {
        const project = items.find(item => item.active) ?? items[0];
        const id = 'id' in project ? String(project.id) : '';
        if (id) {
          updateProjectId(id);
          window.dispatchEvent(new CustomEvent('openharness:active-project', {detail: {id}}));
          setProjectName('name' in project && typeof project.name === 'string' ? project.name : '');
          void refreshThreads(id);
        }
      }
    }).catch(() => {});
    return () => { active = false; };
  }, []);

  useEffect(() => {
    const applyThreadTitle = (event: Event) => {
      const detail = (event as CustomEvent<{ threadId?: string; title?: string }>).detail;
      if (!detail?.threadId || !detail.title) return;
      setThreads(previous => previous.map(thread => thread.id === detail.threadId ? {...thread, title: detail.title as string} : thread));
    };
    window.addEventListener('openharness:thread-title-updated', applyThreadTitle);
    return () => window.removeEventListener('openharness:thread-title-updated', applyThreadTitle);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const compactCurrentThread = async (currentThreadId: string, provider: string, modelId: string, contextLimit: number) => {
      if (compactingRef.current || compactedThreadsRef.current.has(currentThreadId)) return;
      compactingRef.current = true;
      setCompacting(true);
      try {
        const summary = await apiCompactThread(currentThreadId, modelId, provider);
        if (!summary) return;
        compactedThreadsRef.current.add(currentThreadId);
        const refreshed = await apiGetThread(currentThreadId);
        if (threadIdRef.current === currentThreadId) loadThreadMessages(refreshed);
        setThreads(previous => previous.map(thread => thread.id === currentThreadId ? refreshed : thread));
        toast(`Session compacted to stay within ${contextLimit.toLocaleString()} tokens.`);
      } catch (error) {
        toast(`Automatic compaction failed: ${String(error)}`);
      } finally {
        compactingRef.current = false;
        setCompacting(false);
      }
    };
    const runPrompt = async (detail: {text: string; modelId: string; provider: string; projectId?: string | null; threadId?: string | null; attachments?: string[]; supportsVision?: boolean; modelContextTokens?: number; modelOutputTokens?: number; selectedSkills?: string[]; approvalMode?: 'manual' | 'automatic'}) => {
      let persistedUserMessage: ChatMessageLike | null = null;
      if (detail.modelContextTokens) setTokenLimits({context: detail.modelContextTokens, output: detail.modelOutputTokens ?? 4_096});
      const text = String(detail?.text ?? '').trim();
      if (!text && !(detail.attachments?.length)) {
      setRunInProgress(false);
      processingQueueRef.current = false;
      activeRunRef.current = false;
      return;
    }
      setRunInProgress(true);
      if ((detail.attachments?.some(attachment => attachment.includes('data:image/')) ?? false) && !detail.supportsVision) {
        setRunInProgress(false);
        processingQueueRef.current = false;
        activeRunRef.current = false;
        toast('Selected model is not configured for image input.');
        return;
      }
      const providerInfo: ProviderInfo | undefined = (await apiListProviders()).find(item => item.provider === detail.provider);
      const activeLimits = {
        context: detail.modelContextTokens ?? providerInfo?.max_context_tokens ?? tokenLimits.context,
        output: detail.modelOutputTokens ?? providerInfo?.max_output_tokens ?? tokenLimits.output,
      };
      setTokenLimits(activeLimits);
      let currentThreadId = detail.threadId ?? threadIdRef.current;
      const selectedProjectAtStart = detail.projectId ?? projectIdRef.current;
      if (!detail.threadId && (threadId !== threadIdRef.current || projectId !== selectedProjectAtStart)) {
        setRunInProgress(false);
        processingQueueRef.current = false;
        activeRunRef.current = false;
        return;
      }
      const targetProjectId = Object.hasOwn(detail, 'projectId') ? detail.projectId ?? null : projectId;
      const activeSkills = detail.selectedSkills ?? selectedSkillNames;
      if (!currentThreadId && targetProjectId) {
        try {
          const thread = await apiCreateThread(targetProjectId);
          if (cancelled || threadIdRef.current !== currentThreadId || projectIdRef.current !== selectedProjectAtStart) {
            toast('Chat send cancelled because the selected project or thread changed.');
            return;
          }
          currentThreadId = thread.id;
          threadListRequest.current++;
          setThreads(prev => [thread, ...prev.filter(item => item.id !== thread.id)]);
          updateThreadId(thread.id);
          } catch (error) {
            setRunInProgress(false);
            processingQueueRef.current = false;
            activeRunRef.current = false;
            toast(`Could not create chat: ${String(error)}`);
            return;
          }
      }
      if (currentThreadId) compactedThreadsRef.current.delete(currentThreadId);
      const userMessage: Message = {id: `u-${Date.now()}`, role: 'user', text, status: 'queued', attachments: detail.attachments};
      followConversationRef.current = true;
      if (!detail.threadId) setMessages(prev => [...prev, userMessage]);
      const assistantId = `a-${Date.now()}`;
      if (detail.threadId) setMessages(previous => [...previous, {id: assistantId, role: 'assistant', text: '', status: 'thinking'}]);
      else setMessages(prev => [...prev, {id: assistantId, role: 'assistant', text: '', status: 'thinking'}]);
      const streamId = crypto.randomUUID();
      const cancel = () => {
        void apiCancelChatStream(streamId).catch(error => toast(`Could not stop: ${String(error)}`));
      };
      setStopHandle(() => cancel);
      try {
        persistedUserMessage = currentThreadId
          ? await apiAppendMessage(
              currentThreadId,
              'user',
              text,
              detail.attachments?.filter(attachment => attachment.startsWith('data:image/')),
            )
          : null;
        if (persistedUserMessage) setMessages(previous => previous.map(message => message.id === userMessage.id ? {...message, id: persistedUserMessage?.id ?? message.id} : message));
        await apiChatStream(text, detail.modelId, detail.provider, targetProjectId ?? undefined, currentThreadId ?? undefined, detail.attachments, detail.supportsVision, activeSkills, detail.approvalMode ?? 'manual', persistedUserMessage?.id, (token) => {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text: message.text + token, status: 'thinking'} : message));
        }, () => {
          setApproval(null);
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, status: 'done', steps: message.steps} : message.id === userMessage.id || message.id === persistedUserMessage?.id ? {...message, status: 'done' as const} : message));
          void refreshWorkspaceGit(targetProjectId);
        }, (error) => {
          const errText = String(error);
          if (errText.includes('Generation stopped')) {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, status: 'cancelled'} : message.id === userMessage.id || message.id === persistedUserMessage?.id ? {...message, status: 'cancelled'} : message));
          toast('Generation stopped');
          } else {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text: withFailureNotice(message.text, errText), status: 'error'} : message.id === userMessage.id || message.id === persistedUserMessage?.id ? {...message, status: 'error'} : message));
          toast(errText);
          }
        }, (approvalRequest) => setApproval(approvalRequest), step => {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, steps: upsertStep(message.steps ?? [], step), status: 'thinking'} : message));
        }, (usage: ProviderUsageLike, requestId) => {
          if (threadIdRef.current === currentThreadId && requestId) {
            setTokenUsage(previous => addUsageOnce(previous, seenUsageRequestIds.current, requestId, usage));
          }
        }, streamId);
      } catch (error) {
        const text = String(error);
        setApproval(null);
        if (text.includes('Generation stopped')) {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, status: 'cancelled'} : message.id === userMessage.id || message.id === persistedUserMessage?.id ? {...message, status: 'cancelled'} : message));
          toast('Generation stopped');
        } else {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text: withFailureNotice(message.text, text), status: 'error'} : message.id === userMessage.id || message.id === persistedUserMessage?.id ? {...message, status: 'error'} : message));
          toast(text);
        }
      } finally {
        setStopHandle(null);
        setRunInProgress(false);
        if (currentThreadId) {
          try {
            const thread = await apiGetThread(currentThreadId);
            if (threadIdRef.current === currentThreadId && projectIdRef.current === thread.project_id) loadThreadMessages(thread);
            if (projectIdRef.current === thread.project_id) setThreads(previous => previous.map(item => item.id === currentThreadId ? thread : item));
            const uncompactedMessages = thread.messages.slice(thread.compacted_message_count ?? 0);
            const projectedTokens = uncompactedMessages.reduce((total, message) => total + estimateMessageTokens(message.text, message.attachments ?? []), 0) + estimateMessageTokens(thread.compacted_summary ?? '');
            if (projectedTokens / Math.max(1, activeLimits.context) >= 0.7) {
              void compactCurrentThread(currentThreadId, detail.provider, detail.modelId, activeLimits.context);
            }
          } catch {}
          void refreshThreads(targetProjectId);
        }
      }
    };
    const drainQueue = async (queueId: string) => {
      if (processingQueueRef.current || activeRunRef.current || stopHandleRef.current) return;
      const requested = queuedPromptById(queuedPromptsRef.current, queueId);
      const activePrompt = queuedPromptsRef.current.find(prompt => prompt.id.startsWith('active:'));
      const owner = activePrompt?.threadId ?? requested?.threadId ?? threadIdRef.current;
      const next = nextQueuedPrompt(queuedPromptsRef.current.filter(prompt => prompt.threadId === owner));
      if (!next) return;
      processingQueueRef.current = true;
      activeRunRef.current = true;
      try {
        if (next.threadId && next.threadId !== threadIdRef.current) {
          const thread = await apiGetThread(next.threadId).catch(() => null);
          if (!thread) {
            toast('Queued prompt skipped: its thread no longer exists.');
            setQueueForThread(next.threadId ?? null, prompts => removeQueuedPrompt(prompts, next.id));
            activeRunRef.current = false;
            processingQueueRef.current = false;
            return;
          }
          loadThreadMessages(thread);
          updateThreadId(next.threadId);
        }
        setRunInProgress(true);
        setQueueForThread(next.threadId ?? null, prompts => [markPromptActive(next), ...removeQueuedPrompt(prompts, next.id)]);
        await runPromptRef.current(next);
        setRunInProgress(false);
        setQueueForThread(next.threadId ?? null, prompts => removeQueuedPrompt(prompts, next.id));
        activeRunRef.current = false;
        processingQueueRef.current = false;
        setRunInProgress(false);
        const following = nextQueuedPrompt(queuedPromptsRef.current.filter(prompt => prompt.threadId === owner && !prompt.id.startsWith('active:')));
        if (following) queueRef.current(following.id);
      } catch (error) {
        toast(`Queue paused: ${String(error)}`);
        activeRunRef.current = false;
        processingQueueRef.current = false;
        setRunInProgress(false);
      }
    };
    queueRef.current = id => { void drainQueue(id); };
    runPromptRef.current = runPrompt;
    const resumeQueue = window.setTimeout(() => {
      const activePrompt = queuedPromptsRef.current.find(prompt => prompt.id.startsWith('active:'));
      const owner = activePrompt?.threadId ?? threadIdRef.current;
      const firstQueued = nextQueuedPrompt(queuedPromptsRef.current.filter(prompt => prompt.threadId === owner));
      if (firstQueued) queueRef.current(firstQueued.id);
    }, 0);
    const onSend = (event: Event) => {
      const detail = (event as CustomEvent<Omit<QueuedPrompt, 'id' | 'threadId'>>).detail;
      const queued = {...detail, projectId: detail.projectId ?? projectIdRef.current, threadId: threadIdRef.current};
      const item = {id: `${Date.now()}-${Math.random().toString(36).slice(2)}`, ...queued};
      const waitingBehindActive = activeRunRef.current || processingQueueRef.current;
      const ownedQueue = queuedPromptsForThread(queuedPromptsRef.current, queued.threadId ?? null);
      if (waitingBehindActive || ownedQueue.length > 0) {
        setQueueForThread(queued.threadId ?? null, prompts => [...prompts, item]);
        toast('Prompt queued for after the current run.');
        return;
      }
      activeRunRef.current = true;
      setQueueForThread(queued.threadId ?? null, prompts => [item, ...prompts]);
      void runPrompt(queued).finally(() => {
        activeRunRef.current = false;
        setQueueForThread(queued.threadId ?? null, prompts => removeQueuedPrompt(prompts, item.id));
      const following = nextQueuedPrompt(queuedPromptsRef.current.filter(prompt => prompt.threadId === (queued.threadId ?? null)));
      if (following) queueRef.current(following.id);
      });
    };
    const onApprovalModeChanged = (event: Event) => {
      const mode = (event as CustomEvent<string>).detail;
      if (mode === 'manual' || mode === 'automatic') {
        window.dispatchEvent(new CustomEvent('openharness:skills-changed', {detail: {names: selectedSkillNames, approvalMode: mode}}));
      }
    };
    const onLearningReady = (event: Event) => {
      const thread = (event as CustomEvent<ChatThreadLike>).detail;
      if (thread?.id && threadIdRef.current === thread.id) setLearningCandidates(thread.learning_candidates ?? []);
    };
    const onSkillsChanged = (event: Event) => {
      const detail = (event as CustomEvent<{names?: string[]}>).detail;
      setSelectedSkillNames(Array.isArray(detail?.names) ? detail.names.filter((name): name is string => typeof name === 'string') : []);
    };
    const startNewThread = () => {
        threadLoadRequest.current += 1;
        requestOwner.current.invalidate();
        setThreadLoadError('');
        seenUsageRequestIds.current.clear();
        setMessages([]);
        setTokenUsage(emptyTokenUsage());
        setLearningCandidates([]);
        setApproval(null);
        updateThreadId(null);
        window.dispatchEvent(new CustomEvent('openharness:thread-title-updated', {detail: {title: 'OpenHarness'}}));
    };
    const onNewChat = startNewThread;
    const onOpenThread = async (selectedThreadId: string) => {
      const request = ++threadLoadRequest.current;
      const owner = requestOwner.current.begin(projectIdRef.current, selectedThreadId);
      updateThreadId(selectedThreadId);
      setMessages([]);
      setTokenUsage(emptyTokenUsage());
      seenUsageRequestIds.current.clear();
      setLearningCandidates([]);
      setApproval(null);
      setThreadLoadError('');
      try {
        const thread = await apiGetThread(selectedThreadId);
        if (!isCurrentThreadLoad(threadLoadRequest.current, request)
          || !requestOwner.current.isCurrent(owner, projectIdRef.current, selectedThreadId)
          || thread.id !== selectedThreadId || thread.project_id !== projectIdRef.current) return;
        loadThreadMessages(thread);
      } catch (error) {
        if (!cancelled && isCurrentThreadLoad(threadLoadRequest.current, request)
          && requestOwner.current.isCurrent(owner, projectIdRef.current, selectedThreadId)) {
          setThreadLoadError(String(error));
        }
      }
    };
    const onSelectProject = async (event: Event) => {
      threadLoadRequest.current += 1;
      requestOwner.current.invalidate();
      threadListRequest.current++;
      seenUsageRequestIds.current.clear();
      projectSelectionMade.current = true;
      const project = (event as CustomEvent<{id?: string; name?: string; title?: string}>).detail;
      const nextProjectId = project?.id ?? null;
      updateProjectId(nextProjectId);
      updateThreadId(null);
      setThreadLoadError('');
      setMessages([]);
      setTokenUsage(emptyTokenUsage());
      setLearningCandidates([]);
      setApproval(null);
      setThreads([]);
      try {
        const names: unknown = JSON.parse(localStorage.getItem('openharness:global-skills') || '[]');
        setSelectedSkillNames(Array.isArray(names) ? names.filter((name): name is string => typeof name === 'string') : []);
      } catch { setSelectedSkillNames([]); }
          setProjectName(project?.name ?? project?.title ?? '');
          window.dispatchEvent(new CustomEvent('openharness:active-project', {detail: {id: nextProjectId, name: project?.name ?? project?.title ?? ''}}));
      updateThreadId(null);
        setMessages([]);
       setTokenUsage(emptyTokenUsage());
       void refreshWorkspaceGit(nextProjectId);
      await refreshThreads(nextProjectId);
    };
    const onDeleteThread = async (selectedThreadId: string) => {
      if (!window.confirm('Delete this chat?')) return;
      const projectAtStart = projectIdRef.current;
      const request = ++threadLoadRequest.current;
      threadListRequest.current++;
      try {
            const remaining = await apiDeleteThread(selectedThreadId);
            if (request !== threadLoadRequest.current || projectIdRef.current !== projectAtStart) return;
            updateQueuedPrompts(previous => previous.filter(prompt => prompt.threadId !== selectedThreadId));

        const filtered = projectAtStart ? remaining.filter(t => t.project_id === projectAtStart) : remaining;
        setThreads(filtered);
                if (threadIdRef.current === selectedThreadId) {
            threadLoadRequest.current += 1;
            requestOwner.current.invalidate();
            setThreadLoadError('');
            updateThreadId(null);
            seenUsageRequestIds.current.clear();
            setMessages([]);
            setTokenUsage(emptyTokenUsage());
            if (filtered.length) loadThreadMessages(filtered[0]);
          }
      } catch (error) { toast(String(error)); }
    };
    const handleOpenThread = (event: Event) => void onOpenThread(String((event as CustomEvent<string>).detail));
    const handleDeleteThread = (event: Event) => void onDeleteThread(String((event as CustomEvent<string>).detail));
    const handleQuickCmd = () => {
      if (terminalCreationPending.current) return;
      const hasSessions = terminalTabsRef.current.length > 0;
      setActiveTool(current => current === 'terminal' && hasSessions ? null : 'terminal');
      if (!hasSessions) void createTerminal();
    };
    const handleQuickFiles = () => setActiveTool(current => current === 'files' ? null : 'files');
    const handleQuickGit = () => {
      setActiveTool(current => current === 'git' ? null : 'git');
      void refreshGitContext();
    };
    const handleQuickBrowser = () => setActiveTool(current => current === 'browser' ? null : 'browser');
    window.addEventListener('openharness:quick-cmd', handleQuickCmd);
    window.addEventListener('openharness:quick-files', handleQuickFiles);
    window.addEventListener('openharness:quick-git', handleQuickGit);
    window.addEventListener('openharness:quick-browser', handleQuickBrowser);
    window.addEventListener('openharness:learning-ready', onLearningReady);
    window.addEventListener('openharness:approval-mode-changed', onApprovalModeChanged);
    window.addEventListener('openharness:send-message', onSend as EventListener);
    window.addEventListener('openharness:new-chat', onNewChat);
    window.addEventListener('openharness:skills-changed', onSkillsChanged);
    window.addEventListener('openharness:select-project', onSelectProject as EventListener);
    window.addEventListener('openharness:open-thread', handleOpenThread);
    window.addEventListener('openharness:delete-thread', handleDeleteThread);
    return () => { cancelled = true; window.clearTimeout(resumeQueue); window.removeEventListener('openharness:learning-ready', onLearningReady); window.removeEventListener('openharness:approval-mode-changed', onApprovalModeChanged); window.removeEventListener('openharness:send-message', onSend as EventListener); window.removeEventListener('openharness:new-chat', onNewChat); window.removeEventListener('openharness:skills-changed', onSkillsChanged); window.removeEventListener('openharness:select-project', onSelectProject as EventListener); window.removeEventListener('openharness:open-thread', handleOpenThread); window.removeEventListener('openharness:delete-thread', handleDeleteThread); window.removeEventListener('openharness:quick-cmd', handleQuickCmd); window.removeEventListener('openharness:quick-files', handleQuickFiles); window.removeEventListener('openharness:quick-git', handleQuickGit); window.removeEventListener('openharness:quick-browser', handleQuickBrowser); };
  }, [threadId, projectId]);

  useEffect(() => {
    followConversationRef.current = true;
    setShowJumpToLatest(false);
    const element = conversationRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [threadId]);

  useLayoutEffect(() => {
    const element = conversationRef.current;
    if (!element) return;
    const updatePosition = () => {
      const nearBottom = isNearBottom(element);
      followConversationRef.current = nearBottom;
      setShowJumpToLatest(!nearBottom);
    };
    const followLatest = () => {
      if (followConversationRef.current) {
        element.scrollTop = element.scrollHeight;
        setShowJumpToLatest(false);
      } else {
        setShowJumpToLatest(true);
      }
    };
    element.addEventListener('scroll', updatePosition, {passive: true});
    followLatest();
    const observer = new ResizeObserver(followLatest);
    observer.observe(element);
    for (const child of element.children) observer.observe(child);
    return () => {
      element.removeEventListener('scroll', updatePosition);
      observer.disconnect();
    };
  }, [messages]);

  const jumpToLatest = () => {
    const element = conversationRef.current;
    if (!element) return;
    followConversationRef.current = true;
    element.scrollTo({top: element.scrollHeight, behavior: window.matchMedia('(prefers-reduced-motion: reduce)').matches ? 'instant' : 'smooth'});
    setShowJumpToLatest(false);
  };

  useEffect(() => {
    let unlistenOutput: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    let active = true;
    void apiListenTerminalOutput((id, data) => {
      if (active) setTerminalTabs(previous => previous.map(tab => tab.id === id ? {...tab, output: (tab.output + data).slice(-200_000)} : tab));
    }).then(unlisten => { if (active) unlistenOutput = unlisten; else unlisten(); });
    void apiListenTerminalExit((id, code) => {
      if (active) setTerminalTabs(previous => previous.map(tab => tab.id === id ? {...tab, status: 'exited', output: `${tab.output}\n[Process exited: ${code ?? 'unknown'}]\n`} : tab));
    }).then(unlisten => { if (active) unlistenExit = unlisten; else unlisten(); });
    return () => { active = false; unlistenOutput?.(); unlistenExit?.(); };
  }, []);

  useEffect(() => {
    if (terminalOutputRef.current) terminalOutputRef.current.scrollTop = terminalOutputRef.current.scrollHeight;
  }, [terminalTabs, activeTerminalId]);

  const resizeTerminal = (event: React.PointerEvent<HTMLDivElement>) => {
    const startY = event.clientY;
    const startHeight = terminalHeight;
    const move = (pointer: PointerEvent) => setTerminalHeight(Math.min(600, Math.max(140, startHeight + startY - pointer.clientY)));
    const stop = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      window.removeEventListener('pointercancel', stop);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop, {once: true});
    window.addEventListener('pointercancel', stop, {once: true});
    event.currentTarget.setPointerCapture(event.pointerId);
  };

  const resizeDockedPanel = (event: React.PointerEvent<HTMLDivElement>, currentWidth: number, setWidth: (width: number) => void) => {
    const startX = event.clientX;
    const startWidth = currentWidth;
    const captured = event.currentTarget;

    const move = (pointer: PointerEvent) => {
      const width = Math.min(Math.max(280, startWidth + startX - pointer.clientX), Math.min(900, window.innerWidth - 320));
      setWidth(width);
    };

    const stop = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      window.removeEventListener('pointercancel', stop);
      window.removeEventListener('lostpointercapture', stop);
      if (captured && captured.hasPointerCapture?.(event.pointerId)) {
        try { captured.releasePointerCapture(event.pointerId); } catch {}
      }
    };

    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop, { once: true });
    window.addEventListener('pointercancel', stop, { once: true });
    window.addEventListener('lostpointercapture', stop, { once: true });
    captured.setPointerCapture(event.pointerId);
  };

  const resizeThreadSidebar = (event: React.PointerEvent<HTMLDivElement>) => {
    if (threadsCollapsed) return;
    const startX = event.clientX;
    const startWidth = threadsWidth;
    const captured = event.currentTarget;

    const move = (pointer: PointerEvent) => {
      const width = Math.min(320, Math.max(160, startWidth + pointer.clientX - startX));
      setThreadsWidth(width);
    };

    const stop = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      window.removeEventListener('pointercancel', stop);
      window.removeEventListener('lostpointercapture', stop);
      if (captured && captured.hasPointerCapture?.(event.pointerId)) {
        try { captured.releasePointerCapture(event.pointerId); } catch {}
      }
    };

    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop, { once: true });
    window.addEventListener('pointercancel', stop, { once: true });
    window.addEventListener('lostpointercapture', stop, { once: true });
    captured.setPointerCapture(event.pointerId);
  };

  const handleThreadResizeKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      setThreadsWidth(width => Math.min(320, width + 10));
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      setThreadsWidth(width => Math.max(160, width - 10));
    } else if (event.key === 'Home') {
      event.preventDefault();
      setThreadsWidth(160);
    } else if (event.key === 'End') {
      event.preventDefault();
      setThreadsWidth(320);
    }
  };

  const closeTerminal = async (id: string) => {
    try { await apiTerminalKill(id); }
    catch (error) { toast(`Terminal close failed: ${String(error)}`); }
    setTerminalTabs(previous => {
      const remaining = previous.filter(tab => tab.id !== id);
      setActiveTerminalId(current => current === id ? (remaining[0]?.id ?? null) : current);
      return remaining;
    });
  };

  const defaultTerminalShell: TerminalShell = /Win/i.test(navigator.platform) ? 'powershell' : /Mac/i.test(navigator.platform) ? 'zsh' : 'bash';
  const createTerminal = async (shell: TerminalShell = defaultTerminalShell) => {
      if (!isTauri()) {
      toast('Terminal requires the desktop app');
      return;
    }
    if (terminalCreationPending.current) return;
    terminalCreationPending.current = true;
    setTerminalBusy(true);
    try {
      const id = await apiTerminalCreate(projectId, shell);
      const nextIndex = nextTerminalNumber.current++;
      const label = nextIndex === 1 ? 'Terminal' : `Terminal ${nextIndex}`;
      const initialOutput = shell === 'powershell' ? 'PowerShell ready.\r\n' : '';
      terminalScrollRef.current.set(id, {scrollTop: 0, followLatest: true});
      setTerminalTabs(previous => [...previous, {id, label, shell, output: initialOutput, status: 'running'}]);
      setActiveTerminalId(id);
      setActiveTool('terminal');
    } catch (error) {
      toast(`Terminal failed to start: ${String(error)}`);
    } finally {
      terminalCreationPending.current = false;
      setTerminalBusy(false);
    }
  };

  const sendTerminalInput = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!activeTerminalId || !terminalCommand.trim()) return;
    const command = terminalCommand;
    setTerminalCommand('');
    try { await apiTerminalWrite(activeTerminalId, `${command}\r\n`); }
    catch (error) { toast(`Terminal input failed: ${String(error)}`); }
  };

  const reviewLearning = async (decision: 'approve' | 'dismiss') => {
    if (!threadId || !projectId || learningBusy || !learningCandidates.length) return;
    setLearningBusy(true);
    try {
      const updated = decision === 'approve'
        ? await apiUpdateThreadLearning(projectId, threadId, learningCandidates)
        : await apiDismissThreadLearning(threadId);
      setLearningCandidates(updated.learning_candidates ?? []);
      await refreshThreads(projectId);
      toast(decision === 'approve' ? 'Approved project knowledge saved in project scope.' : 'Project learning proposal dismissed.');
    } catch (error) {
      toast(`Could not ${decision} project learning: ${String(error)}`);
    } finally {
      setLearningBusy(false);
    }
  };

  const approveToolCall = async (allowed: boolean) => {
    if (!approval || approvalBusy) return;
    setApprovalBusy(true);
    try {
      await apiRespondToToolApproval(approval.id, allowed);
      setApproval(null);
    } catch (error) {
      toast(`Approval failed: ${String(error)}`);
    } finally {
      setApprovalBusy(false);
    }
  };

  const refreshGitContext = async () => {
    if (!projectId) { setWorkspaceStatus('Select a project to view Git status.'); setGitDiff(''); return; }
    try {
      const [status, diff] = await Promise.all([apiGitStatus(projectId), apiGitDiff(projectId)]);
      setWorkspaceStatus(status);
      setGitDiff(diff.trim());
      window.dispatchEvent(new CustomEvent('openharness:workspace-context', {detail: {status, diff}}));
    } catch (error) {
      setGitDiff(`Git diff failed: ${String(error)}`);
    }
  };

  const stopGenerating = () => stopHandle?.();

  const copyMessage = async (message: Message) => {
    try { await navigator.clipboard.writeText(message.text); toast('Copied'); }
    catch { toast('Copy failed'); }
  };

  const deleteMessage = async (id: string) => {
    if (!threadId) { setMessages(prev => prev.filter(message => message.id !== id)); return; }
    try {
      const remaining = await apiDeleteMessage(threadId, id);
      setMessages(remaining.map(message => ({...message, role: message.role === 'assistant' ? 'assistant' : 'user', status: 'done'})));
      toast('Message deleted');
    } catch (error) { toast(`Delete failed: ${String(error)}`); }
  };

  const saveMessageEdit = async (id: string, text: string) => {
    if (!threadId) { setMessages(prev => prev.map(message => message.id === id ? {...message, text} : message)); return; }
    try {
      const updated = await apiEditMessage(threadId, id, text);
      setMessages(prev => prev.map(message => message.id === id ? {...message, text: updated.text} : message));
      toast('Edit saved');
    } catch (error) { toast(`Edit failed: ${String(error)}`); }
  };

  const regenerateMessage = (id: string) => {
    const idx = messages.findIndex(message => message.id === id);
    if (idx < 0) return;
    const promptMsg = [...messages.slice(0, idx + 1)].reverse().find(message => message.role === 'user');
    if (!promptMsg) return;
    toast('Regeneration will resend the prompt');
    window.dispatchEvent(new CustomEvent('openharness:set-prompt', {detail: promptMsg.text}));
  };

  const empty = messages.length === 0;
  const isWorkspacePanel = activeTool === 'files' || activeTool === 'git';
  const availableCanvasWidth = Math.max(0, layoutWidth - (threadsCollapsed ? 48 : threadsWidth));
  const splitLayout = getSplitLayout(availableCanvasWidth, splitPanels && isWorkspacePanel, filePanelWidth, gitPanelWidth);
  const isSideBySide = splitLayout.enabled;
  const activeFileWidth = isSideBySide ? splitLayout.filesWidth : filePanelWidth;
  const activeGitWidth = isSideBySide ? splitLayout.gitWidth : gitPanelWidth;
  const sidePanelWidth = isSideBySide
    ? splitLayout.sidePanelWidth
    : activeTool === 'files' ? filePanelWidth : activeTool === 'git' ? gitPanelWidth : activeTool === 'browser' ? browserPanelWidth : 0;
  const rightOffset = sidePanelWidth > 0 ? sidePanelWidth + CHAT_PANEL_GAP : 0;

  return (
      <div className="canvas-layout" ref={element => { layoutElementRef.current = element; canvasLayoutRef.current = element; }}>
      {threadsCollapsed ? (
        <aside className="thread-sidebar thread-sidebar-collapsed" aria-label="Chat threads collapsed" style={{width: 48, minWidth: 48, flexBasis: 48}}>
          <button type="button" className="collapse-toggle-btn thread-toggle" onClick={toggleThreads} aria-label="Expand threads sidebar" title="Expand threads sidebar">
            <ChevronRightIcon width={14} height={14} />
          </button>
          <button type="button" className="thread-collapsed-action" onClick={() => window.dispatchEvent(new Event('openharness:new-chat'))} aria-label="New thread" title="New thread"><PlusIcon width={16} height={16} aria-hidden="true" /></button>
          <div className="thread-collapsed-list">
            {threads.slice(0, 6).map(thread => (
              <button
                key={thread.id}
                type="button"
                className={`thread-collapsed-item ${thread.id === threadId ? 'active' : ''}`}
                title={thread.title || 'Chat thread'}
                aria-label={thread.title || 'Chat thread'}
                onClick={() => window.dispatchEvent(new CustomEvent('openharness:open-thread', {detail: thread.id}))}
              >
                <ChatBubbleLeftIcon width={15} height={15} aria-hidden="true" />
              </button>
            ))}
          </div>
        </aside>
      ) : (
        <aside className="thread-sidebar" aria-label="Chat threads" style={{width: threadsWidth, minWidth: 160, flexBasis: threadsWidth}}>
          <div
            className="thread-sidebar-resizer"
            onPointerDown={resizeThreadSidebar}
            onKeyDown={handleThreadResizeKeyDown}
            tabIndex={0}
            role="separator"
            aria-orientation="vertical"
            aria-valuenow={threadsWidth}
            aria-valuemin={160}
            aria-valuemax={320}
            aria-label="Resize threads sidebar"
          />
          <div className="thread-sidebar-header">
            <div className="thread-sidebar-title-row">
              <span className="thread-sidebar-title">Threads</span>
              <button type="button" className="collapse-toggle-btn thread-toggle" onClick={toggleThreads} aria-label="Collapse threads sidebar" title="Collapse threads sidebar">
                <ChevronLeftIcon width={14} height={14} />
              </button>
            </div>
            <button type="button" className="new-thread-btn" onClick={() => window.dispatchEvent(new Event('openharness:new-chat'))} aria-label="New thread">+ New thread</button>
          </div>
          <ul className="thread-list">
              {threads.map(thread => (
              <li key={thread.id} className="thread-row">
                <button type="button" className={`thread-item ${thread.id === threadId ? 'thread-item-active' : ''}`} aria-current={thread.id === threadId ? 'page' : undefined} onClick={() => window.dispatchEvent(new CustomEvent('openharness:open-thread', {detail: thread.id}))}>{thread.title || 'New chat'}</button>
                <button type="button" className="thread-delete" aria-label={`Delete ${thread.title || 'chat'}`} onClick={() => window.dispatchEvent(new CustomEvent('openharness:delete-thread', {detail: thread.id}))}>×</button>
              </li>
            ))}
          </ul>
          {!threads.length && <p className="thread-empty">No chats saved yet.</p>}
          <section className="session-context-card" aria-label="Session token context" aria-live="polite">
            <div className="session-context-heading"><span className="session-context-eyebrow">TOKEN CONTEXT</span><span className="session-context-model">{(tokenLimits.context / 1000).toFixed(0)}K WINDOW</span></div>
            <div className="session-context-values">
              <div><strong>{estimatedContextTokens.toLocaleString()}</strong><span>estimated in context</span></div>
              <div><strong>{tokenUsage.input?.toLocaleString() ?? '—'}</strong><span>provider input</span></div>
              <div><strong>{tokenUsage.output?.toLocaleString() ?? '—'}</strong><span>provider output</span></div>
              <div><strong>{tokenUsage.cached?.toLocaleString() ?? '—'}</strong><span>cached input</span></div>
              <div><strong>{tokenUsage.reasoning?.toLocaleString() ?? '—'}</strong><span>reasoning</span></div>
              <div><strong>{tokenUsage.total?.toLocaleString() ?? '—'}</strong><span>provider total</span></div>
            </div>
            <div className={`session-context-meter${nearContextLimit ? ' near-limit' : ''}`} role="progressbar" aria-label="Estimated context usage" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(contextPercent)}>
              <span style={{width: `${contextPercent}%`}} />
            </div>
            <div className="session-context-meta"><span>{contextPercent.toFixed(1)}% estimated</span><span>{Math.max(0, tokenLimits.context - estimatedContextTokens).toLocaleString()} available</span></div>
            {compacting && <span className="session-context-detail">Compacting session…</span>}
            <span className="session-context-detail">{activeThread?.title || (projectId ? 'New thread' : 'Select a project to show context.')}</span>
          </section>
        </aside>
      )}
      <main
        className={`canvas ${activeTool === 'terminal' ? 'canvas-terminal-open' : ''}`}
        style={
          {
            '--terminal-height': `${terminalHeight}px`,
          marginRight: rightOffset > 0 ? `${rightOffset}px` : undefined,
          } as React.CSSProperties
        }
      >
        {workspaceStatus && !workspaceStatus.includes('\0') && (
          <div className="git-status-badge" aria-label="Git repository status" title={workspaceStatus}>
            <span>{workspaceStatus.split('\n')[0]}</span>
          </div>
        )}
        {/* browser rendered as docked right-side panel below */}

        {threadLoadError && <div className="provider-error" role="alert">Could not load conversation: {threadLoadError}</div>}
        {empty && <Hero className={activeTool === 'terminal' ? 'hero-above-terminal' : undefined} />}
        <section className="conversation" aria-live="polite" ref={conversationRef} hidden={empty}>
          {messages.map(message => (
            <div className="conversation-entry" key={message.id}>
              <MessageView message={message} onCopy={() => copyMessage(message)} onDelete={() => deleteMessage(message.id)} onRegenerate={() => regenerateMessage(message.id)} onSaveEdit={text => void saveMessageEdit(message.id, text)} />
            </div>
          ))}
        </section>
        {showJumpToLatest && !empty && <button type="button" className="jump-to-latest" onClick={jumpToLatest}>Newer messages · Jump to latest</button>}
        {learningCandidates.length > 0 && <LearningProposalCard candidate={learningCandidates[0]} saving={learningBusy} onApprove={() => void reviewLearning('approve')} onDismiss={() => void reviewLearning('dismiss')} />}
        {visibleQueuedPrompts.length > 0 && (
          <section className="prompt-queue" aria-label="Queued prompts">
            <header><strong>Queued prompts ({visibleQueuedPrompts.length})</strong><span>Run in order after the active task</span></header>
            <ol>{visibleQueuedPrompts.map((item, index) => <li key={item.id}>
              <span className="prompt-queue-index">{item.id.startsWith('active:') ? 'Now' : index + 1}</span>
              <span className="prompt-queue-text">{item.text || 'Image or file attachment'}</span>
              {item.attachments.length > 0 && <span className="prompt-queue-attachments">{item.attachments.length} attachment{item.attachments.length === 1 ? '' : 's'}</span>}
              <button type="button" aria-label={`Remove queued prompt ${index + 1}`} onClick={() => updateQueuedPrompts(previous => removeQueuedPrompt(previous, item.id))}>Remove</button>
            </li>)}</ol>
          </section>
        )}
        <div className="composer-wrap">
          <Composer running={runInProgress} onStop={stopGenerating} />
        </div>
        {approval && (
          <div className="approval-backdrop" role="presentation">
            <div ref={approvalDialogRef} className="approval-dialog" role="alertdialog" aria-modal="true" aria-labelledby="tool-approval-title" aria-describedby="tool-approval-description" tabIndex={-1} onKeyDown={handleApprovalKeyDown}>
              <h2 id="tool-approval-title">Approve workspace operation</h2>
              <p id="tool-approval-description"><strong>{approval.tool}</strong> requests:</p>
              <pre>{JSON.stringify(approval.arguments, null, 2)}</pre>
              <div className="approval-actions">
                <button type="button" onClick={() => void approveToolCall(false)} disabled={approvalBusy}>Reject</button>
                <button type="button" className="confirm" onClick={() => void approveToolCall(true)} disabled={approvalBusy}>Approve once</button>
              </div>
            </div>
          </div>
        )}
        {activeTool === 'terminal' && (
          <section className="terminal-panel" aria-label="Terminal bottom panel" style={{height: `${terminalHeight}px`, right: '18px'}}>
            <div className="terminal-resizer" onPointerDown={resizeTerminal} role="separator" aria-orientation="horizontal" aria-label="Resize terminal panel" />
            <div className="terminal-header">
              <div className="terminal-tabs" role="tablist" aria-label="Terminal sessions">
                    {terminalTabs.map(tab => (
                  <button key={tab.id} type="button" role="tab" aria-selected={tab.id === activeTerminalId} className={tab.id === activeTerminalId ? 'terminal-tab active' : 'terminal-tab'} onClick={() => setActiveTerminalId(tab.id)}>
                    {tab.label} {tab.status === 'exited' ? '· exited' : ''}
                  </button>
                ))}
                <button type="button" className="terminal-tab terminal-new" onClick={() => void createTerminal()} disabled={terminalBusy}>+ Terminal</button>
              </div>
              <button type="button" className="workspace-tool-close" aria-label="Close terminal panel" onClick={() => setActiveTool(null)}>×</button>
            </div>
            <pre className="terminal-output" ref={terminalOutputRef} onScroll={event => onTerminalScroll(event.currentTarget)} tabIndex={0} aria-label="Terminal output">{terminalTabs.find(tab => tab.id === activeTerminalId)?.output || 'No terminal sessions. Use + Terminal.'}</pre>
            {terminalHasNewOutput && <button type="button" className="terminal-jump-latest" onClick={jumpTerminalToLatest}>New terminal output · Jump to latest</button>}
            {(() => {
              const active = terminalTabs.find(tab => tab.id === activeTerminalId);
              if (active?.status === 'running') {
                return (
                  <form className="terminal-input-row" onSubmit={sendTerminalInput}>
                    <input ref={terminalInputRef} value={terminalCommand} onChange={e => setTerminalCommand(e.target.value)} aria-label="Terminal command" placeholder="Terminal" autoComplete="off" spellCheck={false} />
                    <button type="submit" aria-label="Send terminal command">Send</button>
                    <button type="button" aria-label="Close active terminal" onClick={() => void closeTerminal(active.id)}>Close</button>
                  </form>
                );
              }
              return active ? (
                <div className="terminal-input-row">
                  <span>Session ended.</span>
                  <button type="button" onClick={() => void closeTerminal(active.id)}>Close</button>
                </div>
              ) : null;
            })()}
          </section>
        )}
        </main>
      {(activeTool === 'files' || activeTool === 'git') && (
        <div className={`docked-panels-layer ${isSideBySide ? 'docked-panels-split' : ''}`} style={isSideBySide ? {maxWidth: `${splitLayout.sidePanelWidth}px`} : undefined}>
          {(activeTool === 'files' || isSideBySide) && (
            <aside className="file-panel" aria-label="Files panel" style={{width: `${activeFileWidth}px`}}>
                <div className="file-panel-header">
                  <div className="file-panel-tabs" role="tablist" aria-label="Workspace panels">
                    <button type="button" role="tab" aria-selected={activeTool === 'files'} className={`file-panel-tab ${activeTool === 'files' ? 'active' : ''}`} onClick={() => setActiveTool('files')}>
                      <DocumentTextIcon width={14} height={14} />Files
                    </button>
                    <button type="button" role="tab" aria-selected={activeTool === 'git'} className={`file-panel-tab ${activeTool === 'git' ? 'active' : ''}`} onClick={() => setActiveTool('git')}>
                      <GitDiffIcon width={14} height={14} />Git
                    </button>
                  </div>
                  {projectName && <span className="file-panel-project-tag">{projectName}</span>}
                  <div className="file-panel-header-controls">
                    <button type="button" className={`file-panel-split-toggle ${splitPanels ? 'active' : ''}`} aria-label={splitPanels ? 'Show one workspace panel' : 'Show files and Git side by side'} title={splitPanels ? 'Show one panel' : 'Split panels'} onClick={() => setSplitPanels(value => !value)}>
                      <SquaresPlusIcon width={15} height={15} />
                    </button>
                    <button
                      type="button"
                      className="workspace-tool-close"
                      aria-label="Close files panel"
                      onClick={() => {
                        if (isSideBySide) {
                          setSplitPanels(false);
                          setActiveTool('git');
                        } else {
                          setActiveTool(null);
                        }
                      }}
                    >×</button>
                  </div>
                </div>
                <div className="file-panel-body">
                  <div className="file-panel-resizer" onPointerDown={(event) => resizeDockedPanel(event, activeFileWidth, setFilePanelWidth)} role="separator" aria-orientation="vertical" aria-label="Resize files panel" />
                  <WorkspaceFiles key={projectId ?? 'no-project'} projectId={projectId} />
                </div>
              </aside>
            )}
            {(activeTool === 'git' || isSideBySide) && (
              <aside className="file-panel git-panel" aria-label="Git changes" style={{width: `${activeGitWidth}px`}}>
                <div className="file-panel-header">
                  <div className="file-panel-tabs" role="tablist" aria-label="Workspace panels">
                    <button type="button" role="tab" aria-selected={activeTool === 'files'} className={`file-panel-tab ${activeTool === 'files' ? 'active' : ''}`} onClick={() => setActiveTool('files')}>
                      <DocumentTextIcon width={14} height={14} />Files
                    </button>
                    <button type="button" role="tab" aria-selected={activeTool === 'git'} className={`file-panel-tab ${activeTool === 'git' ? 'active' : ''}`} onClick={() => setActiveTool('git')}>
                      <GitDiffIcon width={14} height={14} />Git
                    </button>
                  </div>
                  {projectName && <span className="file-panel-project-tag">{projectName}</span>}
                  <div className="file-panel-header-controls">
                    <button type="button" className={`file-panel-split-toggle ${splitPanels ? 'active' : ''}`} aria-label={splitPanels ? 'Show one workspace panel' : 'Show files and Git side by side'} title={splitPanels ? 'Show one panel' : 'Split panels'} onClick={() => setSplitPanels(value => !value)}>
                      <SquaresPlusIcon width={15} height={15} />
                    </button>
                    <button
                      type="button"
                      className="workspace-tool-close"
                      aria-label="Close Git panel"
                      onClick={() => {
                        if (isSideBySide) {
                          setSplitPanels(false);
                          setActiveTool('files');
                        } else {
                          setActiveTool(null);
                        }
                      }}
                    >×</button>
                  </div>
                </div>
                <div className="file-panel-body">
                  <div className="file-panel-resizer" onPointerDown={(event) => resizeDockedPanel(event, activeGitWidth, setGitPanelWidth)} role="separator" aria-orientation="vertical" aria-label="Resize Git panel" />
                  <WorkspaceGitDiff key={projectId ?? 'no-project'} projectId={projectId} workspaceStatus={workspaceStatus} gitDiff={gitDiff} onRefresh={() => void refreshGitContext()} />
                </div>
            </aside>
          )}
        </div>
      )}
      {activeTool === 'browser' && (
        <BrowserSidePanel
          width={browserPanelWidth}
          setWidth={setBrowserPanelWidth}
          onClose={() => setActiveTool(null)}
        />
      )}
      <div className="notice" id="notice" role="status" aria-live="polite"></div>
    </div>
  );
}
