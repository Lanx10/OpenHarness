import {useEffect, useState} from 'react';
import {isTauri} from '@tauri-apps/api/core';
import {
  ArrowPathIcon, ArrowTopRightOnSquareIcon,
  ChevronRightIcon, InformationCircleIcon, MagnifyingGlassIcon,
  QuestionMarkCircleIcon, ServerIcon, XMarkIcon, TrashIcon,
} from '@heroicons/react/20/solid';
import {apiDeleteProvider, apiListProviders, apiSaveProvider, type ProviderInfo} from '../../services/tauriBridge';

type SettingsPageProps = {onClose: () => void; initialSection?: string};
type Provider = {name: string; icon: string; tone: string};

const providers: Provider[] = [];

const sections = ['Model Providers', 'Update'];
const providerOptions = ['OpenRouter', 'Google AI Studio', 'Ollama'];

function ProviderForm({onBack, onSaved}: {onBack: () => void; onSaved: (provider: ProviderInfo) => void}) {
  const [provider, setProvider] = useState(providerOptions[0]);
  const [apiKey, setApiKey] = useState('');
  const [modelId, setModelId] = useState('');
  const [baseUrl, setBaseUrl] = useState('');
  const [customEndpoint, setCustomEndpoint] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState('');

  const saveProvider = async () => {
    if (!apiKey.trim() || !modelId.trim()) {
      setError('Enter an API key and Model ID before saving.');
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
      const saved = await apiSaveProvider(provider, modelId.trim(), customEndpoint ? baseUrl.trim() : null, customEndpoint, apiKey);
      setApiKey('');
      onSaved(saved);
    } catch (err) {
      setError(String(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="provider-form">
      <div className="provider-form-header"><ServerIcon width={19}/><strong>API providers</strong><QuestionMarkCircleIcon width={15} className="help-icon"/></div>
      <label className="field-label" htmlFor="provider-select">Provider</label>
      <div className="provider-select-row">
        <select id="provider-select" value={provider} onChange={event => {setProvider(event.target.value); setError('');}}>
          {providerOptions.map(option => <option key={option}>{option}</option>)}
        </select>
        <a className="provider-setup" href="https://openrouter.ai/docs/quickstart" target="_blank" rel="noreferrer">Provider setup <ArrowTopRightOnSquareIcon width={13}/></a>
      </div>

      <label className="field-label" htmlFor="provider-key">API key <QuestionMarkCircleIcon width={14} className="help-icon"/></label>
      <input id="provider-key" type="password" value={apiKey} onChange={event => {setApiKey(event.target.value); setError('');}} placeholder="Paste your API key" autoComplete="new-password" spellCheck={false}/>

      <label className="field-label" htmlFor="provider-model">Model ID <QuestionMarkCircleIcon width={14} className="help-icon"/></label>
      <input className="wide-input" id="provider-model" value={modelId} onChange={event => {setModelId(event.target.value); setError('');}} placeholder="provider/model-name" autoComplete="off"/>

      <button className="advanced-toggle" type="button" aria-expanded={advanced} onClick={() => setAdvanced(value => !value)}>
        <ChevronRightIcon width={14} className={advanced ? 'rotate' : ''}/> Advanced settings
      </button>
      {advanced && <div className="advanced-panel"><label>Full API endpoint URL<input className="wide-input" value={baseUrl} placeholder="https://openrouter.ai/api/v1/chat/completions" onChange={(event) => {setBaseUrl(event.target.value); setError('');}}/></label><label className="check-row"><input type="checkbox" checked={customEndpoint} onChange={(event) => setCustomEndpoint(event.target.checked)}/> Send requests through a custom endpoint</label></div>}

      <div className="provider-form-footer">
        <span role={error ? 'alert' : 'status'}>{error || 'API key saved in the OS credential store.'}</span>
        <div><button className="save-provider" type="button" onClick={() => void saveProvider()} disabled={saving}>{saving ? 'Saving…' : 'Save provider'}</button><button className="back-button" type="button" onClick={onBack}>Back</button></div>
      </div>
    </div>
  );
}

function ProviderList({items, onAdd, onDelete}: {items: ProviderInfo[]; onAdd: () => void; onDelete: (provider: string) => void}) {
  return (
    <div className="provider-list">
      {items.length === 0 ? <h3>No API providers yet <QuestionMarkCircleIcon width={14} className="help-icon"/></h3> : items.map((item) => (
        <div className="saved-provider" key={item.provider}>
          <div><strong>{item.provider}</strong><small>{item.model_id}{item.has_key ? ' · Key secured' : ' · Missing key'}</small></div>
          <button type="button" className="iconbtn" aria-label={`Remove ${item.provider}`} onClick={() => onDelete(item.provider)}><TrashIcon width={16}/></button>
        </div>
      ))}
      <button className="add-provider" onClick={onAdd}>Add provider</button>
    </div>
  );
}

export function SettingsPage({onClose, initialSection = 'Model Providers'}: SettingsPageProps) {
  const [selectedSection, setSelectedSection] = useState(initialSection);
  const [providerView, setProviderView] = useState<'list' | 'form'>('list');
  const [search, setSearch] = useState('');
  const [selectedSource, setSelectedSource] = useState('API providers');
  const [savedProviders, setSavedProviders] = useState<ProviderInfo[]>([]);
  const [providerError, setProviderError] = useState('');
  const [updateStatus, setUpdateStatus] = useState('');
  const [checkingUpdate, setCheckingUpdate] = useState(false);

  useEffect(() => {
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose();
    };
    document.addEventListener('keydown', closeOnEscape);
    return () => document.removeEventListener('keydown', closeOnEscape);
  }, [onClose]);

  useEffect(() => {
    let cancelled = false;
    apiListProviders().then(items => { if (!cancelled) setSavedProviders(items); }).catch(error => { if (!cancelled) setProviderError(String(error)); });
    return () => { cancelled = true; };
  }, []);

  const visibleSections = sections.filter(section => section.toLowerCase().includes(search.toLowerCase()));
  const activeSection = visibleSections.includes(selectedSection) ? selectedSection : (visibleSections[0] ?? 'Model Providers');
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
        setUpdateStatus(`You are on the latest version.`);
        return;
      }
      setUpdateStatus(`Updating to v${update.version}…`);
      await update.downloadAndInstall((event) => {
        if (event.event === 'Progress') {
          setUpdateStatus(`Downloading… ${Math.round((event.data?.chunkLength ?? 0) / 1024)} KB received`);
        }
      });
      await relaunch();
    } catch (error) {
      setUpdateStatus(`Could not check for updates: ${String(error)}`);
    } finally {
      setCheckingUpdate(false);
    }
  };

  return (
    <div className="settings-page" role="dialog" aria-modal="true" aria-label="OpenHarness settings">
      <aside className="settings-rail">
        <button type="button" className="settings-home" aria-label="Home" title="Home" onClick={() => {onClose(); window.dispatchEvent(new Event('openharness:home'));}}><svg viewBox="0 0 20 20" width="17" height="17" fill="currentColor" aria-hidden="true"><path d="M10 2.8 2.5 9v8a.8.8 0 0 0 .8.8h4.3v-5.2h4.8v5.2h4.3a.8.8 0 0 0 .8-.8V9L10 2.8Z"/></svg></button>
        <button type="button" className="settings-gear active" aria-label="Settings" title="Settings" onClick={onClose}><svg viewBox="0 0 20 20" width="18" height="18" fill="currentColor" aria-hidden="true"><path fillRule="evenodd" d="M8.34 1.804A1 1 0 0 1 9.32 1h1.36a1 1 0 0 1 .98.804l.295 1.473c.497.144.971.342 1.409.582l1.304-.75a1 1 0 0 1 1.225.216l.962.962a1 1 0 0 1 .216 1.225l-.75 1.304c.24.438.438.912.582 1.409l1.473.295a1 1 0 0 1 .804.98v1.36a1 1 0 0 1-.804.98l-1.473.295a6.95 6.95 0 0 1-.582 1.409l.75 1.304a1 1 0 0 1-.216 1.225l-.962.962a1 1 0 0 1-1.225.216l-1.304-.75a6.97 6.97 0 0 1-1.409.582l-.295 1.473a1 1 0 0 1-.98.804H9.32a1 1 0 0 1-.98-.804l-.295-1.473a6.97 6.97 0 0 1-1.409-.582l-1.304.75a1 1 0 0 1-1.225-.216l-.962-.962a1 1 0 0 1-.216-1.225l.75-1.304a6.95 6.95 0 0 1-.582-1.409l-1.473-.295A1 1 0 0 1 1 10.68V9.32a1 1 0 0 1 .804-.98l1.473-.295c.144-.497.342-.971.582-1.409l-.75-1.304a1 1 0 0 1 .216-1.225l.962-.962a1 1 0 0 1 1.225-.216l1.304.75c.438-.24.912-.438 1.409-.582l.295-1.473ZM10 13a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z" clipRule="evenodd"/></svg></button>
      </aside>
      <aside className="settings-nav">
        <div className="settings-brand"><span className="settings-brand-mark" aria-hidden="true">OH</span><span className="settings-brand-name">OpenHarness</span></div>
        <div className="settings-nav-header"><h2>Settings</h2><button className="settings-close" type="button" aria-label="Close settings" title="Close settings" onClick={onClose}><XMarkIcon width={16}/></button></div>
        <label className="settings-search"><MagnifyingGlassIcon width={15}/><input value={search} onChange={event => setSearch(event.target.value)} placeholder="Search settings" aria-label="Search settings"/></label>
        <nav>{ visibleSections.map(section => (
          <button key={section} className={activeSection === section ? 'settings-nav-item active' : 'settings-nav-item'} onClick={() => {setSelectedSection(section); setProviderView('list');}}>{section}</button>
        ))}</nav>
      </aside>
      <main className="settings-content">
        {activeSection === 'Model Providers' ? <>
          <h1>Model Providers</h1>
          <div className="providers-card">
            <div className="provider-sources">
              {providers.map(item => <button className="provider-source disabled-source" key={item.name} type="button" disabled><span className={`source-logo ${item.tone}`}>{item.icon}</span><span className="source-label"><strong>{item.name}</strong><small><i/> Not connected</small></span></button>)}
              <button className={`provider-source api-source ${selectedSource === 'API providers' ? 'selected' : ''}`} onClick={() => setSelectedSource('API providers')}><ServerIcon width={17}/><span className="source-label"><strong>API providers</strong><small><i/> {savedProviders.length ? `${savedProviders.length} configured` : 'None added'}</small></span><ChevronRightIcon width={15}/></button>
            </div>
            <section className="provider-detail">
              {providerView === 'list' ? <><div className="detail-title"><ServerIcon width={18}/><strong>API providers</strong><QuestionMarkCircleIcon width={14} className="help-icon"/></div><ProviderList items={savedProviders} onAdd={() => setProviderView('form')} onDelete={async provider => {await apiDeleteProvider(provider); await updateProviders();}}/><span role="alert">{providerError}</span></> : <ProviderForm onBack={() => setProviderView('list')} onSaved={async () => {await updateProviders(); setProviderView('list');}}/>}
            </section>
          </div>
        </> : <>
          <div className="settings-section-heading"><h1>OpenHarness updates</h1><span>Desktop update preferences</span></div>
          <section className="update-panel">
            <div className="updates-icon"><ArrowPathIcon width={20}/></div>
            <h2>App updates</h2>
            <p>Update availability is provided by the desktop application.</p>
            <div className="update-row"><span className="update-status">{updateStatus || 'Check for the latest version of OpenHarness.'}</span><button className="check-updates" onClick={checkForUpdates} disabled={checkingUpdate} title="Check for updates">{checkingUpdate ? 'Checking…' : 'Check for updates'}</button></div>
          </section>
          <section className="update-panel"><InformationCircleIcon width={18} className="help-icon"/><p>Settings sync and provider integrations will be available in future updates.</p></section>
        </>}
      </main>
    </div>
  );
}
