import {useEffect, useRef, useState} from 'react';
import {ChevronDownIcon, ArrowUpIcon, PaperClipIcon} from '@heroicons/react/20/solid';
import {apiListProviders, type ProviderInfo} from '../services/tauriBridge';

type Model = {
  id: string;
  name: string;
  provider: string;
};

type AttachmentInfo = {
  name: string;
  size: number;
  content: string;
};

export function Composer() {
  const [prompt, setPrompt] = useState('');
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [models, setModels] = useState<Model[]>([]);
  const [selectedModel, setSelectedModel] = useState<Model | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [attachments, setAttachments] = useState<AttachmentInfo[]>([]);
  const pickerRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const loadModels = async () => {
    try {
      const providers: ProviderInfo[] = await apiListProviders();
      const list: Model[] = providers.map((p) => ({id: p.model_id, name: `${p.provider} · ${p.model_id}`, provider: p.provider}));
      const savedDefault = localStorage.getItem('openharness:default-model');
      if (savedDefault) {
        const parsed: Model = JSON.parse(savedDefault);
        if (!list.some((m) => m.id === parsed.id && m.provider === parsed.provider)) {
          list.unshift(parsed);
        }
      }
      setModels(list);
      if (list.length && !selectedModel) {
        const match = savedDefault ? list.find((m) => m.id === JSON.parse(savedDefault).id && m.provider === JSON.parse(savedDefault).provider) : null;
        setSelectedModel(match || list[0]);
      }
    } catch {
      setModels([]);
    }
  };

  useEffect(() => { void loadModels(); }, []);
  useEffect(() => {
    const refresh = () => { void loadModels(); };
    window.addEventListener('openharness:provider-saved', refresh);
    return () => window.removeEventListener('openharness:provider-saved', refresh);
  }, []);

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

  const handleFiles = async (files: FileList | null) => {
    if (!files) return;
    const next: AttachmentInfo[] = [];
    for (const file of Array.from(files)) {
      if (file.size > 256 * 1024) {
        toastError(`Attachment too large: ${file.name}`);
        continue;
      }
      try {
        const content = await new Promise<string>((resolve, reject) => {
          const reader = new FileReader();
          reader.onload = () => resolve(String(reader.result || ''));
          reader.onerror = () => reject(reader.error);
          reader.readAsText(file);
        });
        next.push({ name: file.name, size: file.size, content: `[${file.name}]\n${content}` });
      } catch {
        toastError(`Could not read: ${file.name}`);
      }
    }
    if (next.length) setAttachments(prev => [...prev, ...next]);
  };

  const toastError = (message: string) => {
    const notice = document.getElementById('notice');
    if (notice) notice.textContent = message;
  };

  const removeAttachment = (index: number) => {
    setAttachments(prev => prev.filter((_, i) => i !== index));
  };

  const send = () => {
    const text = prompt.trim();
    if (!text && attachments.length === 0) return;
    const combined = attachments.length ? `${attachments.map(a => a.content).join('\n\n')}\n\n${text}` : text;
    const model = selectedModel;
    if (!model) return;
    const detail = {text: combined, modelId: model.id, provider: model.provider, projectId, attachments: attachments.map(a => a.content)};
    window.dispatchEvent(new CustomEvent('openharness:send-message', {detail}));
    setPrompt('');
    setAttachments([]);
    localStorage.setItem('openharness:default-model', JSON.stringify(model));
  };

  return (
    <form
      className="composer"
      id="composer-form"
      onSubmit={(e) => {
        e.preventDefault();
        send();
      }}
      onDragOver={(e) => { e.preventDefault(); }}
      onDrop={(e) => { e.preventDefault(); void handleFiles(e.dataTransfer.files); }}
    >
      {attachments.length > 0 && (
        <div className="attachments" aria-label="Attachments">
          {attachments.map((item, index) => (
            <span key={`${item.name}:${index}`} className="attachment-chip">
              <span title={item.name}>{item.name} · {Math.max(1, Math.ceil(item.size / 1024))} KB</span>
              <button type="button" onClick={() => removeAttachment(index)} aria-label={`Remove ${item.name}`} title="Remove attachment">×</button>
            </span>
          ))}
        </div>
      )}
      <textarea
        id="prompt"
        rows={2}
        aria-label="Message"
        placeholder="Message OpenHarness…  (Enter to send, Shift+Enter for a new line)"
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
        <input ref={fileInputRef} type="file" multiple style={{display:'none'}} onChange={(e) => { void handleFiles(e.target.files); e.target.value = ''; }} />
        <button className="model-button" type="button" aria-label="Attach files" title="Attach files" onClick={() => fileInputRef.current?.click()}>
          <PaperClipIcon width={16} height={16} aria-hidden="true" />
        </button>
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
            <span id="model-label">{selectedModel ? selectedModel.name : 'No models configured'}</span>
            <ChevronDownIcon width={12} height={12} aria-hidden="true" />
          </button>
          <div className={`model-menu ${modelPickerOpen ? 'open' : ''}`} id="model-menu" role="listbox" aria-label="Choose model">
            {models.length === 0 ? <div className="model-option">Add a provider in Settings</div> : models.map((m) => {
              const isChosen = selectedModel?.id === m.id && selectedModel?.provider === m.provider;
              return (
                <button
                  key={`${m.provider}:${m.id}`}
                  type="button"
                  className={`model-option ${isChosen ? 'chosen' : ''}`}
                  role="option"
                  aria-selected={isChosen}
                  onClick={() => {
                    setSelectedModel(m);
                    setModelPickerOpen(false);
                    localStorage.setItem('openharness:default-model', JSON.stringify(m));
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