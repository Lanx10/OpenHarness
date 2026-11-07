import {useEffect, useRef, useState} from 'react';
import {PlusIcon, ComputerDesktopIcon} from '@heroicons/react/20/solid';
import {Composer} from './Composer';
import {apiChatSend, type ChatResponse} from '../services/tauriBridge';

type Message = {role: 'user'|'assistant'; text: string; status?: 'thinking'|'done'|'error'};

export function Hero() {
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
        <button type="button" className="quick-pill local" onClick={() => window.dispatchEvent(new Event('openharness:open-settings'))}>
          <ComputerDesktopIcon width={11} height={11} aria-hidden="true" /> Local
        </button>
      </div>
    </section>
  );
}

export function Canvas() {
  const [messages, setMessages] = useState<Message[]>([]);
  const conversationRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    let requestId = 0;
    const onSend = async (event: Event) => {
      const detail = (event as CustomEvent<{text: string; modelId: string; provider: string; projectId?: string}>).detail;
      const text = String(detail?.text ?? '').trim();
      if (!text) return;
      const id = ++requestId;
      setMessages(prev => [...prev, {role: 'user', text}, {role: 'assistant', text: '', status: 'thinking'}]);
      try {
        const response: ChatResponse = await apiChatSend(text, detail.modelId, detail.provider, detail.projectId);
        setMessages(prev => id === requestId ? [...prev.slice(0, -1), {role: 'assistant', text: response.text, status: 'done'}] : prev);
      } catch (error) {
        setMessages(prev => id === requestId ? [...prev.slice(0, -1), {role: 'assistant', text: String(error), status: 'error'}] : prev);
      }
    };
    const clear = () => { requestId += 1; setMessages([]); };
    window.addEventListener('openharness:send-message', onSend);
    window.addEventListener('openharness:new-chat', clear);
    return () => {
      window.removeEventListener('openharness:send-message', onSend);
      window.removeEventListener('openharness:new-chat', clear);
    };
  }, []);

  useEffect(() => {
    const element = conversationRef.current;
    if (element) element.scrollTop = element.scrollHeight;
  }, [messages]);

  const empty = messages.length === 0;

  return (
    <main className="canvas">
      {empty ? <Hero/> : null}
      <section className="conversation" aria-live="polite" ref={conversationRef} hidden={empty}>
        {messages.map((message, index) => (
          <article key={index} className={message.role === 'user' ? 'message user' : 'message assistant'}>
            <p>{message.status === 'thinking' ? 'Thinking…' : message.text}</p>
          </article>
        ))}
      </section>
      <div className="composer-wrap">
        <Composer />
      </div>
      <div className="notice" id="notice" role="status" aria-live="polite"></div>
    </main>
  );
}
