import {useEffect, useRef, useState} from 'react';
import {ChevronDownIcon, ArrowUpIcon} from '@heroicons/react/20/solid';

type Model = {
  id: string;
  name: string;
  provider: string;
};

const MODELS: Model[] = [
  {id: 'claude-3-7-sonnet', name: 'Claude Sonnet', provider: 'Anthropic-compatible'},
  {id: 'claude-3-opus', name: 'Claude Opus', provider: 'Anthropic-compatible'},
  {id: 'gpt-4.1', name: 'GPT-4.1', provider: 'OpenAI-compatible'},
  {id: 'deepseek-r1', name: 'DeepSeek R1', provider: 'OpenRouter'},
  {id: 'ollama-local', name: 'Ollama (Local)', provider: 'Ollama'},
];

export function Composer() {
  const [prompt, setPrompt] = useState('');
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [selectedModel, setSelectedModel] = useState<Model>(MODELS[0]);
  const [projectId, setProjectId] = useState<string | null>(null);
  const pickerRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const handleSelectProject = (event: Event) => {
      const project = (event as CustomEvent<{id?: string}>).detail;
      setProjectId(project?.id ?? null);
    };
    window.addEventListener('openharness:select-project', handleSelectProject);
    return () => window.removeEventListener('openharness:select-project', handleSelectProject);
  }, []);

  useEffect(() => {
    if (!modelPickerOpen) return;
    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (!pickerRef.current?.contains(event.target as Node)) setModelPickerOpen(false);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setModelPickerOpen(false);
    };
    document.addEventListener('pointerdown', closeOnOutsidePointer);
    document.addEventListener('keydown', closeOnEscape);
    return () => {
      document.removeEventListener('pointerdown', closeOnOutsidePointer);
      document.removeEventListener('keydown', closeOnEscape);
    };
  }, [modelPickerOpen]);

  useEffect(() => {
    const setText = (event: Event) => setPrompt(String((event as CustomEvent<string>).detail || ''));
    window.addEventListener('openharness:set-prompt', setText);
    return () => window.removeEventListener('openharness:set-prompt', setText);
  }, []);

  const send = () => {
    const text = prompt.trim();
    if (!text) return;
    const detail = {text, modelId: selectedModel.id, provider: selectedModel.provider, projectId};
    window.dispatchEvent(new CustomEvent('openharness:send-message', {detail}));
    setPrompt('');
  };

  return (
    <form
      className="composer"
      id="composer-form"
      onSubmit={(e) => {
        e.preventDefault();
        send();
      }}
    >
      <textarea
        id="prompt"
        rows={2}
        aria-label="Message"
        placeholder="Ask anything, or describe what you want to build…"
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' && !e.shiftKey) {
            e.preventDefault();
            send();
          }
        }}
      />
      <div className="composer-row">
        {/* ponytail: attachment UI removed because file contents are not sent; add back with actual content/path handling. */}
        <span className="composer-spacer" />
        <div ref={pickerRef} className="model-wrap">
          <button
            className="model-button"
            id="model-button"
            type="button"
            aria-haspopup="listbox"
            aria-expanded={modelPickerOpen}
            onClick={() => setModelPickerOpen((open) => !open)}
          >
            <span id="model-label">{selectedModel.name}</span>
            <ChevronDownIcon width={12} height={12} aria-hidden="true" />
          </button>
          <div className={`model-menu ${modelPickerOpen ? 'open' : ''}`} id="model-menu" role="listbox" aria-label="Choose model">
            {MODELS.map((m) => {
              const isChosen = m.id === selectedModel.id;
              return (
                <button
                  key={m.id}
                  type="button"
                  className={`model-option ${isChosen ? 'chosen' : ''}`}
                  role="option"
                  aria-selected={isChosen}
                  onClick={() => {
                    setSelectedModel(m);
                    setModelPickerOpen(false);
                  }}
                >
                  {m.name}
                </button>
              );
            })}
          </div>
        </div>
        <button className="send-button" type="submit" aria-label="Send message" title="Send">
          <ArrowUpIcon width={16} height={16} aria-hidden="true" />
        </button>
      </div>
    </form>
  );
}
