import {useEffect, useState} from 'react';
import {invoke, isTauri} from '@tauri-apps/api/core';
import {TitleBar} from './Chrome';
import {Canvas} from './Canvas';

export function ChatWindow() {
  const [updateAvailable, setUpdateAvailable] = useState(false);

  useEffect(() => {
    const updateCheck = (event: Event) => {
      setUpdateAvailable(Boolean((event as CustomEvent<boolean>).detail));
    };
    window.addEventListener('openharness:update-availability', updateCheck);
    return () => window.removeEventListener('openharness:update-availability', updateCheck);
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
