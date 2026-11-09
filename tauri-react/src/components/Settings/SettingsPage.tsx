import {useEffect, useRef, useState} from 'react';
import {isTauri} from '@tauri-apps/api/core';
import {
  ArrowPathIcon, ArrowTopRightOnSquareIcon,
  MagnifyingGlassIcon, ServerIcon, XMarkIcon, TrashIcon,
} from '@heroicons/react/20/solid';
import {apiDeleteProvider, apiListProviders, apiSaveProvider, type ProviderInfo} from '../../services/tauriBridge';

type SettingsPageProps = {onClose: () => void; initialSection?: string};

const sections = ['Providers', 'Update'];
const providerOptions = ['OpenRouter', 'Google AI Studio', 'Ollama', 'OpenAI Compatible'];

const providerDocs: Record<string, string> = {
  'OpenRouter': 'https://openrouter.ai/docs/quickstart',
  'Google AI Studio': 'https://ai.google.dev/gemini-api/docs/openai',
  'Ollama': 'https://ollama.com/library',
  'OpenAI Compatible': 'https://platform.openai.com/docs/api-reference/chat/create',
};

function ProviderForm({onBack, onSaved}: {onBack: () => void; onSaved: () => void}) {
  const [provider, setProvider] = useState(providerOptions[0]);
  const [apiKey, setApiKey] = useState('');
  const [modelId, setModelId] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [customEndpoint, setCustomEndpoint] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  const saveProvider = async () => {
    if ((provider !== 'Ollama' && provider !== 'OpenAI Compatible' && !apiKey.trim()) || !modelId.trim()) {
      setError(provider === 'Ollama' ? 'Enter a Model ID before saving.' : 'Enter an API key and Model ID before saving.');
      return;
    }
    if (customEndpoint) {
      if (!baseUrl.trim()) {
        setError('Enter an endpoint URL when custom endpoint is enabled.');
        return;
      }
      try {
        const parsed = new URL(baseUrl);
        if (!['http:', 'https:'].includes(parsed.protocol) || !parsed.hostname) throw new Error('bad URL');
      } catch {
        setError('Endpoint URL must be a valid http(s) URL.');
        return;
      }
    }
    setSaving(true);
    setError('');
    try {
      await apiSaveProvider(provider, modelId.trim(), customEndpoint ? baseUrl.trim() : null, customEndpoint, apiKey);
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
    <div className="provider-form">
      <div className="provider-form-header"><ServerIcon width={19}/><strong>Add provider</strong></div>
      <label className="field-label" htmlFor="provider-select">Provider</label>
      <div className="provider-select-row">
        <select id="provider-select" value={provider} onChange={event => { const next = event.target.value; setProvider(next); if (next === 'OpenAI Compatible') setCustomEndpoint(true); setError(''); }}>
          {providerOptions.map(option => <option key={option}>{option}</option>)}
        </select>
        <a className="provider-setup" href={providerDocs[provider]} target="_blank" rel="noreferrer">Docs <ArrowTopRightOnSquareIcon width={13}/></a>
      </div>
      <label className="field-label" htmlFor="model-id">Model ID</label>
      <input id="model-id" value={modelId} onChange={event => setModelId(event.target.value)} placeholder={provider === 'OpenAI Compatible' ? "e.g. gpt-4o, llama-3.3-70b, or deepseek-chat" : "e.g. google/gemini-2.5-flash-lite"} />
      <label className="field-label" htmlFor="api-key">API key {provider === 'OpenAI Compatible' || provider === 'Ollama' ? '(optional for local endpoints)' : ''}</label>
      <input id="api-key" type="password" value={apiKey} onChange={event => setApiKey(event.target.value)} placeholder={provider === 'OpenAI Compatible' || provider === 'Ollama' ? "Optional for local endpoints, required for remote APIs" : "Paste your API key"} />
      <button className="advanced-toggle" type="button" aria-expanded={advanced} onClick={() => setAdvanced(value => !value)}>Advanced endpoint settings {advanced ? '▾' : '▸'}</button>
      {advanced && (
        <div className="advanced-panel">
          <label><input type="checkbox" checked={customEndpoint} onChange={event => setCustomEndpoint(event.target.checked)} /> Use custom endpoint</label>
          <label>Endpoint URL<input value={baseUrl} onChange={event => setBaseUrl(event.target.value)} placeholder={provider === 'OpenAI Compatible' ? "https://api.openai.com/v1 or http://localhost:1234/v1" : "https://…"} disabled={!customEndpoint} /></label>
        </div>
      )}
      {error && <div role="alert" className="provider-form-error">{error}</div>}
      <div className="provider-form-footer">
        <span>Keys are stored through your operating system&rsquo;s secure credential store.</span>
        <div>
          <button className="save-provider" type="button" onClick={() => void saveProvider()} disabled={saving}>{saving ? 'Saving…' : 'Save'}</button>
          <button className="back-button" type="button" onClick={onBack}>Back</button>
        </div>
      </div>
    </div>
  );
}

function ProviderList({items, onAdd, onDelete}: {items: ProviderInfo[]; onAdd: () => void; onDelete: (provider: string) => void}) {
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
          <div>
            <strong>{item.provider}</strong>
            <small>{item.model_id}{item.has_key ? ' · Key secured' : ' · Missing key'}</small>
          </div>
          {pending === item.provider ? (
            <div className="provider-delete-confirm">
              <span>Remove?</span>
              <button type="button" onClick={() => { void (async () => { await onDelete(item.provider); setPending(null); })(); }}>Remove</button>
              <button type="button" className="back-button" onClick={() => setPending(null)}>Cancel</button>
            </div>
          ) : (
            <button type="button" className="iconbtn" aria-label={`Remove ${item.provider}`} onClick={() => setPending(item.provider)}><TrashIcon width={16}/></button>
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
  const [search, setSearch] = useState('');
  const [savedProviders, setSavedProviders] = useState<ProviderInfo[]>([]);
  const [providerError, setProviderError] = useState('');
  const [updateStatus, setUpdateStatus] = useState('');
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const closeRef = useRef<HTMLButtonElement | null>(null);
  const openerRef = useRef<Element | null>(null);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', closeOnEscape);
    return () => document.removeEventListener('keydown', closeOnEscape);
  }, [onClose]);

  useEffect(() => {
    openerRef.current = document.activeElement;
    const close = closeRef.current;
    if (close) close.focus();
    return () => {
      if (openerRef.current && 'focus' in openerRef.current) (openerRef.current as HTMLElement).focus();
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    apiListProviders().then(items => { if (!cancelled) setSavedProviders(items); }).catch(error => { if (!cancelled) setProviderError(String(error)); });
    return () => { cancelled = true; };
  }, []);

  const visibleSections = sections.filter(section => section.toLowerCase().includes(search.toLowerCase()));
  const activeSection = visibleSections.includes(selectedSection) ? selectedSection : (visibleSections[0] ?? '');
  const noResults = search.trim() !== '' && visibleSections.length === 0;

  const updateProviders = async () => {
    try {
      setSavedProviders(await apiListProviders());
      setProviderError('');
    } catch (error) {
      setProviderError(String(error));
    }
  };

  const checkForUpdates = async () => {
    setCheckingUpdate(true);
    setUpdateStatus('Checking for updates…');
    try {
      if (!isTauri()) {
        setUpdateStatus('Updates are available only in the desktop app.');
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
        setUpdateStatus(currentVer ? `You are on the latest version (v${currentVer}).` : 'You are on the latest version.');
        return;
      }
      setUpdateStatus(`Update available: v${update.version} (current: v${update.currentVersion}). Downloading…`);
      await update.downloadAndInstall((event) => {
        if (event.event === 'Progress') {
          setUpdateStatus(`Downloading v${update.version}… ${Math.round((event.data?.chunkLength ?? 0) / 1024)} KB received`);
        }
      });
      setUpdateStatus(`Update v${update.version} installed. Relaunching…`);
      await relaunch();
    } catch (error) {
      setUpdateStatus(`Could not check for updates: ${String(error)}`);
    } finally {
      setCheckingUpdate(false);
    }
  };

  return (
    <div className="settings-modal-backdrop" onClick={onClose}>
      <div className="settings-page settings-modal" role="dialog" aria-modal="true" aria-label="Settings" onClick={e => e.stopPropagation()}>
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
        {activeSection === 'Providers' ? <>
          <section className="settings-section-heading"><h1>Providers</h1><p>Connect a service to choose which models power your conversations.</p></section>
          <div className="providers-card">
            <section className="provider-detail">
              {providerView === 'list' ? <>
                <ProviderList items={savedProviders} onAdd={() => setProviderView('form')} onDelete={async provider => {await apiDeleteProvider(provider); await updateProviders(); window.dispatchEvent(new Event('openharness:provider-saved'));}}/>
                {providerError && <span role="alert" className="provider-error">{providerError}</span>}
              </> : <ProviderForm onBack={() => setProviderView('list')} onSaved={async () => {await updateProviders(); setProviderView('list');}}/>}
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
              <button className="check-updates" onClick={checkForUpdates} disabled={checkingUpdate} title="Check for updates">{checkingUpdate ? 'Checking…' : 'Check for updates'}</button>
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
