import {useEffect, useState} from 'react';
import {PlusIcon, FolderIcon, XMarkIcon, PencilIcon, TrashIcon, MagnifyingGlassIcon, ChevronLeftIcon, ChevronRightIcon} from '@heroicons/react/20/solid';
import {apiCreateProject, apiGetProjects, apiSelectProject, apiRenameProject, apiDeleteProject, apiRelinkProject, type ProjectInfo} from '../services/tauriBridge';
import {pickProjectFolder} from '../services/tauriBridge';

export function Drawer() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [drawerCollapsed, setDrawerCollapsed] = useState(() => {
    try { return localStorage.getItem('openharness:drawer-collapsed') === 'true'; } catch { return false; }
  });
  const [projectsExpanded, setProjectsExpanded] = useState(true);
  const [search, setSearch] = useState('');
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState('');
  const [projectModalOpen, setProjectModalOpen] = useState(false);
  const [projectName, setProjectName] = useState('');
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const [isSelecting, setIsSelecting] = useState(false);

  const toggleDrawer = () => {
    setDrawerCollapsed(prev => {
      const next = !prev;
      try { localStorage.setItem('openharness:drawer-collapsed', String(next)); } catch {}
      return next;
    });
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
        const validItems = items.filter(project => !project.path || true);
        setProjects(validItems);
        const activeProject = validItems.find(project => project.active);
        if (activeProject) window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: activeProject}));
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
      const activeProject = updated.find(p => p.active);
      if (activeProject) {
        window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: activeProject}));
      }
    } catch (error) {
      setNotice(`Could not create project: ${String(error)}`);
    }
  };

  const selectProject = async (project: ProjectInfo) => {
    try {
      const next = await apiSelectProject(project.id);
      setProjects(next);
      window.dispatchEvent(new CustomEvent('openharness:select-project', {detail: project}));
    } catch (error) {
      setNotice(`Could not select project: ${String(error)}`);
    }
  };

  return (
    <>
      <aside className={`drawer ${drawerCollapsed ? 'drawer-collapsed' : ''}`} aria-label="Projects and chats" data-od-id="project-drawer">
        {drawerCollapsed ? (
          <button
            type="button"
            className="collapse-toggle-btn"
            onClick={toggleDrawer}
            aria-label="Expand new chat sidebar"
            title="Expand new chat sidebar"
            style={{
              background: 'transparent',
              border: 0,
              color: 'var(--accent)',
              cursor: 'pointer',
              display: 'grid',
              placeItems: 'center',
              padding: '4px',
              borderRadius: 4,
              alignSelf: 'center'
            }}
          >
            <ChevronRightIcon width={14} height={14} />
          </button>
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
          <button
            className="action-button new-chat-primary"
            id="new-chat"
            type="button"
            data-od-id="new-chat-button"
            onClick={() => window.dispatchEvent(new Event('openharness:new-chat'))}
          >
            <PlusIcon width={14} height={14} aria-hidden="true" /> New chat
          </button>
          <button className="action-button" id="new-project" type="button" data-od-id="new-project-button" onClick={() => window.dispatchEvent(new Event('openharness:new-project'))}>
            <FolderIcon width={14} height={14} aria-hidden="true" /> New project
          </button>
        </div>

        <label className="project-search"><MagnifyingGlassIcon width={14} aria-hidden="true" /><input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search projects" aria-label="Search projects"/></label>
        <div style={{display: 'flex', alignItems: 'center', justifyContent: 'space-between', margin: '0 0 7px'}}>
          <p className="group-heading" style={{margin: 0}}>Projects</p>
          <button
            type="button"
            className="collapse-toggle-btn"
            onClick={() => setProjectsExpanded(v => !v)}
            aria-label={projectsExpanded ? "Collapse projects" : "Expand projects"}
            title={projectsExpanded ? "Collapse projects" : "Expand projects"}
            style={{
              background: 'transparent',
              border: 0,
              color: 'var(--muted)',
              cursor: 'pointer',
              display: 'grid',
              placeItems: 'center',
              padding: '2px 4px',
              borderRadius: 4
            }}
          >
            {projectsExpanded ? <ChevronLeftIcon width={14} height={14} style={{transform: 'rotate(-90deg)'}} /> : <ChevronRightIcon width={14} height={14} style={{transform: 'rotate(90deg)'}} />}
          </button>
        </div>
        {projectsExpanded && (
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
                  title={project.path || project.name}
                  onClick={() => void selectProject(project)}
                >
                  <span className="project-top">
                    <FolderIcon width={14} height={14} aria-hidden="true" />
                    <span className="project-name">{project.name}</span>
                  </span>
                  <span className="project-subtitle">{project.path ? project.path : project.subtitle}</span>
                </button>
                <div className="project-actions">
                  <button type="button" aria-label={`Rename ${project.name}`} title="Rename project" onClick={() => { setEditingId(project.id); setEditingName(project.name); }}><PencilIcon width={12}/></button>
                  <button type="button" aria-label={`Relink ${project.name} folder`} title="Relink folder" onClick={async () => { const picked = await pickProjectFolder(); if (picked) { try { setProjects(await apiRelinkProject(project.id, picked.path)); } catch (error) { setNotice(String(error)); } } }}><FolderIcon width={12}/></button>
                  <button type="button" aria-label={`Delete ${project.name}`} title="Delete project" onClick={async () => { if (!window.confirm(`Delete ${project.name}?`)) return; try { setProjects(await apiDeleteProject(project.id)); } catch (error) { setNotice(String(error)); } }}><TrashIcon width={12}/></button>
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
        )}
        <div className="drawer-foot"><span className="group-heading">Workspace · Local</span></div>
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
