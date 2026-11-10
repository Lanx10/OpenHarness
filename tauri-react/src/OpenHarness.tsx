import {useEffect, useRef, useState} from 'react';
import {ArrowPathIcon, MagnifyingGlassIcon, SparklesIcon, XMarkIcon} from '@heroicons/react/20/solid';
import {apiListSkills, type SkillInfoLike} from './services/tauriBridge';
import {Rail} from './components/Rail';
import {Drawer} from './components/Drawer';
import {ChatWindow} from './components/ChatWindow';
import {SettingsPage} from './components/Settings/SettingsPage';

type SkillInfo = SkillInfoLike;

function SkillsPage({skills, loading, error, selected, search, onSearch, onToggle, onRefresh, onClose}: {
  skills: SkillInfo[]; loading: boolean; error: string | null; selected: Set<string>; search: string;
  onSearch: (value: string) => void; onToggle: (name: string) => void; onRefresh: () => void; onClose: () => void;
}) {
  const filtered = skills.filter(skill => `${skill.name} ${skill.description}`.toLowerCase().includes(search.toLowerCase()));
  return <section className="skills-page" aria-labelledby="skills-title">
    <header className="skills-header">
      <div><span className="skills-eyebrow">GLOBAL AGENT LIBRARY</span><h1 id="skills-title">Skills</h1><p>Choose skills from your user-level agent directory to load into the coding agent’s instructions.</p></div>
      <div className="skills-header-actions"><button type="button" className="skills-refresh" onClick={onRefresh} aria-label="Refresh skills" title="Refresh skills"><ArrowPathIcon width={16} height={16} aria-hidden="true" /></button><button type="button" className="skills-refresh" onClick={onClose} aria-label="Close skills and return to chat" title="Back to chat"><XMarkIcon width={16} height={16} aria-hidden="true" /></button></div>
    </header>
    <label className="skills-search"><MagnifyingGlassIcon width={16} height={16} aria-hidden="true"/><span className="visually-hidden">Search skills</span><input value={search} onChange={event => onSearch(event.target.value)} placeholder="Search skills…" /></label>
    {error && <p className="skills-error" role="alert">{error}</p>}
    <div className="skills-selection"><span>{selected.size} selected</span><span>Selected skills are included in new requests.</span></div>
    {loading ? <div className="skills-state" role="status">Loading skills…</div> : filtered.length ? <ul className="skills-list">{filtered.map(skill => <li key={skill.name}>
      <label className={`skill-row ${selected.has(skill.name) ? 'selected' : ''}`}>
        <input type="checkbox" checked={selected.has(skill.name)} disabled={!selected.has(skill.name) && selected.size >= 12} onChange={() => onToggle(skill.name)} />
        <span className="skill-row-icon"><SparklesIcon width={15} height={15} aria-hidden="true" /></span>
        <span className="skill-row-copy"><strong>{skill.name}</strong><span>{skill.description || 'No description provided.'}</span><small>{skill.path}</small></span>
      </label>
    </li>)}</ul> : <div className="skills-state">{search ? 'No skills match this search.' : 'No global skills found under your user-level agent directories (.agents/skills, .claude/skills, or .kilo/skills).'}</div>}
  </section>;
}

export function OpenHarness() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [skillsOpen, setSkillsOpen] = useState(false);
  const [skills, setSkills] = useState<SkillInfo[]>([]);
  const [loadingSkills, setLoadingSkills] = useState(false);
  const [skillsError, setSkillsError] = useState<string | null>(null);
  const [selectedSkills, setSelectedSkills] = useState<string[]>(() => {
    try {
      const parsed: unknown = JSON.parse(localStorage.getItem('openharness:global-skills') || '[]');
      return Array.isArray(parsed) ? parsed.filter((name): name is string => typeof name === 'string') : [];
    } catch { return []; }
  });
  const [skillSearch, setSkillSearch] = useState('');
  const visibleSelectedSkills = new Set(selectedSkills);
  const selectedSkillsRef = useRef(selectedSkills);
  const [initialSection, setInitialSection] = useState('Providers');
  selectedSkillsRef.current = selectedSkills;

  const refreshSkills = async () => {
    setLoadingSkills(true);
    setSkillsError(null);
    try {
      const items = await apiListSkills();
      setSkills(items);
    } catch (error) {
      setSkillsError(String(error));
    } finally {
      setLoadingSkills(false);
    }
  };
  const toggleSkill = (name: string) => {
    const next = new Set(selectedSkillsRef.current);
    if (next.has(name)) next.delete(name); else next.add(name);
    const updated = [...next];
    selectedSkillsRef.current = updated;
    setSelectedSkills(updated);
    try { localStorage.setItem('openharness:global-skills', JSON.stringify(updated)); } catch {}
    window.dispatchEvent(new CustomEvent('openharness:skills-changed', {detail: {names: updated}}));
  };

  useEffect(() => {
    const updateSkills = (event: Event) => {
      const names = (event as CustomEvent<{names?: string[]}>).detail?.names ?? [];
      selectedSkillsRef.current = names;
      setSelectedSkills(names);
      try { localStorage.setItem('openharness:global-skills', JSON.stringify(names)); } catch {}
    };
    window.addEventListener('openharness:skills-changed', updateSkills);
    return () => { window.removeEventListener('openharness:skills-changed', updateSkills); };
  }, [skillsOpen]);

  useEffect(() => {
    const openSettings = (event?: Event) => {
      const section = (event as CustomEvent<{section?: string}>)?.detail?.section;
      if (section) setInitialSection(section === 'Model Providers' ? 'Providers' : section);
      setSettingsOpen(true);
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { setSettingsOpen(false); setSkillsOpen(false); }
    };
    window.addEventListener('openharness:open-settings', openSettings);
    const goHome = () => { setSettingsOpen(false); setSkillsOpen(false); };
    const openSkills = () => {
      setSettingsOpen(false);
      setSkillsOpen(true);
      void refreshSkills();
      window.dispatchEvent(new CustomEvent('openharness:skills-changed', {detail: {names: selectedSkillsRef.current}}));
    };
    window.addEventListener('openharness:home', goHome);
    window.addEventListener('openharness:skills', openSkills);
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
      window.removeEventListener('openharness:home', goHome);
      window.removeEventListener('openharness:skills', openSkills);
      window.removeEventListener('keydown', closeOnEscape);
    };
  }, []);

  return (
    <div className="app">
      <Rail onGearClick={() => { setSkillsOpen(false); setSettingsOpen(value => !value); }} gearActive={settingsOpen} skillsActive={skillsOpen}/>
      {skillsOpen ? <><Drawer /><SkillsPage skills={skills} loading={loadingSkills} error={skillsError} selected={visibleSelectedSkills} search={skillSearch} onSearch={setSkillSearch} onToggle={toggleSkill} onRefresh={() => void refreshSkills()} onClose={() => setSkillsOpen(false)} /></> : <><Drawer /><ChatWindow /></>}
      {settingsOpen && <SettingsPage onClose={() => setSettingsOpen(false)} initialSection={initialSection}/>}
    </div>
  );
}
