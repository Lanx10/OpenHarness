import {useEffect, useRef, useState} from 'react';
import {PlusIcon, ComputerDesktopIcon, ClipboardDocumentIcon, ArrowPathIcon, ArrowDownTrayIcon, TrashIcon} from '@heroicons/react/20/solid';
import {Composer} from './Composer';
import {apiChatStream, apiAppendMessage, apiCreateThread} from '../services/tauriBridge';

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

export function Hero() {
  const [localDetail, setLocalDetail] = useState('Open Settings → Model Providers to add OpenRouter, Google AI Studio, or Ollama.');
  return (
    <section className="hero" aria-hidden="false">
      <div className="hero-mark" aria-hidden="true">
        <img src="/appicon/openharness-desktop-mark-only-512-v2.svg" alt="OpenHarness" width={38} height={38} style={{display: 'block'}} />
      </div>
      <h1 data-od-id="hero-heading">What can I help build?</h1>
      <p className="hero-note">Start with a task, or pick up where you left off.</p>
      <div className="quick-actions">
        <button className="quick-pill" id="quick-project" type="button" data-od-id="quick-project" onClick={() => window.dispatchEvent(new Event('openharness:new-project'))}>
          <PlusIcon width={10} height={10} aria-hidden="true" /> New project
        </button>
        <button type="button" className="quick-pill local" onClick={() => { setLocalDetail('Use Model Providers to test Ollama or another configured service.'); window.dispatchEvent(new CustomEvent('openharness:open-settings', {detail: {section: 'Model Providers'}})); }} title={localDetail}>
          <ComputerDesktopIcon width={11} height={11} aria-hidden="true" /> Model Providers
        </button>
      </div>
    </section>
  );
}

type MessageProps = {message: Message; onCopy: () => void; onDelete: () => void; onRegenerate: () => void};

function MessageView({message, onCopy, onDelete, onRegenerate}: MessageProps) {
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
        <button type="button" onClick={() => { if (editing) { toast('Edit saved locally'); setEditing(false); } else setEditing(true); }} aria-label={editing ? 'Save edit' : 'Edit'}>{editing ? 'Save' : 'Edit'}</button>
        <button type="button" onClick={onDelete} aria-label="Delete"><TrashIcon width={12}/></button>
      </div>
    </article>
  );
}

export function Canvas() {
  const [messages, setMessages] = useState<Message[]>([]);
  const [threadId, setThreadId] = useState<string | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [stopHandle, setStopHandle] = useState<(() => void) | null>(null);
  const conversationRef = useRef<HTMLElement | null>(null);

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
          if (!cancelled) setThreadId(thread.id);
        } catch {}
      }
      const userMessage: Message = {id: `u-${Date.now()}`, role: 'user', text, status: 'done', attachments: detail.attachments};
      setMessages(prev => [...prev, userMessage]);
      const assistantId = `a-${Date.now()}`;
      setMessages(prev => [...prev, {id: assistantId, role: 'assistant', text: '', status: 'thinking'}]);
      if (currentThreadId) void apiAppendMessage(currentThreadId, 'user', text);
      let cancelledLocally = false;
      const cancel = () => { cancelledLocally = true; toast('Stopped generation'); };
      setStopHandle(() => cancel);
      try {
        await apiChatStream(text, detail.modelId, detail.provider, currentThreadId ?? undefined, (token) => {
          if (cancelledLocally) return;
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text: message.text + token, status: 'done'} : message));
        }, () => {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, status: 'done'} : message));
        }, (error) => {
          setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text: error, status: 'error'} : message));
          toast(error);
        });
      } catch (error) {
        setMessages(prev => prev.map(message => message.id === assistantId ? {...message, text: String(error), status: 'error'} : message));
      } finally {
        setStopHandle(null);
      }
    };
    const onNewChat = () => {
      setMessages([]);
      setThreadId(null);
      toast('New chat started');
    };
    const onSelectProject = (event: Event) => {
      const project = (event as CustomEvent<{id?: string}>).detail;
      setProjectId(project?.id ?? null);
      setThreadId(null);
      setMessages([]);
    };
    window.addEventListener('openharness:send-message', onSend as EventListener);
    window.addEventListener('openharness:new-chat', onNewChat);
    window.addEventListener('openharness:select-project', onSelectProject as EventListener);
    return () => { cancelled = true; window.removeEventListener('openharness:send-message', onSend as EventListener); window.removeEventListener('openharness:new-chat', onNewChat); window.removeEventListener('openharness:select-project', onSelectProject as EventListener); };
  }, [threadId, projectId]);

  useEffect(() => {
    const element = conversationRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [messages]);

  const stopGenerating = () => stopHandle?.();

  const copyMessage = async (message: Message) => {
    try { await navigator.clipboard.writeText(message.text); toast('Copied'); }
    catch { toast('Copy failed'); }
  };

  const deleteMessage = (id: string) => {
    setMessages(prev => prev.filter(message => message.id !== id));
  };

  const regenerateMessage = (id: string) => {
    const target = messages.find(message => message.id === id);
    if (!target) return;
    toast('Regeneration will resend the prompt');
    window.dispatchEvent(new CustomEvent('openharness:set-prompt', {detail: target.text}));
  };

  const empty = messages.length === 0;
  return (
    <main className="canvas">
      {stopHandle && <button className="stop-generation" type="button" onClick={stopGenerating} title="Stop generation" aria-label="Stop generation"><ArrowDownTrayIcon width={14} aria-hidden="true" style={{transform: 'rotate(180deg)'}}/></button>}
      {empty ? <Hero/> : null}
      <section className="conversation" aria-live="polite" ref={conversationRef} hidden={empty}>
        {messages.map((message) => (
          <MessageView key={message.id} message={message} onCopy={() => copyMessage(message)} onDelete={() => deleteMessage(message.id)} onRegenerate={() => regenerateMessage(message.id)} />
        ))}
      </section>
      <div className="composer-wrap">
        <Composer />
      </div>
      <div className="notice" id="notice" role="status" aria-live="polite"></div>
    </main>
  );
}
