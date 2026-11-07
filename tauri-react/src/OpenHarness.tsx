import {useEffect, useState} from 'react';
import {Rail} from './components/Rail';
import {Drawer} from './components/Drawer';
import {ChatWindow} from './components/ChatWindow';
import {SettingsPage} from './components/Settings/SettingsPage';

export function OpenHarness() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [initialSection, setInitialSection] = useState('Model Providers');

  useEffect(() => {
    const openSettings = (event?: Event) => {
      const section = (event as CustomEvent<{section?: string}>)?.detail?.section;
      if (section) setInitialSection(section);
      setSettingsOpen(true);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setSettingsOpen(false);
    };
    window.addEventListener('openharness:open-settings', openSettings);
    const checkUpdates = async () => {
      try {
        const {check} = await import('@tauri-apps/plugin-updater');
      const update = await check();
      window.dispatchEvent(new CustomEvent('openharness:update-availability', {detail: !!update}));
    } catch {}
    };
    void checkUpdates();
    window.addEventListener('keydown', closeOnEscape);
    return () => {
      window.removeEventListener('openharness:open-settings', openSettings);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, []);

  return (
    <div className="app">
      <Rail onGearClick={() => setSettingsOpen(value => !value)} gearActive={settingsOpen}/>
      <Drawer />
      <ChatWindow />
      {settingsOpen && <SettingsPage onClose={() => setSettingsOpen(false)} initialSection={initialSection}/>}
    </div>
  );
}
