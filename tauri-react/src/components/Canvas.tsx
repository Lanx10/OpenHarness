import {useEffect, useRef, useState} from 'react';
import {isTauri} from '@tauri-apps/api/core';
import {PlusIcon, ComputerDesktopIcon, ClipboardDocumentIcon, ArrowPathIcon, ArrowDownTrayIcon, TrashIcon, ChevronLeftIcon, ChevronRightIcon, DocumentTextIcon, GlobeAltIcon} from '@heroicons/react/20/solid';
import {Composer} from './Composer';
import {
  apiChatStream,
  apiWorkspaceListDir,
  apiWorkspaceReadFile,
  type FileEntryLike,
  apiCreateThread,
  apiCancelChatStream,
  apiListThreads,
  apiGetThread,
  apiDeleteThread,
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
  type ApprovalRequestLike,
} from '../services/tauriBridge';

type Message = {id: string; role: 'user'|'assistant'; text: string; status?: 'thinking'|'done'|'error'; attachments?: string[]};

function toast(message: string) {
  const notice = document.getElementById('notice');
  if (notice) {
    notice.textContent = message;
    window.setTimeout(() => { if (notice.textContent === message) notice.textContent = ''; }, 2500);
  }
}

function escapeHtml(text: string) {
  return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function renderMarkdown(text: string) {
  const blocks = text.split('```');
  return blocks.map((block, index) => {
    if (index % 2 === 1) {
      const [first, ...rest] = block.split('\n');
      const language = first.trim();
      const code = rest.join('\n');
      return (
        <div key={index} className="code-block">
          <div className="code-header">
            <span>{language || 'code'}</span>
            <button type="button" onClick={() => navigator.clipboard.writeText(code)} aria-label="Copy code">Copy</button>
          </div>
          <pre><code>{code}</code></pre>
        </div>
      );
    }
    const html = escapeHtml(block).replace(/\*\*(.*?)\*\*/g, '<strong>$1</strong>').replace(/`([^`]+)`/g, '<code>$1</code>').replace(/\n/g, '<br/>');
    return <div key={index} className="markdown-block" dangerouslySetInnerHTML={{__html: html}} />;
  });
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

type MessageProps = {message: Message; onCopy: () => void; onDelete: () => void; onRegenerate: () => void; onSaveEdit: (text: string) => void};

function MessageView({message, onCopy, onDelete, onRegenerate, onSaveEdit}: MessageProps) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(message.text);
  useEffect(() => setDraft(message.text), [message.text]);
  return (
    <article className={message.role === 'user' ? 'message user' : 'message assistant'} aria-live="polite">
      {editing ? (
        <textarea value={draft} onChange={e => setDraft(e.target.value)} aria-label="Edit message" />
      ) : (
        message.status === 'thinking' ? <p>Thinking…</p> : renderMarkdown(message.text)
      )}
      <div className="message-actions">
        <button type="button" onClick={onCopy} aria-label="Copy"><ClipboardDocumentIcon width={12}/></button>
        {message.role === 'assistant' && <button type="button" onClick={onRegenerate} aria-label="Regenerate"><ArrowPathIcon width={12}/></button>}
        <button type="button" onClick={() => { if (editing) { onSaveEdit(draft); setEditing(false); } else setEditing(true); }} aria-label={editing ? 'Save edit' : 'Edit'}>{editing ? 'Save' : 'Edit'}</button>
        <button type="button" onClick={onDelete} aria-label="Delete"><TrashIcon width={12}/></button>
      </div>
    </article>
  );
}

type TerminalTab = {id: string; label: string; shell: 'powershell'|'cmd'; output: string; status: 'running'|'exited'};

export function Canvas() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [threads, setThreads] = useState<ChatThreadLike[]>([]);
  const [threadId, setThreadId] = useState<string | null>(null);
  const [threadsCollapsed, setThreadsCollapsed] = useState(() => {
    try { return localStorage.getItem('openharness:threads-collapsed') === 'true'; } catch { return false; }
  });
  const [projectId, setProjectId] = useState<string | null>(null);
  const [projectName, setProjectName] = useState('');

  const toggleThreads = () => {
    setThreadsCollapsed(prev => {
      const next = !prev;
      try { localStorage.setItem('openharness:threads-collapsed', String(next)); } catch {}
      return next;
    });
  };
  const [stopHandle, setStopHandle] = useState<(() => void) | null>(null);
  const [approval, setApproval] = useState<ApprovalRequestLike | null>(null);
  const [workspaceStatus, setWorkspaceStatus] = useState('');
  const [gitDiff, setGitDiff] = useState('');
  const [activeTool, setActiveTool] = useState<'terminal'|'browser'|'files'|'git'|null>(null);
  const [terminalHeight, setTerminalHeight] = useState(() => {
    try { const saved = Number(localStorage.getItem('openharness:terminal-height')); return Number.isFinite(saved) && saved >= 140 && saved <= 600 ? saved : 240; }
    catch { return 240; }
  });
  const [filePanelWidth, setFilePanelWidth] = useState(() => {
    try { const saved = Number(localStorage.getItem('openharness:file-panel-width')); return Number.isFinite(saved) && saved >= 280 && saved <= 900 ? saved : 420; }
    catch { return 420; }
  });
  const [terminalTabs, setTerminalTabs] = useState<TerminalTab[]>([]);
  const [activeTerminalId, setActiveTerminalId] = useState<string | null>(null);
  const [terminalCommand, setTerminalCommand] = useState('');
  const [terminalBusy, setTerminalBusy] = useState(false);
  const terminalOutputRef = useRef<HTMLPreElement | null>(null);
  const terminalInputRef = useRef<HTMLInputElement | null>(null);
  const conversationRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    try { localStorage.setItem('openharness:terminal-height', String(terminalHeight)); } catch {}
  }, [terminalHeight]);
  useEffect(() => {
    try { localStorage.setItem('openharness:file-panel-width', String(filePanelWidth)); } catch {}
  }, [filePanelWidth]);

  const refreshWorkspaceGit = async (currentProjectId: string | null) => {
    if (!currentProjectId) {
      setWorkspaceStatus('');
      setGitDiff('');
      return;
    }
    try {
      const status = await apiGitStatus(currentProjectId);
      setWorkspaceStatus(status.trim());
    } catch {
      setWorkspaceStatus('');
    }
    try {
      setGitDiff((await apiGitDiff(currentProjectId)).trim());
    } catch {
      setGitDiff('');
    }
  };

  const loadThreadMessages = (thread: ChatThreadLike) => {
    setThreadId(thread.id);
    setMessages(thread.messages.map((message, index) => ({
      id: message.id || `${thread.id}:${index}`,
      role: message.role === 'assistant' ? 'assistant' : 'user',
      text: message.text,
      status: 'done',
      attachments: message.attachments,
    })));
  };

  const refreshThreads = async (nextProjectId: string | null) => {
    try {
      const items = await apiListThreads(nextProjectId ?? undefined);
      setThreads(items);
      window.dispatchEvent(new CustomEvent('openharness:threads-updated', {detail: items}));
      return items;
    } catch {
      setThreads([]);
      window.dispatchEvent(new CustomEvent('openharness:threads-updated', {detail: []}));
      return [];
    }
  };

  useEffect(() => {
    void refreshThreads(null);
  }, []);

  useEffect(() => {
    let cancelled = false;
    const onSend = async (event: Event) => {
      const detail = (event as CustomEvent<{text: string; modelId: string; provider: string; projectId?: string; attachments?: string[]}>).detail;
      const text = String(detail?.text ?? '').trim();
      if (!text) return;
      let currentThreadId = threadId;
      if (!currentThreadId && projectId) {
        try {
          const thread = await apiCreateThread(projectId);
          currentThreadId = thread.id;
          if (!cancelled) { setThreadId(thread.id); setThreads(prev => [thread, ...prev]); }
        } catch (error) { toast(`Could not create chat: ${String(error)}`); return; }
      }
      const userMessage: Message = {id: `u-${Date.now()}`, role: 'user', text, status: 'done', attachments: detail.attachments};
      setMessages(prev => [...prev, userMessage]);
      const assistantId = `a-${Date.now()}`;
      setMessages(prev => [...prev, {id: assistantId, role: 'assistant', text: '', status: 'thinking'}]);
      const cancel = () => {
        void apiCancelChatStream().catch(error => toast(`Could not stop: ${String(error)}`));
      };
      setStopHandle(() => cancel);
      try {
        await apiChatStream(text, detail.modelId, detail.provider, projectId ?? undefined, currentThreadId ?? undefined, detail.attachments, (token) => {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text: message.text + token, status: 'thinking'} : message));
        }, () => {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, status: 'done'} : message));
          void refreshWorkspaceGit(projectId ?? null);
        }, (error) => {
          const errText = String(error);
          if (errText.includes('Generation stopped')) {
            setMessages(prev => prev.map(message => message.id === assistantId ? {...message, status: 'done'} : message));
            toast('Generation stopped');
          } else {
            setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text: errText, status: 'error'} : message));
            toast(errText);
          }
        }, (approvalRequest) => setApproval(approvalRequest));
      } catch (error) {
        const text = String(error);
        if (text.includes('Generation stopped')) {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, status: 'done'} : message));
          toast('Generation stopped');
        } else {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text, status: 'error'} : message));
          toast(text);
        }
      } finally {
        setStopHandle(null);
        if (currentThreadId) {
          try {
            const thread = await apiGetThread(currentThreadId);
            loadThreadMessages(thread);
          } catch {}
          void refreshThreads(projectId);
        }
      }
    };
    const onNewChat = async () => {
      setMessages([]);
      setThreadId(null);
      toast('New chat started');
      if (projectId) await refreshThreads(projectId);
    };
    const onOpenThread = async (selectedThreadId: string) => {
      try {
        const thread = await apiGetThread(selectedThreadId);
        loadThreadMessages(thread);
        setThreadId(thread.id);
      } catch (error) { toast(String(error)); }
    };
    const onSelectProject = async (event: Event) => {
      const project = (event as CustomEvent<{id?: string; name?: string; title?: string}>).detail;
      const nextProjectId = project?.id ?? null;
      setProjectId(nextProjectId);
      setProjectName(project?.name ?? project?.title ?? '');
      setThreadId(null);
      setMessages([]);
      void refreshWorkspaceGit(nextProjectId);
      const nextThreads = await refreshThreads(nextProjectId);
      if (nextThreads.length) loadThreadMessages(nextThreads[0]);
    };
    const onDeleteThread = async (selectedThreadId: string) => {
      if (!window.confirm('Delete this chat?')) return;
      try {
        const remaining = await apiDeleteThread(selectedThreadId);
        const filtered = projectId ? remaining.filter(t => t.project_id === projectId) : remaining;
        setThreads(filtered);
        if (threadId === selectedThreadId) { setThreadId(null); setMessages([]); if (filtered.length) loadThreadMessages(filtered[0]); }
      } catch (error) { toast(String(error)); }
    };
    const handleOpenThread = (event: Event) => void onOpenThread(String((event as CustomEvent<string>).detail));
    const handleDeleteThread = (event: Event) => void onDeleteThread(String((event as CustomEvent<string>).detail));
    const handleQuickCmd = () => setActiveTool('terminal');
    const handleQuickFiles = () => setActiveTool('files');
    const handleQuickGit = () => { setActiveTool('git'); void refreshGitContext(); };
    const handleQuickBrowser = () => setActiveTool('browser');
    window.addEventListener('openharness:quick-powershell', () => void createTerminal('powershell'));
    window.addEventListener('openharness:quick-cmdprompt', () => void createTerminal('cmd'));
    window.addEventListener('openharness:quick-cmd', handleQuickCmd);
    window.addEventListener('openharness:quick-files', handleQuickFiles);
    window.addEventListener('openharness:quick-git', handleQuickGit);
    window.addEventListener('openharness:quick-browser', handleQuickBrowser);
    window.addEventListener('openharness:new-chat', onNewChat);
    window.addEventListener('openharness:select-project', onSelectProject as EventListener);
    window.addEventListener('openharness:open-thread', handleOpenThread);
    window.addEventListener('openharness:delete-thread', handleDeleteThread);
    return () => { cancelled = true; window.removeEventListener('openharness:send-message', onSend as EventListener); window.removeEventListener('openharness:new-chat', onNewChat); window.removeEventListener('openharness:select-project', onSelectProject as EventListener); window.removeEventListener('openharness:open-thread', handleOpenThread); window.removeEventListener('openharness:delete-thread', handleDeleteThread); window.removeEventListener('openharness:quick-cmd', handleQuickCmd); window.removeEventListener('openharness:quick-files', handleQuickFiles); window.removeEventListener('openharness:quick-git', handleQuickGit); window.removeEventListener('openharness:quick-browser', handleQuickBrowser); };
  }, [threadId, projectId]);

  useEffect(() => {
    const element = conversationRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [messages]);

  useEffect(() => {
    let unlistenOutput: (() => void) | undefined;
    let unlistenExit: (() => void) | undefined;
    void apiListenTerminalOutput((id, data) => {
      setTerminalTabs(previous => previous.map(tab => tab.id === id ? {...tab, output: (tab.output + data).slice(-200_000)} : tab));
    }).then(unlisten => { unlistenOutput = unlisten; });
    void apiListenTerminalExit((id, code) => {
      setTerminalTabs(previous => previous.map(tab => tab.id === id ? {...tab, status: 'exited', output: `${tab.output}\\n[Process exited: ${code ?? 'unknown'}]\\n`} : tab));
    }).then(unlisten => { unlistenExit = unlisten; });
    return () => { unlistenOutput?.(); unlistenExit?.(); };
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

  const resizeFilePanel = (event: React.PointerEvent<HTMLDivElement>) => {
    const startX = event.clientX;
    const startWidth = filePanelWidth;
    const move = (pointer: PointerEvent) => {
      const width = Math.min(Math.max(280, startWidth + startX - pointer.clientX), Math.min(900, window.innerWidth - 320));
      setFilePanelWidth(width);
    };
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

  const closeTerminal = async (id: string) => {
    try { await apiTerminalKill(id); }
    catch (error) { toast(`Terminal close failed: ${String(error)}`); }
    setTerminalTabs(previous => previous.filter(tab => tab.id !== id));
    if (activeTerminalId === id) setActiveTerminalId(terminalTabs.find(tab => tab.id !== id)?.id ?? null);
  };

  const createTerminal = async (shell: 'powershell' | 'cmd') => {
    if (!isTauri()) {
      toast('Terminal requires the desktop app');
      return;
    }
    setTerminalBusy(true);
    try {
      const id = await apiTerminalCreate(projectId, shell);
      const label = shell === 'cmd' ? 'Command Prompt' : 'PowerShell';
      setTerminalTabs(previous => [...previous, {id, label, shell, output: '', status: 'running'}]);
      setActiveTerminalId(id);
      setActiveTool('terminal');
    } catch (error) {
      toast(`Terminal failed to start: ${String(error)}`);
    } finally {
      setTerminalBusy(false);
    }
  };

  const sendTerminalInput = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!activeTerminalId || !terminalCommand.trim()) return;
    const command = terminalCommand;
    setTerminalCommand('');
    try { await apiTerminalWrite(activeTerminalId, `${command}\\r\\n`); }
    catch (error) { toast(`Terminal input failed: ${String(error)}`); }
  };

  const approveToolCall = async (allowed: boolean) => {
    if (!approval) return;
    try {
      await apiRespondToToolApproval(approval.id, allowed);
      setApproval(null);
    } catch (error) {
      toast(`Approval failed: ${String(error)}`);
    }
  };

  const refreshGitContext = async () => {
    if (!projectId) { setWorkspaceStatus('Select a project to view Git status.'); setGitDiff(''); return; }
    try {
      const [status, diff] = await Promise.all([apiGitStatus(projectId), apiGitDiff(projectId)]);
      setWorkspaceStatus(status.trim());
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
  return (
    <div className="canvas-layout">
      {threadsCollapsed ? (
        <aside className="thread-sidebar thread-sidebar-collapsed" aria-label="Chat threads collapsed">
          <button type="button" className="collapse-toggle-btn thread-toggle" onClick={toggleThreads} aria-label="Expand threads sidebar" title="Expand threads sidebar">
            <ChevronRightIcon width={14} height={14} />
          </button>
        </aside>
      ) : (
        <aside className="thread-sidebar" aria-label="Chat threads">
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
                <button type="button" className={`thread-item ${thread.id === threadId ? 'thread-item-active' : ''}`} onClick={() => window.dispatchEvent(new CustomEvent('openharness:open-thread', {detail: thread.id}))}>{thread.title}</button>
                <button type="button" className="thread-delete" aria-label={`Delete ${thread.title}`} onClick={() => window.dispatchEvent(new CustomEvent('openharness:delete-thread', {detail: thread.id}))}>×</button>
              </li>
            ))}
          </ul>
          {!threads.length && <p className="thread-empty">No chats saved yet.</p>}
        </aside>
      )}
      <main className={`canvas ${activeTool === 'files' ? 'canvas-files-open' : ''} ${activeTool === 'terminal' ? 'canvas-terminal-open' : ''}`} style={{'--terminal-height': `${terminalHeight}px`} as React.CSSProperties}>
        {workspaceStatus && (
          <div className="git-status-badge" aria-label="Git repository status" title={workspaceStatus}>
            <span>{workspaceStatus}</span>
            <button type="button" onClick={() => void refreshGitContext()} aria-label="Refresh git status and diff" title="Refresh git context">Refresh</button>
          </div>
        )}
        {activeTool && activeTool !== 'terminal' && activeTool !== 'files' && <section className="workspace-tool-panel" aria-label={`${activeTool} panel`}>
          <div className="workspace-tool-header">
            <div className="workspace-tool-title">{activeTool === 'browser' ? <GlobeAltIcon width={16} height={16}/> : <span aria-hidden="true">⑂</span>}{activeTool === 'browser' ? 'Browser' : 'Git diff'}</div>
            <button type="button" className="workspace-tool-close" aria-label="Close workspace tool" onClick={() => setActiveTool(null)}>×</button>
          </div>
          <div className="workspace-tool-body">
            {activeTool === 'browser' && <div className="browser-empty"><GlobeAltIcon width={22} height={22}/><span>Browser tool is not configured.</span></div>}
            {activeTool === 'git' && <div className="git-panel"><strong>{workspaceStatus || 'No Git status available.'}</strong><pre className="git-output">{gitDiff || 'Working tree clean; no diff to show.'}</pre></div>}
          </div>
        </section>}
        {stopHandle && (
          <button className="stop-generation" type="button" onClick={stopGenerating} title="Stop generation" aria-label="Stop generation">
            <ArrowDownTrayIcon width={14} aria-hidden="true" />
          </button>
        )}
        {empty && <Hero className={activeTool === 'terminal' ? 'hero-above-terminal' : undefined} />}
        <section className="conversation" aria-live="polite" ref={conversationRef} hidden={empty}>
          {messages.map(message => (
            <MessageView key={message.id} message={message} onCopy={() => copyMessage(message)} onDelete={() => deleteMessage(message.id)} onRegenerate={() => regenerateMessage(message.id)} onSaveEdit={text => void saveMessageEdit(message.id, text)} />
          ))}
        </section>
        <div className="composer-wrap">
          <Composer />
          {approval && (
            <div className="approval-backdrop" role="alertdialog" aria-modal="true" aria-labelledby="tool-approval-title">
              <div className="approval-dialog">
                <h2 id="tool-approval-title">Approve workspace operation</h2>
                <p><strong>{approval.tool}</strong> requests:</p>
                <pre>{JSON.stringify(approval.arguments, null, 2)}</pre>
                <div className="approval-actions">
                  <button type="button" onClick={() => void approveToolCall(false)}>Reject</button>
                  <button type="button" className="confirm" onClick={() => void approveToolCall(true)}>Approve once</button>
                </div>
              </div>
            </div>
          )}
        </div>
        {activeTool === 'terminal' && (
          <section className="terminal-panel" aria-label="Terminal bottom panel" style={{height: `${terminalHeight}px`}}>
            <div className="terminal-resizer" onPointerDown={resizeTerminal} role="separator" aria-orientation="horizontal" aria-label="Resize terminal panel" />
            <div className="terminal-header">
              <div className="terminal-tabs" role="tablist" aria-label="Terminal sessions">
                {terminalTabs.map(tab => (
                  <button key={tab.id} type="button" role="tab" aria-selected={tab.id === activeTerminalId} className={tab.id === activeTerminalId ? 'terminal-tab active' : 'terminal-tab'} onClick={() => setActiveTerminalId(tab.id)}>
                    {tab.label} {tab.status === 'exited' ? '· exited' : ''}
                  </button>
                ))}
                <button type="button" className="terminal-tab terminal-new" onClick={() => void createTerminal('powershell')} disabled={terminalBusy}>+ PowerShell</button>
                <button type="button" className="terminal-tab terminal-new" onClick={() => void createTerminal('cmd')} disabled={terminalBusy}>+ Command Prompt</button>
              </div>
              <button type="button" className="workspace-tool-close" aria-label="Close terminal panel" onClick={() => setActiveTool(null)}>×</button>
            </div>
            <pre className="terminal-output" ref={terminalOutputRef} tabIndex={0} aria-label="Terminal output">{terminalTabs.find(tab => tab.id === activeTerminalId)?.output || 'No terminal sessions. Use + PowerShell or + Command Prompt.'}</pre>
            {(() => {
              const active = terminalTabs.find(tab => tab.id === activeTerminalId);
              if (active?.status === 'running') {
                return (
                  <form className="terminal-input-row" onSubmit={sendTerminalInput}>
                    <input ref={terminalInputRef} value={terminalCommand} onChange={e => setTerminalCommand(e.target.value)} aria-label="Terminal command" placeholder={active.shell === 'cmd' ? 'Command Prompt' : 'PowerShell'} autoComplete="off" spellCheck={false} />
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
        {activeTool === 'files' && <aside className="file-panel" aria-label="Files panel" style={{width: `${filePanelWidth}px`}}>
          <div className="file-panel-header"><span className="workspace-tool-title"><DocumentTextIcon width={16} height={16}/>Files{projectName ? ` · ${projectName}` : ''}</span><button type="button" className="workspace-tool-close" aria-label="Close files panel" onClick={() => setActiveTool(null)}>×</button></div>
          <div className="file-panel-body"><div className="file-panel-resizer" onPointerDown={resizeFilePanel} role="separator" aria-orientation="vertical" aria-label="Resize files panel"/><WorkspaceFiles key={projectId ?? 'no-project'} projectId={projectId}/></div>
        </aside>}
        <div className="notice" id="notice" role="status" aria-live="polite"></div>
      </main>
    </div>
  );
}
