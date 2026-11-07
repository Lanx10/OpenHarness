import {useEffect, useState} from 'react';
import {getCurrentWindow} from '@tauri-apps/api/window';
import {invoke, isTauri} from '@tauri-apps/api/core';
import {XMarkIcon, PlusIcon, MinusIcon, Square2StackIcon} from '@heroicons/react/20/solid';

type Props = {onMinimize: () => void; onToggleMaximize: () => void; onClose: () => void; updateAvailable?: boolean; onOpenSettings?: () => void};

export function TitleBar({onMinimize, onToggleMaximize, onClose, updateAvailable, onOpenSettings}: Props) {
  const [maximized, setMaximized] = useState(false);

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
        <span className="breadcrumb-separator" aria-hidden="true">/</span>
        <span className="breadcrumb-chat" id="breadcrumb-chat">New chat</span>
        <button className="new-tab" id="new-tab" type="button" aria-label="New chat" title="New chat" onClick={() => window.dispatchEvent(new Event('openharness:new-chat'))}>
          <PlusIcon width={13} height={13} aria-hidden="true" />
        </button>
      </div>

      <div className="drag-region" aria-hidden="true" data-tauri-drag-region />

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
