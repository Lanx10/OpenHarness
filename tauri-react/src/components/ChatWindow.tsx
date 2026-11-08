import {useEffect, useState} from 'react';
import {invoke, isTauri} from '@tauri-apps/api/core';
import {TitleBar} from './Chrome';
import {Canvas} from './Canvas';

export function ChatWindow() {
  const [updateAvailable, setUpdateAvailable] = useState(false);

  useEffect(() => {
    const updateProjectTab = (event: Event) => {
      const project = (event as CustomEvent<{name?: string}>).detail;
      const projElem = document.getElementById('breadcrumb-project');
      if (projElem) projElem.textContent = project?.name || 'OpenHarness';
    };
    const updateCheck = (event: Event) => {
      setUpdateAvailable(Boolean((event as CustomEvent<boolean>).detail));
    };
    window.addEventListener('openharness:select-project', updateProjectTab);
    window.addEventListener('openharness:update-availability', updateCheck);
    return () => {
      window.removeEventListener('openharness:select-project', updateProjectTab);
      window.removeEventListener('openharness:update-availability', updateCheck);
    };
  }, []);

  const windowControls = {
    onMinimize: () => { if (isTauri()) void invoke('app_minimize'); },
    onToggleMaximize: () => { if (isTauri()) void invoke('app_toggle_maximize'); },
    onClose: () => { if (isTauri()) void invoke('app_close'); },
  };

  return (
    <main className="workspace" data-od-id="chat-canvas">
      <TitleBar
        {...windowControls}
        updateAvailable={updateAvailable}
        onOpenSettings={() => window.dispatchEvent(new CustomEvent('openharness:open-settings', {detail: {section: 'Update'}}))}
      />
      <Canvas />
    </main>
  );
}
