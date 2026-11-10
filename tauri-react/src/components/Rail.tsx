import {useEffect, useState} from 'react';
import {HomeIcon, Cog8ToothIcon} from '@heroicons/react/20/solid';

type Props = {
  onGearClick: () => void;
  gearActive: boolean;
};

export function Rail({onGearClick, gearActive}: Props) {
  const [hasUpdate, setHasUpdate] = useState(false);

  useEffect(() => {
    const handleUpdate = (event: Event) => {
      setHasUpdate(Boolean((event as CustomEvent<boolean>).detail));
    };
    window.addEventListener('openharness:update-availability', handleUpdate);
    return () => window.removeEventListener('openharness:update-availability', handleUpdate);
  }, []);

  const homeActive = !gearActive;

  return (
    <nav className="rail" aria-label="Primary navigation" data-od-id="left-rail">
      <button
        className={`rail-button ${homeActive ? 'active' : ''}`}
        type="button"
        aria-label="Home"
        title="Home"
        data-od-id="home-button"
        aria-pressed={homeActive}
        onClick={() => window.dispatchEvent(new Event('openharness:home'))}
      >
        <HomeIcon width={18} height={18} aria-hidden="true" />
      </button>

      <button
        className={`rail-button rail-bottom ${gearActive ? 'active' : ''}`}
        type="button"
        aria-label="Settings"
        title={hasUpdate ? 'Settings (Update available)' : 'Settings'}
        data-od-id="settings-button"
        aria-pressed={gearActive}
        onClick={onGearClick}
      >
        <Cog8ToothIcon width={18} height={18} aria-hidden="true" />
        {hasUpdate && <span className="rail-badge-dot" aria-hidden="true" />}
      </button>
    </nav>
  );
}
