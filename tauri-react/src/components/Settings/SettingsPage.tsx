import {useEffect, useRef, useState} from 'react';
import {isTauri} from '@tauri-apps/api/core';
import {
  ArrowPathIcon, ArrowTopRightOnSquareIcon,
  MagnifyingGlassIcon, ServerIcon, XMarkIcon, TrashIcon, PencilSquareIcon, PhotoIcon,
} from '@heroicons/react/20/solid';
import {apiDeleteProvider, apiGetStartupReadiness, apiListProviders, apiSaveProvider, type ProviderInfo, type ReadinessComponentLike} from '../../services/tauriBridge';
import {classifyProviderEndpoint} from './providerEndpoint';
import {containTabKey} from '../modalFocus';
import {createDownloadProgress} from './updateProgress';

type SettingsPageProps = {onClose: () => void; initialSection?: string};

const sections = ['Providers', 'System Status', 'Update'];
const providerOptions = ['OpenRouter', 'Google AI Studio', 'Ollama', 'OpenAI Compatible'];

const OPENAI_PRESETS = [
  { label: 'LM Studio', url: 'http://localhost:1234/v1', model: 'local-model' },
  { label: 'Ollama', url: 'http://localhost:11434/v1', model: 'llama3.3' },
  { label: 'vLLM', url: 'http://localhost:8000/v1', model: 'meta-llama/Llama-3-8B-Instruct' },
  { label: 'OpenAI', url: 'https://api.openai.com/v1', model: 'gpt-4o' },
  { label: 'DeepSeek', url: 'https://api.deepseek.com/v1', model: 'deepseek-chat' },
  { label: 'Groq', url: 'https://api.groq.com/openai/v1', model: 'llama-3.3-70b-versatile' },
];

const providerDocs: Record<string, string> = {
  'OpenRouter': 'https://openrouter.ai/docs/quickstart',
  'Google AI Studio': 'https://ai.google.dev/gemini-api/docs/openai',
  'Ollama': 'https://ollama.com/library',
  'OpenAI Compatible': 'https://platform.openai.com/docs/api-reference/chat/create',
};

function ProviderForm({onBack, onSaved, editProvider}: {onBack: () => void; onSaved: () => void; editProvider?: ProviderInfo}) {
  const [provider, setProvider] = useState(editProvider?.provider ?? providerOptions[0]);
  const [apiKey, setApiKey] = useState('');
  const [modelId, setModelId] = useState(editProvider?.model_id ?? '');
  const [baseUrl, setBaseUrl] = useState(editProvider?.base_url ?? '');
  const [maxContextTokens, setMaxContextTokens] = useState(editProvider?.max_context_tokens ?? 128_000);
  const [maxOutputTokens, setMaxOutputTokens] = useState(editProvider?.max_output_tokens ?? 4_096);
  const [supportsVision, setSupportsVision] = useState(editProvider?.supports_vision ?? false);
  const [customEndpoint, setCustomEndpoint] = useState(editProvider?.custom_endpoint ?? false);
  const [advanced, setAdvanced] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');
  const isOpenAICompatible = provider === 'OpenAI Compatible';

  const saveProvider = async () => {
    if ((provider !== 'Ollama' && provider !== 'OpenAI Compatible' && !apiKey.trim() && !editProvider?.has_key) || !modelId.trim()) {
      setError(provider === 'Ollama' ? 'Enter a Model ID before saving.' : 'Enter an API key and Model ID before saving.');
      return;
    }
    if (isOpenAICompatible || customEndpoint) {
      const endpoint = classifyProviderEndpoint(baseUrl);
      if (endpoint.kind === 'invalid') {
        setError(endpoint.message);
        return;
      }
      if (endpoint.kind === 'remote_http') {
        setError('Remote HTTP is unencrypted: credentials and request contents can be read or modified in transit. Use HTTPS or a local loopback endpoint.');
        return;
      }
    }
    const parsedContext = Number(maxContextTokens);
    const parsedOutput = Number(maxOutputTokens);
    if (!Number.isInteger(parsedContext) || parsedContext < 1024 || parsedContext > 2_000_000 || !Number.isInteger(parsedOutput) || parsedOutput < 1 || parsedOutput > parsedContext) {
      setError('Context must be 1,024–2,000,000 tokens; output must be within the context window.');
      return;
    }
    setSaving(true);
    setError('');
    try {
      await apiSaveProvider(
        provider,
        modelId.trim(),
        (isOpenAICompatible || customEndpoint) ? baseUrl.trim() : null,
        isOpenAICompatible || customEndpoint,
        apiKey,
        maxContextTokens,
        maxOutputTokens,
        supportsVision,
        editProvider?.has_key ?? false
      );
      setApiKey('');
      window.dispatchEvent(new Event('openharness:provider-saved'));
      onSaved();
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className={`provider-form ${provider === 'OpenAI Compatible' ? 'provider-form-openai' : ''}`}>
      {provider === 'OpenAI Compatible' && (
        <div className="provider-openai-callout">
          <div className="provider-openai-callout-icon"><ServerIcon width={20}/></div>
          <div className="provider-openai-callout-copy">
            <strong>OpenAI-compatible endpoint</strong>
            <span>Connect to any API that speaks the OpenAI chat completions protocol — OpenAI, LM Studio, vLLM, llama.cpp, or a local gateway.</span>
          </div>
        </div>
      )}
      <div className="provider-form-header"><ServerIcon width={19}/><strong>{provider === 'OpenAI Compatible' ? 'Connect a compatible API' : 'Add provider'}</strong></div>
      <label className="field-label" htmlFor="provider-select">Provider</label>
      <div className="provider-select-row">
        <select id="provider-select" value={provider} disabled={Boolean(editProvider)} onChange={event => { const next = event.target.value; setProvider(next); if (next === 'OpenAI Compatible') { setCustomEndpoint(true); setAdvanced(true); if (!baseUrl) setBaseUrl('https://api.openai.com/v1'); } setError(''); }}>
          {providerOptions.map(option => <option key={option}>{option}</option>)}
        </select>
        <a className="provider-setup" href={providerDocs[provider]} target="_blank" rel="noreferrer">Docs <ArrowTopRightOnSquareIcon width={13}/></a>
      </div>
      {provider === 'OpenAI Compatible' ? (
        <div className="provider-openai-fields">
          <div className="provider-openai-header-row">
            <label className="field-label" htmlFor="openai-endpoint">Endpoint Base URL</label>
            {(() => {
              const endpoint = classifyProviderEndpoint(baseUrl);
              return <span className={`provider-openai-protocol-badge ${endpoint.kind === 'remote_http' ? 'endpoint-insecure' : ''}`}>
                {endpoint.kind === 'invalid' ? 'Invalid endpoint' : endpoint.label}
              </span>;
            })()}
          </div>
          <input
            id="openai-endpoint"
            value={baseUrl}
            onChange={event => setBaseUrl(event.target.value)}
            placeholder="http://localhost:1234/v1 or https://api.openai.com/v1"
          />
          <span className="provider-openai-hint">
            Specify the base URL. <code>/chat/completions</code> is automatically appended.
            {classifyProviderEndpoint(baseUrl).kind === 'remote_http' && ' Remote HTTP does not encrypt API keys or request contents; saving is blocked.'}
          </span>

          <div className="provider-openai-presets-wrap">
            <span className="provider-presets-label">Quick Presets:</span>
            <div className="provider-openai-presets" role="group" aria-label="Quick preset endpoints">
              {OPENAI_PRESETS.map(preset => (
                <button
                  key={preset.label}
                  type="button"
                  className={`provider-preset-btn ${baseUrl.trim() === preset.url ? 'active' : ''}`}
                  onClick={() => {
                    setBaseUrl(preset.url);
                    if (!modelId.trim() || OPENAI_PRESETS.some(p => p.model === modelId.trim())) {
                      setModelId(preset.model);
                    }
                    setError('');
                  }}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </div>

          <label className="field-label" htmlFor="model-id">Model ID</label>
          <input
            id="model-id"
            value={modelId}
            onChange={event => setModelId(event.target.value)}
            placeholder="e.g. gpt-4o, llama-3.3-70b, or deepseek-chat"
          />

          <div className="provider-openai-header-row">
            <label className="field-label" htmlFor="api-key">API Key</label>
            <span className="provider-openai-optional">OPTIONAL FOR LOCAL</span>
          </div>
          <input
            id="api-key"
            type="password"
            value={apiKey}
            onChange={event => setApiKey(event.target.value)}
            placeholder="Leave empty for local servers (LM Studio, Ollama, vLLM)"
          />
        </div>
      ) : (
        <>
          <label className="field-label" htmlFor="model-id">Model ID</label>
          <input id="model-id" value={modelId} onChange={event => setModelId(event.target.value)} placeholder="e.g. google/gemini-2.5-flash-lite" />
          <label className="field-label" htmlFor="api-key">API key {provider === 'Ollama' ? '(optional for local endpoints)' : ''}</label>
          <input id="api-key" type="password" value={apiKey} onChange={event => setApiKey(event.target.value)} placeholder={provider === 'Ollama' ? "Optional for local endpoints, required for remote APIs" : "Paste your API key"} />
          <button className="advanced-toggle" type="button" aria-expanded={advanced} onClick={() => setAdvanced(value => !value)}>Advanced endpoint settings {advanced ? '▾' : '▸'}</button>
          {advanced && (
            <div className="advanced-panel">
              <label className="check-row"><input type="checkbox" checked={customEndpoint} onChange={event => setCustomEndpoint(event.target.checked)} /> Use custom endpoint</label>
              <label>Endpoint URL<input value={baseUrl} onChange={event => setBaseUrl(event.target.value)} placeholder="https://…" disabled={!customEndpoint} /></label>
            </div>
          )}
        </>
      )}
      <section className="provider-model-limits" aria-label="Model capabilities and limits">
        <header><div><PhotoIcon width={17} aria-hidden="true"/><strong>Model capabilities</strong></div><span>Set model-specific limits</span></header>
        <label className="provider-vision-toggle"><span><strong>Image input</strong><small>Model accepts image attachments</small></span><input type="checkbox" checked={supportsVision} onChange={event => setSupportsVision(event.target.checked)} aria-label="Model supports images" /></label>
        <div className="provider-token-fields">
          <label>Context window (tokens)<input type="number" min={1024} max={2000000} step={1024} value={maxContextTokens} onChange={event => setMaxContextTokens(Number(event.target.value))} /></label>
          <label>Max output (tokens)<input type="number" min={1} max={maxContextTokens} step={256} value={maxOutputTokens} onChange={event => setMaxOutputTokens(Number(event.target.value))} /></label>
        </div>
      </section>
      {error && <div role="alert" className="provider-form-error">{error}</div>}
      <div className="provider-form-footer">
        <span>{editProvider?.has_key ? 'Leave the API key blank to keep the saved credential.' : isOpenAICompatible ? 'API keys are stored through your operating system’s secure credential store.' : <>Keys are stored through your operating system&rsquo;s secure credential store.</>}</span>
        <div>
          <button className="save-provider" type="button" onClick={() => void saveProvider()} disabled={saving}>{saving ? 'Saving…' : 'Save'}</button>
          <button className="back-button" type="button" onClick={onBack}>Back</button>
        </div>
      </div>
    </div>
  );
}

function ProviderList({items, onAdd, onEdit, onDelete}: {items: ProviderInfo[]; onAdd: () => void; onEdit: (provider: ProviderInfo) => void; onDelete: (provider: string) => void}) {
  const [pending, setPending] = useState<string | null>(null);
  if (items.length === 0) {
    return (
      <div className="provider-emptystate">
        <p>No providers configured yet.</p>
        <button type="button" className="save-provider" onClick={onAdd}>Add provider</button>
      </div>
    );
  }
  return (
    <div className="provider-list">
      {items.map((item) => (
        <div className="saved-provider" key={item.provider}>
          <div className="saved-provider-copy">
            <strong>{item.provider}</strong>
            <small>{item.model_id}{item.has_key ? ' · Key secured' : ' · Missing key'}{item.supports_vision ? ' · Images' : ''}</small>
            <small>{item.max_context_tokens.toLocaleString()} context · {item.max_output_tokens.toLocaleString()} output tokens</small>
          </div>
          {pending === item.provider ? (
            <div className="provider-delete-confirm">
              <span>Remove?</span>
              <button type="button" onClick={() => { void (async () => { await onDelete(item.provider); setPending(null); })(); }}>Remove</button>
              <button type="button" className="back-button" onClick={() => setPending(null)}>Cancel</button>
            </div>
          ) : (
            <div className="saved-provider-actions">
              <button type="button" className="iconbtn" aria-label={`Edit ${item.provider}`} title="Edit provider" onClick={() => onEdit(item)}><PencilSquareIcon width={16}/></button>
              <button type="button" className="iconbtn" aria-label={`Remove ${item.provider}`} title="Remove provider" onClick={() => setPending(item.provider)}><TrashIcon width={16}/></button>
            </div>
          )}
        </div>
      ))}
      <button className="add-provider" onClick={onAdd}>+ Add provider</button>
    </div>
  );
}

export function SettingsPage({onClose, initialSection = 'Providers'}: SettingsPageProps) {
  const [selectedSection, setSelectedSection] = useState(initialSection);
  const [providerView, setProviderView] = useState<'list' | 'form'>('list');
  const [editingProvider, setEditingProvider] = useState<ProviderInfo | undefined>();
  const [search, setSearch] = useState('');
  const [savedProviders, setSavedProviders] = useState<ProviderInfo[]>([]);
  const [providerError, setProviderError] = useState('');
  const [readiness, setReadiness] = useState<ReadinessComponentLike[]>([]);
  const [readinessError, setReadinessError] = useState('');
  const [refreshingReadiness, setRefreshingReadiness] = useState(false);
  const [readinessRevision, setReadinessRevision] = useState(0);
  const [updateStatus, setUpdateStatus] = useState('');
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const updateInFlight = useRef(false);
  const updateMounted = useRef(true);
  const closeRef = useRef<HTMLButtonElement | null>(null);
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const backdropRef = useRef<HTMLDivElement | null>(null);
  const openerRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    updateMounted.current = true;
    return () => { updateMounted.current = false; };
  }, []);

  useEffect(() => {
    openerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const backdrop = backdropRef.current;
    const background = backdrop?.parentElement ? Array.from(backdrop.parentElement.children).filter((child): child is HTMLElement => child instanceof HTMLElement && child !== backdrop) : [];
    const previous = background.map(element => element.inert);
    background.forEach(element => { element.inert = true; });
    const close = closeRef.current;
    if (close) close.focus();
    return () => {
      background.forEach((element, index) => { element.inert = previous[index]; });
      if (openerRef.current?.isConnected) openerRef.current.focus();
      else document.querySelector<HTMLElement>('[aria-label="Settings"]')?.focus();
    };
  }, []);

  const handleDialogKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    containTabKey(event.nativeEvent, event.currentTarget);
    if (event.key === 'Escape' && !checkingUpdate) {
      event.preventDefault();
      onClose();
    }
  };

  useEffect(() => {
    let cancelled = false;
    apiListProviders().then(items => { if (!cancelled) setSavedProviders(items); }).catch(error => { if (!cancelled) setProviderError(String(error)); });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const loadReadiness = async () => {
      setRefreshingReadiness(true);
      setReadinessError('');
      try {
        const items = await apiGetStartupReadiness();
        if (!cancelled) setReadiness(items);
      } catch {
        if (!cancelled) setReadinessError('Could not load system readiness.');
      } finally {
        if (!cancelled) setRefreshingReadiness(false);
      }
    };
    let cancelled = false;
    void loadReadiness();
    return () => { cancelled = true; };
  }, [readinessRevision]);

  useEffect(() => {
    const refresh = () => setReadinessRevision(value => value + 1);
    window.addEventListener('openharness:provider-saved', refresh);
    return () => window.removeEventListener('openharness:provider-saved', refresh);
  }, []);

  const refreshReadiness = () => setReadinessRevision(value => value + 1);
  const statusLabel = (status: ReadinessComponentLike['status']) => status.replaceAll('_', ' ');
  const actionableMessage = (component: ReadinessComponentLike) => component.message;

  const readinessPanel = (
    <section className="update-panel" aria-labelledby="readiness-heading">
      <div className="updates-row"><div className="updates-icon"><ServerIcon width={18}/></div><div className="updates-copy"><h2 id="readiness-heading">System status</h2><p>Component health and actions. Secrets are never shown.</p></div></div>
      <div className="update-row"><span className="update-status">{readinessError || (refreshingReadiness ? 'Checking components…' : 'Startup diagnostics')}</span><button className="check-updates" onClick={refreshReadiness} disabled={refreshingReadiness}>{refreshingReadiness ? 'Checking…' : 'Refresh status'}</button></div>
      <div className="readiness-list" role="list">
        {readiness.map(component => <div className="readiness-item" role="listitem" key={component.id}><div className="readiness-heading-row"><strong>{component.displayName}</strong><span className={`readiness-status readiness-${component.status}`}>{statusLabel(component.status)}</span></div><p>{actionableMessage(component)}</p></div>)}
      </div>
    </section>
  );
  const visibleSections = sections.filter(section => section.toLowerCase().includes(search.toLowerCase()));
  const activeSection = visibleSections.includes(selectedSection) ? selectedSection : (visibleSections[0] ?? '');
  const noResults = search.trim() !== '' && visibleSections.length === 0;

  const updateProviders = async () => {
    try {
      setSavedProviders(await apiListProviders());
      setProviderError('');
      refreshReadiness();
    } catch (error) {
      setProviderError(String(error));
    }
  };

  const checkForUpdates = async () => {
    if (updateInFlight.current) return;
    updateInFlight.current = true;
    setCheckingUpdate(true);
    setUpdateStatus('Checking for updates…');
    const showStatus = (message: string) => { if (updateMounted.current) setUpdateStatus(message); };
    try {
      if (!isTauri()) {
        showStatus('Updates are available only in the desktop app.');
        return;
      }
      const {check} = await import('@tauri-apps/plugin-updater');
      const {relaunch} = await import('@tauri-apps/plugin-process');
      const update = await check();
      if (!update) {
        let currentVer = '';
        try {
          const {getVersion} = await import('@tauri-apps/api/app');
          currentVer = await getVersion();
        } catch {}
        showStatus(currentVer ? `You are on the latest version (v${currentVer}).` : 'You are on the latest version.');
        return;
      }
      showStatus(`Update available: v${update.version} (current: v${update.currentVersion}). Starting download…`);
      const progress = createDownloadProgress();
      let totalBytes: number | undefined;
      await update.downloadAndInstall(event => {
        if (event.event === 'Started') {
          totalBytes = event.data.contentLength;
          showStatus(totalBytes && totalBytes > 0 ? `Downloading v${update.version}… 0 KB of ${Math.ceil(totalBytes / 1024).toLocaleString()} KB` : `Downloading v${update.version}…`);
        } else if (event.event === 'Progress') {
          const current = progress.add(event.data.chunkLength, totalBytes);
          const totalText = current.total === null ? '' : ` of ${Math.ceil(current.total / 1024).toLocaleString()} KB`;
          showStatus(`Downloading v${update.version}… ${Math.ceil(current.downloaded / 1024).toLocaleString()} KB${totalText}`);
        } else if (event.event === 'Finished') {
          showStatus(`Download complete. Installing v${update.version}…`);
        }
      });
      showStatus(`Update v${update.version} installed. Relaunching…`);
      await relaunch();
    } catch (error) {
      showStatus(`Update failed: ${String(error)}`);
    } finally {
      updateInFlight.current = false;
      if (updateMounted.current) setCheckingUpdate(false);
    }
  };

  return (
    <div ref={backdropRef} className="settings-modal-backdrop" onMouseDown={event => { if (event.target === event.currentTarget) onClose(); }}>
          <div ref={dialogRef} className="settings-page settings-modal" role="dialog" aria-modal="true" aria-label="Settings" tabIndex={-1} onKeyDown={handleDialogKeyDown} onClick={e => e.stopPropagation()}>
      <button ref={closeRef} className="settings-close settings-modal-close" type="button" aria-label="Close settings" title="Close settings" onClick={onClose}><XMarkIcon width={16}/></button>
      <aside className="settings-nav">
        <div className="settings-nav-header"><h2>Settings</h2></div>
        <label className="settings-search"><MagnifyingGlassIcon width={15}/><input value={search} onChange={event => setSearch(event.target.value)} placeholder="Search settings" aria-label="Search settings"/></label>
        <nav>
          {noResults ? <p className="settings-nav-empty">No matching settings.</p>
            : visibleSections.map(section => (
              <button key={section} className={activeSection === section ? 'settings-nav-item active' : 'settings-nav-item'} onClick={() => {setSelectedSection(section); setProviderView('list');}}>
                {section}
                {section === 'Providers' && savedProviders.length > 0 && <span className="nav-count">{savedProviders.length}</span>}
              </button>
            ))}
        </nav>
      </aside>
      <main className="settings-content">
        {activeSection === 'System Status' ? readinessPanel : activeSection === 'Providers' ? <>
          <section className="settings-section-heading"><h1>Providers</h1><p>Connect a service to choose which models power your conversations.</p></section>
          <div className="providers-card">
            <section className="provider-detail">
              {providerView === 'list' ? <>
                <ProviderList items={savedProviders} onAdd={() => {setEditingProvider(undefined); setProviderView('form');}} onEdit={provider => {setEditingProvider(provider); setProviderView('form');}} onDelete={async provider => {await apiDeleteProvider(provider); await updateProviders(); window.dispatchEvent(new Event('openharness:provider-saved'));}}/>
                {providerError && <span role="alert" className="provider-error">{providerError}</span>}
              </> : <ProviderForm editProvider={editingProvider} onBack={() => setProviderView('list')} onSaved={async () => {await updateProviders(); setProviderView('list'); setEditingProvider(undefined);}}/>}
            </section>
          </div>
        </> : activeSection === 'Update' ? <>
          <section className="settings-section-heading"><h1>Updates</h1><p>Desktop application updater</p></section>
          <section className="update-panel">
            <div className="updates-row">
              <div className="updates-icon"><ArrowPathIcon width={18}/></div>
              <div className="updates-copy"><h2>App updates</h2><p>Update availability is provided by the desktop application.</p></div>
            </div>
            <div className="update-row">
              <span className="update-status">{updateStatus || 'Check for the latest version of OpenHarness.'}</span>
              <button className="check-updates" onClick={checkForUpdates} disabled={checkingUpdate} title="Check for updates">{checkingUpdate ? 'Updating…' : 'Check for updates'}</button>
            </div>
          </section>
        </> : (
          <section className="settings-section-heading"><h1>Providers</h1><p>Section not found.</p></section>
        )}
      </main>
      </div>
    </div>
  );
}
