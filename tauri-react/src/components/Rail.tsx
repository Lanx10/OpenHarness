import {HomeIcon, Cog8ToothIcon} from '@heroicons/react/20/solid';

type Props = {
  onGearClick: () => void;
  gearActive: boolean;
};

export function Rail({onGearClick, gearActive}: Props) {
  return (
    <nav className="rail" aria-label="Primary navigation" data-od-id="left-rail">
      <button
        className="rail-button active"
        type="button"
        aria-label="Home"
        title="Home"
        data-od-id="home-button"
        onClick={() => window.dispatchEvent(new Event('openharness:home'))}
      >
        <HomeIcon width={17} height={17} aria-hidden="true" />
      </button>

      <button
        className={`rail-button rail-bottom ${gearActive ? 'active' : ''}`}
        type="button"
        aria-label="Settings"
        title="Settings"
        data-od-id="settings-button"
        aria-pressed={gearActive}
        onClick={onGearClick}
      >
        <Cog8ToothIcon width={18} height={18} aria-hidden="true" />
      </button>
    </nav>
  );
}
