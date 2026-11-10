import {useEffect, useRef, useState} from 'react';
import {PlusIcon, FolderIcon, XMarkIcon, PencilIcon, TrashIcon, MagnifyingGlassIcon, ChevronLeftIcon, ChevronRightIcon} from '@heroicons/react/20/solid';

const DRAWER_MIN_WIDTH = 160;
const DRAWER_MAX_WIDTH = 260;
const DRAWER_COLLAPSED_WIDTH = 52;
import {apiCreateProject, apiGetProjects, apiSelectProject, apiRenameProject, apiDeleteProject, apiRelinkProject, type ProjectInfo} from '../services/tauriBridge';
import {pickProjectFolder} from '../services/tauriBridge';

export function Drawer() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [, setActiveProject] = useState<ProjectInfo | null>(null);
  const [drawerCollapsed, setDrawerCollapsed] = useState(() => {
    try { return localStorage.getItem('openharness:drawer-collapsed') === 'true'; } catch { return false; }
  });
  const [drawerWidth, setDrawerWidth] = useState(() => {
    try {
      const saved = Number(localStorage.getItem('openharness:drawer-width'));
      return Number.isFinite(saved) && saved >= DRAWER_MIN_WIDTH && saved <= DRAWER_MAX_WIDTH ? saved : 250;
    } catch {
      return 250;
    }
  });
  const [search, setSearch] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState('');
  const [projectModalOpen, setProjectModalOpen] = useState(false);
  const [projectName, setProjectName] = useState('');
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const [isSelecting, setIsSelecting] = useState(false);
  const searchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    try {
      localStorage.setItem('openharness:drawer-width', String(drawerWidth));
    } catch {}
  }, [drawerWidth]);

  const toggleDrawer = () => {
    setDrawerCollapsed(prev => {
      const next = !prev;
      try { localStorage.setItem('openharness:drawer-collapsed', String(next)); } catch {}
      return next;
    });
  };

  const handleCollapsedSearchClick = () => {
    setDrawerCollapsed(false);
    try { localStorage.setItem('openharness:drawer-collapsed', 'false'); } catch {}
    window.setTimeout(() => {
      searchInputRef.current?.focus();
    }, 60);
  };

  const handleResizePointerDown = (event: React.PointerEvent<HTMLDivElement>) => {
    if (drawerCollapsed) return;
    const startX = event.clientX;
    const startWidth = drawerWidth;
    const captured = event.currentTarget;

    const move = (pointer: PointerEvent) => {
      const nextWidth = Math.min(DRAWER_MAX_WIDTH, Math.max(DRAWER_MIN_WIDTH, startWidth + (pointer.clientX - startX)));
      setDrawerWidth(nextWidth);
    };

    const stop = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', stop);
      window.removeEventListener('pointercancel', stop);
      window.removeEventListener('lostpointercapture', stop);
      if (captured && captured.hasPointerCapture?.(event.pointerId)) {
        try { captured.releasePointerCapture(event.pointerId); } catch {}
      }
    };

    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', stop, {once: true});
    window.addEventListener('pointercancel', stop, {once: true});
    window.addEventListener('lostpointercapture', stop, {once: true});
    captured.setPointerCapture(event.pointerId);
  };

  const handleResizeKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (drawerCollapsed) return;
    if (event.key === 'ArrowRight') {
      event.preventDefault();
      setDrawerWidth(w => Math.min(DRAWER_MAX_WIDTH, w + 10));
    } else if (event.key === 'ArrowLeft') {
      event.preventDefault();
      setDrawerWidth(w => Math.max(DRAWER_MIN_WIDTH, w - 10));
    } else if (event.key === 'Home') {
      event.preventDefault();
      setDrawerWidth(DRAWER_MIN_WIDTH);
    } else if (event.key === 'End') {
      event.preventDefault();
      setDrawerWidth(DRAWER_MAX_WIDTH);
    }
  };

  useEffect(() => {
    const handleFocus = () => { /* focus management point */ };
    window.addEventListener('focus', handleFocus);
    return () => window.removeEventListener('focus', handleFocus);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void apiGetProjects().then(items => {
      if (!cancelled) {
        const validItems = items;
        setProjects(validItems);
        const active = validItems.find(project => project.active) ?? validItems[0] ?? null;
        setActiveProject(active);
        if (active) window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: active}));
        else if (validItems.length) void selectProject(validItems[0]);
      }
    }).catch(error => {
      if (!cancelled) setNotice(`Could not load projects: ${String(error)}`);
    });
    return () => { cancelled = true; };
  }, []);

  useEffect(() => {
    const handleOpenProject = async () => {
      setProjectModalOpen(true);
      setProjectName('');
      setProjectPath(null);
    };
    window.addEventListener('openharness:new-project', handleOpenProject);
    window.addEventListener('openharness:create-project', handleOpenProject);
    return () => {
      window.removeEventListener('openharness:new-project', handleOpenProject);
      window.removeEventListener('openharness:create-project', handleOpenProject);
    };
  }, []);

  const handleBrowseFolder = async () => {
    setIsSelecting(true);
    try {
      const picked = await pickProjectFolder();
      if (picked) {
        setProjectPath(picked.path);
        setProjectName(picked.name);
      }
    } catch (err) {
      console.error(err);
    } finally {
      setIsSelecting(false);
    }
  };

  const createProject = async () => {
    const name = projectName.trim();
    if (!name) return;
    try {
      await apiCreateProject(name, projectPath);
      const updated = await apiGetProjects();
      setProjects(updated);
      setProjectModalOpen(false);
      setNotice(`Project "${name}" opened`);
      window.setTimeout(() => setNotice(''), 2500);
       const active = updated.find(project => project.active) ?? updated[0] ?? null;
       setActiveProject(active);
       if (active) window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: active}));
       else { setActiveProject(null); window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: null})); }
     } catch (error) {
       setNotice(`Could not create project: ${String(error)}`);
    }
  };

  const selectProject = async (project: ProjectInfo) => {
    try {
       const next = await apiSelectProject(project.id);
       setProjects(next);
       const active = next.find(item => item.active) ?? project;
       setActiveProject(active);
       window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: active}));
    } catch (error) {
      setNotice(`Could not select project: ${String(error)}`);
    }
  };

  return (
    <>
      <aside className={`drawer ${drawerCollapsed ? 'drawer-collapsed' : ''}`} style={{width: drawerCollapsed ? DRAWER_COLLAPSED_WIDTH : drawerWidth, flexBasis: drawerCollapsed ? DRAWER_COLLAPSED_WIDTH : drawerWidth}} aria-label="Projects and chats" data-od-id="project-drawer">
        {drawerCollapsed ? (
          <div className="drawer-collapsed-actions">
            <button type="button" className="collapsed-drawer-action" onClick={toggleDrawer} aria-label="Expand project sidebar" title="Expand project sidebar"><ChevronRightIcon width={15} height={15} aria-hidden="true" /></button>
            <button type="button" className="collapsed-drawer-action collapsed-drawer-primary" onClick={() => window.dispatchEvent(new Event('openharness:new-chat'))} aria-label="New chat" title="New chat"><PlusIcon width={17} height={17} aria-hidden="true" /></button>
            <button type="button" className="collapsed-drawer-action" onClick={() => window.dispatchEvent(new Event('openharness:new-project'))} aria-label="New project" title="New project"><FolderIcon width={16} height={16} aria-hidden="true" /></button>
            <button type="button" className="collapsed-drawer-action" onClick={handleCollapsedSearchClick} aria-label="Search projects" title="Search projects"><MagnifyingGlassIcon width={16} height={16} aria-hidden="true" /></button>
          </div>
        ) : <>
        <div className="brand" data-od-id="brand" style={{justifyContent: 'space-between', alignItems: 'center'}}>
          <div style={{display: 'flex', alignItems: 'center', gap: 9}}>
            <span className="brand-mark" aria-hidden="true">
              <img src="/appicon/openharness-desktop-mark-only-512-v2.svg" alt="" width={16} height={16} style={{display: 'block'}} />
            </span>
            <span className="brand-name">OpenHarness</span>
          </div>
          <button
            type="button"
            className="collapse-toggle-btn"
            aria-label="Collapse sidebar"
            title="Collapse sidebar"
            onClick={toggleDrawer}
            style={{
              background: 'transparent',
              border: 0,
              color: 'var(--muted)',
              cursor: 'pointer',
              display: 'grid',
              placeItems: 'center',
              padding: '3px 5px',
              borderRadius: 4
            }}
          >
            <ChevronLeftIcon width={14} height={14} />
          </button>
        </div>

        <div className="actions">
          <button className="action-button new-chat-primary" id="new-chat" type="button" data-od-id="new-chat-button" onClick={() => window.dispatchEvent(new Event('openharness:new-chat'))}>
            <PlusIcon width={14} height={14} aria-hidden="true" /> New chat
          </button>
          <button className="action-button" id="new-project" type="button" data-od-id="new-project-button" onClick={() => window.dispatchEvent(new Event('openharness:new-project'))}>
            <FolderIcon width={14} height={14} aria-hidden="true" /> New project
          </button>
        </div>

        <label className="project-search"><MagnifyingGlassIcon width={14} aria-hidden="true" /><input ref={searchInputRef} value={search} onChange={e => setSearch(e.target.value)} placeholder="Search projects" aria-label="Search projects"/></label>
        <p className="group-heading projects-heading">Projects</p>
        <div className="project-list" id="project-list" role="list">
            {projects.length === 0 ? (
              <div className="empty-state" style={{padding: '16px 14px', fontSize: '12px', color: '#94a3b8'}}>
                No projects yet. Click "+ New project" above to open a local project folder.
              </div>
            ) : projects.filter(p => p.name.toLowerCase().includes(search.toLowerCase())).map(project => (
              <div key={project.id} className={`project-row ${project.active ? 'selected' : ''}`}>
                <button
                  type="button"
                  role="listitem"
                  data-project={project.name}
                  className={`project ${project.active ? 'selected project-active' : ''}`}
                  title={project.name}
                  onClick={() => void selectProject(project)}
                >
                  <span className="project-top">
                    <FolderIcon width={14} height={14} aria-hidden="true" />
                    <span className="project-name">{project.name}</span>
                  </span>
                </button>
                <div className="project-actions">
                  <button type="button" aria-label={`Rename ${project.name}`} title="Rename project" onClick={() => { setEditingId(project.id); setEditingName(project.name); }}><PencilIcon width={12}/></button>
                   <button type="button" aria-label={`Relink ${project.name} folder`} title="Relink folder" onClick={async () => {
                     const picked = await pickProjectFolder();
                     if (!picked) return;
                     try {
                       const next = await apiRelinkProject(project.id, picked.path);
                       setProjects(next);
                       const active = next.find(item => item.active) ?? null;
                       setActiveProject(active);
                       if (active) window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: active}));
                     } catch (error) { setNotice(String(error)); }
                   }}><FolderIcon width={12}/></button>
                  <button type="button" aria-label={`Delete ${project.name}`} title="Delete project" onClick={async () => {
                     if (!window.confirm(`Delete ${project.name}?`)) return;
                     try {
                       const next = await apiDeleteProject(project.id);
                       setProjects(next);
                       const active = next.find(item => item.active) ?? next[0] ?? null;
                       setActiveProject(active);
                       window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: active}));
                     } catch (error) { setNotice(String(error)); }
                   }}><TrashIcon width={12}/></button>
                </div>
                {editingId === project.id && (
                  <div className="project-edit">
                    <input value={editingName} onChange={e => setEditingName(e.target.value)} aria-label="Project name" autoFocus onKeyDown={async e => { if (e.key === 'Enter') { try { setProjects(await apiRenameProject(project.id, editingName)); setEditingId(null); } catch (error) { setNotice(String(error)); } } if (e.key === 'Escape') setEditingId(null); }} />
                    <button type="button" onClick={async () => { try { setProjects(await apiRenameProject(project.id, editingName)); setEditingId(null); } catch (error) { setNotice(String(error)); } }}>Save</button>
                  </div>
                )}
              </div>
            ))}
          </div>
        <div className="drawer-foot"><span className="group-heading">Workspace · Local</span></div>
        <div
          className="drawer-resizer"
          onPointerDown={handleResizePointerDown}
          onKeyDown={handleResizeKeyDown}
          tabIndex={0}
          role="separator"
          aria-orientation="vertical"
          aria-valuenow={drawerWidth}
          aria-valuemin={DRAWER_MIN_WIDTH}
          aria-valuemax={DRAWER_MAX_WIDTH}
          aria-label="Resize project drawer"
        />
        </>}
      </aside>

      <div className={`modal-backdrop ${projectModalOpen ? 'open' : ''}`} role="presentation" onMouseDown={event => { if (event.target === event.currentTarget) setProjectModalOpen(false); }} onKeyDown={event => { if (event.key === 'Escape') setProjectModalOpen(false); }}>
        <div className="modal" role="dialog" aria-modal="true" aria-labelledby="modal-title">
          <div className="modal-header">
            <h2 id="modal-title">Open Project Folder</h2>
            <button type="button" className="iconbtn" aria-label="Close modal" title="Close" onClick={() => setProjectModalOpen(false)}>
              <XMarkIcon width={16} height={16} aria-hidden="true" />
            </button>
          </div>
          <p>Select a folder from your file system. The project name syncs automatically to your folder name.</p>
          
          <label htmlFor="project-path-display" className="modal-sub-label">Selected Directory</label>
          <div className="folder-select-row">
            <input id="project-path-display" readOnly value={projectPath || 'No folder selected'} placeholder="Pick a directory" className={projectPath ? 'has-path' : ''} />
            <button type="button" className="browse-btn" onClick={handleBrowseFolder} disabled={isSelecting}>
              <FolderIcon width={14} height={14} aria-hidden="true" /> {isSelecting ? 'Opening...' : 'Browse...'}
            </button>
          </div>

          <label htmlFor="project-name-input" className="modal-sub-label" style={{marginTop: '14px'}}>Project Name (Auto-synced)</label>
          <input id="project-name-input" autoFocus value={projectName} onChange={event => setProjectName(event.target.value)} onKeyDown={event => { if (event.key === 'Enter') void createProject(); }} placeholder="Project name" aria-label="Project name"/>
          
          <div className="modal-actions">
            <button type="button" onClick={() => setProjectModalOpen(false)}>Cancel</button>
            <button type="button" className="confirm" onClick={() => void createProject()} disabled={!projectName.trim()}>Open Project</button>
          </div>
        </div>
      </div>
      {notice && <div className="drawer-notice" role="status" aria-live="polite">{notice}</div>}
    </>
  );
}
