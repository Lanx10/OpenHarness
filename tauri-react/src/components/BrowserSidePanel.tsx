import {useEffect, useRef, useState, type FormEvent, type PointerEvent as ReactPointerEvent} from 'react';
import {
  GlobeAltIcon,
  ArrowPathIcon,
  ArrowLeftIcon,
  ArrowRightIcon,
  ArrowTopRightOnSquareIcon,
  XMarkIcon,
  HomeIcon,
  ComputerDesktopIcon,
  DevicePhoneMobileIcon,
  DeviceTabletIcon,
} from '@heroicons/react/20/solid';

import {normalizeBrowserAddress, DEFAULT_BROWSER_URL} from '../utils/browserAddress';
export {normalizeBrowserAddress};
const DEFAULT_URL = DEFAULT_BROWSER_URL;
const STORAGE_KEY = 'openharness:browser-panel-url';
const PRESETS = [
  {label: ':3000', url: 'http://localhost:3000'},
  {label: ':5173', url: 'http://localhost:5173'},
  {label: ':8080', url: 'http://localhost:8080'},
  {label: ':1420', url: 'http://localhost:1420'},
  {label: 'Docs', url: 'https://docs.rs'},
  {label: 'MDN', url: 'https://developer.mozilla.org'},
];

type ViewportMode = 'fill' | 'tablet' | 'mobile';

export type BrowserSidePanelProps = {
  width: number;
  setWidth: (width: number) => void;
  onClose: () => void;
};

export function BrowserSidePanel({width, setWidth, onClose}: BrowserSidePanelProps) {
  const initialUrl = useRef(() => {
    try { return normalizeBrowserAddress(localStorage.getItem(STORAGE_KEY) || DEFAULT_URL); }
    catch { return DEFAULT_URL; }
  }).current();
  const [urlInput, setUrlInput] = useState(initialUrl);
  const [currentUrl, setCurrentUrl] = useState(initialUrl);
  const [isLoading, setIsLoading] = useState(true);
  const [history, setHistory] = useState([initialUrl]);
  const [historyIndex, setHistoryIndex] = useState(0);
  const [viewportMode, setViewportMode] = useState<ViewportMode>('fill');
  const [loadError, setLoadError] = useState(false);
  const [addressError, setAddressError] = useState('');
  const iframeRef = useRef<HTMLIFrameElement | null>(null);
  const loadTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    try { localStorage.setItem(STORAGE_KEY, currentUrl); } catch {}
  }, [currentUrl]);

  const beginLoad = () => {
    if (loadTimer.current) clearTimeout(loadTimer.current);
    loadTimer.current = setTimeout(() => {
      setIsLoading(false);
      setLoadError(true);
    }, 20000);
  };

  useEffect(() => {
    beginLoad();
    return () => { if (loadTimer.current) clearTimeout(loadTimer.current); };
  }, []);

  const navigateTo = (address: string) => {
    let target: string;
    try { target = normalizeBrowserAddress(address); }
    catch (error) {
      setAddressError(error instanceof Error ? error.message : 'Invalid web address.');
      setIsLoading(false);
      setLoadError(false);
      return;
    }
    setAddressError('');
    if (target !== currentUrl) {
      setHistory(previous => {
        const next = previous.slice(0, historyIndex + 1);
        if (next[next.length - 1] !== target) next.push(target);
        setHistoryIndex(next.length - 1);
        return next;
      });
      setCurrentUrl(target);
    }
    setUrlInput(target);
    setIsLoading(true);
    setLoadError(false);
    beginLoad();
  };

  const handleSubmit = (event: FormEvent) => {
    event.preventDefault();
    navigateTo(urlInput);
  };

  const navigateHistory = (nextIndex: number) => {
    const target = history[nextIndex];
    if (!target) return;
    setHistoryIndex(nextIndex);
    setCurrentUrl(target);
    setUrlInput(target);
    setIsLoading(true);
    setLoadError(false);
    setAddressError('');
    beginLoad();
  };

  const handleReload = () => {
    if (!iframeRef.current) return;
    setIsLoading(true);
    setLoadError(false);
    beginLoad();
    try {
      iframeRef.current.contentWindow?.location.reload();
    } catch {
      // Cross-origin reload may be blocked; resetting src remains browser-controlled.
      iframeRef.current.src = currentUrl;
    }
  };

  const openExternally = () => {
    const popup = window.open(currentUrl, '_blank', 'noopener,noreferrer');
    if (!popup) setLoadError(true);
  };

  const resize = (event: ReactPointerEvent<HTMLDivElement>) => {
    const startX = event.clientX;
    const startWidth = width;
    const captured = event.currentTarget;
    const move = (pointer: PointerEvent) => setWidth(Math.min(1100, Math.max(340, startWidth + startX - pointer.clientX)));
    const stop = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      window.removeEventListener('pointercancel', stop);
      window.removeEventListener('lostpointercapture', stop);
      if (captured.hasPointerCapture?.(event.pointerId)) {
        try { captured.releasePointerCapture(event.pointerId); } catch {}
      }
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop, {once: true});
    window.addEventListener('pointercancel', stop, {once: true});
    window.addEventListener('lostpointercapture', stop, {once: true});
    captured.setPointerCapture(event.pointerId);
  };

  const maxWidth = viewportMode === 'mobile' ? 375 : viewportMode === 'tablet' ? 768 : undefined;

  return (
    <aside className="browser-side-panel" aria-label="Browser panel" style={{width: `${width}px`}}>
      <div className="browser-resizer" onPointerDown={resize} role="separator" aria-orientation="vertical" aria-label="Resize browser panel" />
      <div className="browser-panel-header">
        <div className="browser-panel-title"><GlobeAltIcon width={15} height={15} aria-hidden="true" /><span>Browser</span></div>
        <div className="browser-header-actions">
          <div className="browser-viewport-toggles" role="group" aria-label="Viewport size">
            <button type="button" className={`browser-tool-btn ${viewportMode === 'fill' ? 'active' : ''}`} title="Full width" aria-label="Full width view" onClick={() => setViewportMode('fill')}><ComputerDesktopIcon width={14} height={14} /></button>
            <button type="button" className={`browser-tool-btn ${viewportMode === 'tablet' ? 'active' : ''}`} title="Tablet (768px)" aria-label="Tablet view" onClick={() => setViewportMode('tablet')}><DeviceTabletIcon width={14} height={14} /></button>
            <button type="button" className={`browser-tool-btn ${viewportMode === 'mobile' ? 'active' : ''}`} title="Mobile (375px)" aria-label="Mobile view" onClick={() => setViewportMode('mobile')}><DevicePhoneMobileIcon width={14} height={14} /></button>
          </div>
          <button type="button" className="browser-tool-btn" title="Open in default system browser" aria-label="Open in external browser" onClick={openExternally}><ArrowTopRightOnSquareIcon width={14} height={14} /></button>
          <button type="button" className="workspace-tool-close" aria-label="Close browser panel" title="Close browser" onClick={onClose}><XMarkIcon width={16} height={16} /></button>
        </div>
      </div>

      <div className="browser-controls-row">
        <div className="browser-nav-buttons">
          <button type="button" className="browser-nav-btn" disabled={historyIndex <= 0} onClick={() => navigateHistory(historyIndex - 1)} aria-label="Back" title="Go back"><ArrowLeftIcon width={13} height={13} /></button>
          <button type="button" className="browser-nav-btn" disabled={historyIndex >= history.length - 1} onClick={() => navigateHistory(historyIndex + 1)} aria-label="Forward" title="Go forward"><ArrowRightIcon width={13} height={13} /></button>
          <button type="button" className={`browser-nav-btn ${isLoading ? 'spinning' : ''}`} onClick={handleReload} aria-label="Reload" title="Reload page"><ArrowPathIcon width={13} height={13} /></button>
          <button type="button" className="browser-nav-btn" onClick={() => navigateTo(DEFAULT_URL)} aria-label="Home" title="Open local preview default"><HomeIcon width={13} height={13} /></button>
        </div>
        <form className="browser-url-form" onSubmit={handleSubmit}>
          <input className="browser-url-input" value={urlInput} onChange={event => setUrlInput(event.target.value)} placeholder="Search or enter URL (e.g. localhost:5173)" spellCheck={false} autoComplete="off" aria-label="Web address" />
          <button type="submit" className="browser-url-submit" aria-label="Navigate">Go</button>
        </form>
      </div>
      {(addressError || loadError) && <div className="browser-status-warning" role="alert">{addressError || 'Site blocked or unavailable. Try opening it externally.'}</div>}

      <div className="browser-presets-bar">
        <span className="browser-presets-label">Quick:</span>
        <div className="browser-presets-list">
          {PRESETS.map(preset => <button key={preset.url} type="button" className={`browser-preset-chip ${currentUrl === preset.url ? 'active' : ''}`} onClick={() => navigateTo(preset.url)} title={preset.url}>{preset.label}</button>)}
        </div>
      </div>

      <div className="browser-viewport-container">
        {isLoading && <div className="browser-loading-bar" aria-hidden="true" />}
        <div className={`browser-viewport-wrapper ${maxWidth ? 'is-device-frame' : ''}`} style={{maxWidth: maxWidth ? `${maxWidth}px` : '100%'}}>
          <iframe
            ref={iframeRef}
            src={currentUrl}
            title="Browser preview"
            className="browser-iframe"
            sandbox="allow-scripts allow-forms"
            referrerPolicy="strict-origin-when-cross-origin"
            onLoad={() => { if (loadTimer.current) clearTimeout(loadTimer.current); setIsLoading(false); setLoadError(false); }}
            onError={() => { if (loadTimer.current) clearTimeout(loadTimer.current); setIsLoading(false); setLoadError(true); }}
          />
        </div>
      </div>

      <div className="browser-status-bar">
        <div className="browser-status-text" title={currentUrl}>
          <span className={`browser-status-dot ${currentUrl.startsWith('https:') || currentUrl.startsWith('http://localhost') ? 'secure' : ''}`} />
          <span className="browser-status-url">{currentUrl}</span>
        </div>
        {loadError ? <span className="browser-status-warning">Site blocked or unavailable. <button type="button" onClick={openExternally}>Open externally</button></span> : <button type="button" className="browser-status-open-btn" onClick={openExternally}>Open externally ↗</button>}
      </div>
    </aside>
  );
}

export const __browserDefaults = {DEFAULT_URL};
// ponytail: iframe navigation cannot bypass sites that deny embedding; open externally or upgrade to a Tauri WebViewWindow for those sites.
