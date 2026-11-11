import {useEffect, useRef, useState} from 'react';
import {ChevronDownIcon, ArrowUpIcon, PaperClipIcon, StopIcon} from '@heroicons/react/20/solid';
import {apiGetApprovalMode, apiListProviders, apiListSkills, apiUpdateApprovalMode, type ProviderInfo} from '../services/tauriBridge';

type Model = {
  id: string;
  name: string;
  provider: string;
  supportsVision: boolean;
  maxContextTokens: number;
  maxOutputTokens: number;
};

type AttachmentInfo = {
  name: string;
  size: number;
  content: string;
  isImage?: boolean;
};

export function Composer({running = false, onStop}: {running?: boolean; onStop?: () => void}) {
  const [prompt, setPrompt] = useState('');
  const [modelPickerOpen, setModelPickerOpen] = useState(false);
  const [models, setModels] = useState<Model[]>([]);
  const [selectedModel, setSelectedModel] = useState<Model | null>(null);
  const [projectId, setProjectId] = useState<string | null>(null);
  const [attachments, setAttachments] = useState<AttachmentInfo[]>([]);
  const [approvalMode, setApprovalMode] = useState<'manual' | 'automatic'>(() => localStorage.getItem('openharness:approval-mode') === 'automatic' ? 'automatic' : 'manual');
  const [selectedSkills, setSelectedSkills] = useState<string[]>(() => {
    try {
      const value: unknown = JSON.parse(localStorage.getItem('openharness:global-skills') || '[]');
      return Array.isArray(value) ? value.filter((item): item is string => typeof item === 'string') : [];
    } catch { return []; }
  });
  const [skillPickerOpen, setSkillPickerOpen] = useState(false);
  const [availableSkills, setAvailableSkills] = useState<{name: string; description: string}[]>([]);
  const pickerRef = useRef<HTMLDivElement>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const approvalModeRef = useRef(approvalMode);
  approvalModeRef.current = approvalMode;
  const selectedSkillsRef = useRef(selectedSkills);
  selectedSkillsRef.current = selectedSkills;

  const loadModels = async () => {
    try {
      const providers: ProviderInfo[] = await apiListProviders();
      const list: Model[] = providers.map((p) => ({id: p.model_id, name: `${p.provider} · ${p.model_id}`, provider: p.provider, supportsVision: p.supports_vision ?? false, maxContextTokens: p.max_context_tokens ?? 128_000, maxOutputTokens: p.max_output_tokens ?? 4_096}));
      const savedDefault = localStorage.getItem('openharness:default-model');
      if (savedDefault) {
        const parsed: Model = JSON.parse(savedDefault);
        if (!list.some((m) => m.id === parsed.id && m.provider === parsed.provider)) {
          list.unshift({...parsed, supportsVision: parsed.supportsVision ?? false, maxContextTokens: parsed.maxContextTokens ?? 128_000, maxOutputTokens: parsed.maxOutputTokens ?? 4_096});
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

  useEffect(() => { void loadModels(); void apiListSkills().then(setAvailableSkills).catch(() => setAvailableSkills([])); }, []);
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
    if (!skillPickerOpen) return;
    const closeOnEscape = (event: KeyboardEvent) => { if (event.key === 'Escape') setSkillPickerOpen(false); };
    document.addEventListener('keydown', closeOnEscape);
    return () => document.removeEventListener('keydown', closeOnEscape);
  }, [skillPickerOpen]);

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
    let disposed = false;
    const textarea = document.getElementById('prompt');
    const paste = (event: ClipboardEvent) => {
      const target = event.target;
      if (!(target instanceof HTMLElement) || target.id !== 'prompt' || !textarea?.contains(target)) return;
      const images = Array.from(event.clipboardData?.items ?? [])
        .filter(item => item.type.startsWith('image/'))
        .map(item => item.getAsFile())
        .filter((file): file is File => file !== null);
      if (images.length) {
        event.preventDefault();
        const transfer = new DataTransfer();
        images.forEach(image => transfer.items.add(image));
        void handleFiles(transfer.files);
      }
    };
    void apiListSkills().then(skills => { if (!disposed) setAvailableSkills(skills); }).catch(() => {});
    void apiGetApprovalMode().then(mode => {
      approvalModeRef.current = mode;
      if (!disposed) setApprovalMode(mode);
    }).catch(() => {});
    const updateSkills = (event: Event) => {
      const detail = (event as CustomEvent<{names?: string[]; approvalMode?: 'manual' | 'automatic'}>).detail;
      const names = detail?.names ?? [];
      if (detail?.approvalMode) {
        const mode = detail.approvalMode === 'automatic' ? 'automatic' : 'manual';
        approvalModeRef.current = mode;
          setApprovalMode(mode);
      }
      setSelectedSkills(names);
    };
    document.addEventListener('paste', paste);
    window.addEventListener('openharness:skills-changed', updateSkills);
    return () => {
      document.removeEventListener('paste', paste);
      window.removeEventListener('openharness:skills-changed', updateSkills);
      disposed = true;
    };
  }, []);

  useEffect(() => {
    const setText = (event: Event) => setPrompt(String((event as CustomEvent<string>).detail || ''));
    window.addEventListener('openharness:set-prompt', setText);
    return () => window.removeEventListener('openharness:set-prompt', setText);
  }, []);

  const handleFiles = async (files: FileList | null) => {
    if (!files) return;
    const next: AttachmentInfo[] = [];
    for (const file of Array.from(files)) {
      const isImg = file.type.startsWith('image/') || /\.(png|jpe?g|webp|gif|svg)$/i.test(file.name);
      const maxSize = isImg ? 4 * 1024 * 1024 : 256 * 1024;
      if (file.size > maxSize) {
        toastError(`Attachment too large: ${file.name} (max ${isImg ? '4MB' : '256KB'})`);
        continue;
      }
      try {
        if (isImg) {
          const dataUrl = await new Promise<string>((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(String(reader.result || ''));
            reader.onerror = () => reject(reader.error);
            reader.readAsDataURL(file);
          });
          next.push({
            name: file.name,
            size: file.size,
            isImage: true,
            content: `![${file.name}](${dataUrl})`,
          });
        } else {
          const content = await new Promise<string>((resolve, reject) => {
            const reader = new FileReader();
            reader.onload = () => resolve(String(reader.result || ''));
            reader.onerror = () => reject(reader.error);
            reader.readAsText(file);
          });
          next.push({
            name: file.name,
            size: file.size,
            isImage: false,
            content: `[${file.name}]\n${content}`,
          });
        }
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
    const fileText = attachments.filter(attachment => !attachment.isImage).map(attachment => attachment.content).join('\n\n');
    const combined = [fileText, text].filter(Boolean).join('\n\n');
    const model = selectedModel;
    if (!model) return;
    const imageAttachments = attachments.filter(item => item.isImage);
    if (imageAttachments.length && !model.supportsVision) {
      toastError('Selected model does not support images. Remove image attachments or choose a vision-capable model.');
      return;
    }
    const detail = {text: combined, modelId: model.id, provider: model.provider, projectId, attachments: attachments.map(a => a.content), supportsVision: model.supportsVision, modelContextTokens: model.maxContextTokens, modelOutputTokens: model.maxOutputTokens, selectedSkills, approvalMode: approvalModeRef.current};
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
        onChange={(e) => {
          const value = e.target.value;
          setPrompt(value);
          setSkillPickerOpen(/(?:^|\s)\/[\w-]*$/.test(value) && availableSkills.length > 0);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Escape' && skillPickerOpen) {
            setSkillPickerOpen(false);
            return;
          }
          if (e.key === 'Enter' && !e.shiftKey && !skillPickerOpen) {
            e.preventDefault();
            send();
          }
        }}
      />
      <div className="composer-hint" aria-label="Composer capabilities">Use /skill-name to invoke a skill · paste or attach images · drop files to add context</div>
      {skillPickerOpen && (
        <div className="composer-skill-menu" role="listbox" aria-label="Available skills">
          {availableSkills.map(skill => (
            <button type="button" role="option" aria-selected={selectedSkills.includes(skill.name)} key={skill.name} onClick={() => {
              const next = selectedSkillsRef.current.includes(skill.name) ? selectedSkillsRef.current.filter(name => name !== skill.name) : [...selectedSkillsRef.current, skill.name];
              selectedSkillsRef.current = next;
              setSelectedSkills(next);
              localStorage.setItem('openharness:global-skills', JSON.stringify(next));
              window.dispatchEvent(new CustomEvent('openharness:skills-changed', {detail: {names: next}}));
              setPrompt(value => value.replace(/(?:^|\s)\/[\w-]*$/, '').trimStart());
              setSkillPickerOpen(false);
            }}>{skill.name}<span>{skill.description}</span></button>
          ))}
        </div>
      )}
      {running && <span className="composer-running-label" role="status" aria-live="polite">Working — new prompts will queue</span>}
      <div className="composer-row">
        <input ref={fileInputRef} type="file" accept="image/*,.txt,.md,.json,.ts,.js,.rs,.py,.css,.html" multiple style={{display:'none'}} onChange={(e) => { void handleFiles(e.target.files); e.target.value = ''; }} />
        <button className="model-button" type="button" aria-label="Attach files or images" title="Attach files or images" onClick={() => fileInputRef.current?.click()}>
          <PaperClipIcon width={16} height={16} aria-hidden="true" />
        </button>
        <label className="approval-mode-control" htmlFor="approval-mode">Approval
          <select id="approval-mode" aria-label="Tool approval mode" value={approvalMode} onChange={event => {
            const value = event.target.value === 'automatic' ? 'automatic' : 'manual';
            approvalModeRef.current = value;
            setApprovalMode(value);
            void apiUpdateApprovalMode(value).catch(error => {
              approvalModeRef.current = value === 'automatic' ? 'manual' : 'automatic';
              const fallback = approvalModeRef.current;
              setApprovalMode(fallback);
              toastError(`Could not save approval mode: ${String(error)}`);
            });
          }}>
            <option value="manual">Manual</option>
            <option value="automatic">Automatic</option>
          </select>
        </label>
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
                  {m.name}{m.supportsVision ? ' · Images' : ''}
                </button>
              );
            })}
          </div>
        </div>
        {running && <button className="send-button stop-generation" type="button" onClick={onStop} aria-label="Stop generation" title="Stop generation">
          <StopIcon width={14} aria-hidden="true" />
        </button>}
        <button className="send-button" type="submit" aria-label="Send message" title="Send">
          <ArrowUpIcon width={16} height={16} aria-hidden="true" />
        </button>
      </div>
    </form>
  );
}