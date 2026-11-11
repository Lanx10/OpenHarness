import {useEffect, useState} from 'react';
import {getCurrentWindow} from '@tauri-apps/api/window';
import {invoke, isTauri} from '@tauri-apps/api/core';
import {XMarkIcon, MinusIcon, Square2StackIcon, Squares2X2Icon, CommandLineIcon, GlobeAltIcon, DocumentTextIcon} from '@heroicons/react/20/solid';

type Props = {onMinimize: () => void; onToggleMaximize: () => void; onClose: () => void; updateAvailable?: boolean; onOpenSettings?: () => void; trailingContent?: import('react').ReactNode};

export function TitleBar({onMinimize, onToggleMaximize, onClose, updateAvailable, onOpenSettings, trailingContent}: Props) {
  const [maximized, setMaximized] = useState(false);
  const [toolsOpen, setToolsOpen] = useState(false);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let mounted = true;
    if (isTauri()) {
      void getCurrentWindow().isMaximized().then(value => { if (mounted) setMaximized(value); }).catch(() => {});
      void getCurrentWindow().onResized(() => {
        void getCurrentWindow().isMaximized().then(value => { if (mounted) setMaximized(value); }).catch(() => {});
      }).then(dispose => {
        if (mounted) unlisten = dispose;
        else dispose();
      }).catch(() => {});
    }
    return () => { mounted = false; unlisten?.(); };
  }, []);

  const handleMinimize = async () => {
    try {
      if (isTauri()) {
        await getCurrentWindow().minimize();
        return;
      }
    } catch {}
    try {
      if (isTauri()) {
        await invoke('app_minimize');
        return;
      }
    } catch {}
    onMinimize();
  };

  const handleToggleMaximize = async () => {
    try {
      if (isTauri()) {
        await getCurrentWindow().toggleMaximize();
        const isMax = await getCurrentWindow().isMaximized();
        setMaximized(isMax);
        return;
      }
    } catch {}
    try {
      if (isTauri()) {
        await invoke('app_toggle_maximize');
        setMaximized(prev => !prev);
        return;
      }
    } catch {}
    onToggleMaximize();
  };

  const handleClose = async () => {
    try {
      if (isTauri()) {
        await getCurrentWindow().close();
        return;
      }
    } catch {}
    try {
      if (isTauri()) {
        await invoke('app_close');
        return;
      }
    } catch {}
    onClose();
  };

  return (
    <header className="titlebar" data-od-id="title-bar">
      <div className="breadcrumb" aria-live="polite">
        <span className="breadcrumb-project" id="breadcrumb-project">OpenHarness</span>
      </div>

      <div className="drag-region" aria-hidden="true" data-tauri-drag-region />

      <div className="window-tools-menu">
        <button className="window-tools-trigger" type="button" aria-label="Open workspace tools" title="Workspace tools" aria-expanded={toolsOpen} aria-haspopup="menu" onClick={() => setToolsOpen(open => !open)}>
          <Squares2X2Icon width={18} height={18} aria-hidden="true" />
        </button>
        {toolsOpen && <>
          <button className="window-tools-dismiss" type="button" aria-label="Close workspace tools menu" onClick={() => setToolsOpen(false)} />
          <div className="window-tools-popover" role="menu" aria-label="Workspace tools">
            <button type="button" role="menuitem" onClick={() => { setToolsOpen(false); window.dispatchEvent(new Event('openharness:quick-cmd')); }}>
              <CommandLineIcon width={17} height={17} aria-hidden="true" /><span>Terminal</span>
            </button>
            <button type="button" role="menuitem" onClick={() => { setToolsOpen(false); window.dispatchEvent(new CustomEvent('openharness:quick-browser')); }}>
              <GlobeAltIcon width={17} height={17} aria-hidden="true" /><span>Browser</span>
            </button>
            <button type="button" role="menuitem" onClick={() => { setToolsOpen(false); window.dispatchEvent(new CustomEvent('openharness:quick-files')); }}>
              <DocumentTextIcon width={17} height={17} aria-hidden="true" /><span>Files</span>
            </button>
            <button type="button" role="menuitem" onClick={() => { setToolsOpen(false); window.dispatchEvent(new CustomEvent('openharness:quick-git')); }}>
              <svg viewBox="0 0 16 16" width={17} height={17} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
                <circle cx="4" cy="4" r="2" /><circle cx="4" cy="12" r="2" /><circle cx="12" cy="7" r="2" /><path d="M4 6v4M4 7.5a4.5 4.5 0 0 1 4.5-1.5H10" />
              </svg><span>Git diff</span>
            </button>
          </div>
        </>}
      </div>

      {trailingContent}
      <div className="window-controls" aria-label="Window controls">
        {updateAvailable && (
          <button
            className="window-control update-available-btn"
            type="button"
            aria-label="App update available"
            title="Update available — click to view"
            onClick={() => {
              if (onOpenSettings) onOpenSettings();
              else window.dispatchEvent(new CustomEvent('openharness:open-settings', {detail: {section: 'Update'}}));
            }}
            style={{
              color: '#38bdf8',
              display: 'inline-flex',
              alignItems: 'center',
              justifyContent: 'center',
              marginRight: 4,
              padding: '0 6px',
              width: 'auto',
              fontSize: '11px',
              fontWeight: 600,
              gap: 4
            }}
          >
            <span style={{width: 6, height: 6, borderRadius: '50%', backgroundColor: '#38bdf8'}} />
            Update
          </button>
        )}
        <button className="window-control" type="button" aria-label="Minimize window" title="Minimize" onClick={handleMinimize}>
          <MinusIcon width={13} height={13} aria-hidden="true" />
        </button>
        <button className="window-control" type="button" aria-label={maximized ? 'Restore window' : 'Maximize window'} title={maximized ? 'Restore' : 'Maximize'} onClick={handleToggleMaximize}>
          {maximized ? (
            <Square2StackIcon width={12} height={12} aria-hidden="true" />
          ) : (
            <svg viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.2" aria-hidden="true"><rect x="3.5" y="3.5" width="9" height="9"/></svg>
          )}
        </button>
        <button className="window-control close" type="button" aria-label="Close window" title="Close" onClick={handleClose}>
          <XMarkIcon width={13} height={13} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
