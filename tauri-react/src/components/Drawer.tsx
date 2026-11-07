import {useEffect, useState} from 'react';
import {PlusIcon, FolderIcon, XMarkIcon} from '@heroicons/react/20/solid';
import {apiCreateProject, apiGetProjects, apiSelectProject, type ProjectInfo} from '../services/tauriBridge';
import {pickProjectFolder} from '../services/tauriBridge';

export function Drawer() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [projectModalOpen, setProjectModalOpen] = useState(false);
  const [projectName, setProjectName] = useState('');
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const [isSelecting, setIsSelecting] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void apiGetProjects().then(items => {
      if (!cancelled) {
        setProjects(items);
        const activeProject = items.find(project => project.active);
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
      <aside className="drawer" aria-label="Projects and chats" data-od-id="project-drawer">
        <div className="brand" data-od-id="brand">
          <span className="brand-mark" aria-hidden="true">
            <img src="/appicon/openharness-desktop-mark-only-512-v2.svg" alt="" width={16} height={16} style={{display: 'block'}} />
          </span>
          <span className="brand-name">OpenHarness</span>
        </div>

        <div className="actions">
          <button className="action-button" id="new-chat" type="button" data-od-id="new-chat-button" onClick={() => window.dispatchEvent(new Event('openharness:new-chat'))}>
            <PlusIcon width={14} height={14} aria-hidden="true" /> New chat
          </button>
          <button className="action-button" id="new-project" type="button" data-od-id="new-project-button" onClick={() => window.dispatchEvent(new Event('openharness:new-project'))}>
            <FolderIcon width={14} height={14} aria-hidden="true" /> New project
          </button>
        </div>

        <p className="group-heading">Projects</p>
        <div className="project-list" id="project-list" role="list">
          {projects.map(project => (
            <button
              key={project.id}
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
          ))}
        </div>
        <div className="drawer-foot"><span className="group-heading">Workspace · Local</span></div>
      </aside>

      <div className={`modal-backdrop ${projectModalOpen ? 'open' : ''}`} role="presentation" onMouseDown={event => { if (event.target === event.currentTarget) setProjectModalOpen(false); }}>
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
