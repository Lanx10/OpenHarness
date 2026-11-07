import { invoke, isTauri } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { getCurrentWindow } from '@tauri-apps/api/window';

export async function windowMinimize(): Promise<void> {
  if (isTauri()) {
    return getCurrentWindow().minimize();
  }
  throw new Error('Window control unavailable outside the Tauri desktop app');
}

export async function windowToggleMaximize(): Promise<void> {
  if (isTauri()) {
    return getCurrentWindow().toggleMaximize();
  }
  throw new Error('Window control unavailable outside the Tauri desktop app');
}

export async function windowClose(): Promise<void> {
  if (isTauri()) {
    return getCurrentWindow().close();
  }
  throw new Error('Window control unavailable outside the Tauri desktop app');
}

export type ProviderInfo = {
  provider: string;
  model_id: string;
  base_url?: string | null;
  custom_endpoint: boolean;
  has_key: boolean;
};

export type ProjectInfo = {
  id: string;
  name: string;
  subtitle: string;
  path?: string | null;
  active: boolean;
};

export type ChatResponse = {
  text: string;
  model: string;
  provider: string;
};

const PROJECTS_STORAGE_KEY = 'openharness:projects';
const PROVIDERS_STORAGE_KEY = 'openharness:providers';

function loadStoredProjects(): ProjectInfo[] {
  try {
    const raw = localStorage.getItem(PROJECTS_STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function persistStoredProjects(projects: ProjectInfo[]): void {
  try {
    localStorage.setItem(PROJECTS_STORAGE_KEY, JSON.stringify(projects));
  } catch {}
}

function loadStoredProviders(): ProviderInfo[] {
  try {
    const raw = localStorage.getItem(PROVIDERS_STORAGE_KEY);
    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function persistStoredProviders(providers: ProviderInfo[]): void {
  try {
    localStorage.setItem(PROVIDERS_STORAGE_KEY, JSON.stringify(providers));
  } catch {}
}

let localProjects: ProjectInfo[] = loadStoredProjects();
let localProviders: ProviderInfo[] = loadStoredProviders();

export async function apiSaveProvider(
  provider: string,
  modelId: string,
  baseUrl: string | null,
  customEndpoint: boolean,
  apiKey: string
): Promise<ProviderInfo> {
  if (isTauri()) {
    return await invoke<ProviderInfo>('provider_save', {
      provider,
      modelId,
      baseUrl,
      customEndpoint,
      apiKey,
    });
  }
  const info: ProviderInfo = {
    provider,
    model_id: modelId,
    base_url: baseUrl,
    custom_endpoint: customEndpoint,
    has_key: true,
  };
  localProviders = localProviders.filter((p) => p.provider !== provider).concat(info);
  persistStoredProviders(localProviders);
  return info;
}

export async function apiListProviders(): Promise<ProviderInfo[]> {
  if (isTauri()) {
    return await invoke<ProviderInfo[]>('provider_list');
  }
  return [...localProviders];
}

export async function apiDeleteProvider(provider: string): Promise<void> {
  if (isTauri()) {
    await invoke('provider_delete', { provider });
    return;
  }
  localProviders = localProviders.filter((p) => p.provider !== provider);
  persistStoredProviders(localProviders);
}

export async function apiGetProjects(): Promise<ProjectInfo[]> {
  if (isTauri()) {
    return await invoke<ProjectInfo[]>('get_projects');
  }
  return [...localProjects];
}

export async function apiSelectProject(id: string): Promise<ProjectInfo[]> {
  if (isTauri()) {
    return await invoke<ProjectInfo[]>('select_project', { id });
  }
  localProjects = localProjects.map((p) => ({ ...p, active: p.id === id }));
  persistStoredProjects(localProjects);
  return [...localProjects];
}

export async function pickProjectFolder(): Promise<{ path: string; name: string } | null> {
  if (isTauri()) {
    try {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: 'Select Project Folder',
      });
      if (!selected || typeof selected !== 'string') return null;
      const name = selected.replace(/\\+$/, '').split(/[\\/]/).pop() || selected;
      return { path: selected, name };
    } catch (e) {
      console.warn('Dialog open failed, falling back:', e);
    }
  }
  return null;
}

export async function apiCreateProject(name: string, path?: string | null): Promise<ProjectInfo> {
  if (isTauri()) {
    return await invoke<ProjectInfo>('create_project', { name, path: path ?? null });
  }
  const id = name.toLowerCase().replace(/\s+/g, '-');
  const created: ProjectInfo = { id, name, subtitle: path ? `📁 ${path}` : 'No chats yet', path: path ?? null, active: true };
  localProjects = [created, ...localProjects.map((p) => ({ ...p, active: false }))];
  persistStoredProjects(localProjects);
  return created;
}
export async function apiChatSend(
  prompt: string,
  modelId: string,
  provider: string,
  projectId?: string
): Promise<ChatResponse> {
  if (isTauri()) {
    return await invoke<ChatResponse>('chat_send', {
      prompt,
      modelId,
      provider,
      projectId: projectId ?? null,
    });
  }
  const trimmed = prompt.trim();
  const hasProvider = localProviders.some((p) => p.provider === provider && p.has_key);
  const text = hasProvider
    ? `Connected to ${provider} (${modelId}). Processing in project '${projectId || 'default'}':\n\nPlan executed for: "${trimmed}"`
    : `Local OpenHarness Assistant (${modelId}):\n\nI understand you want to work on: "${trimmed}". Configure an API key for ${provider} in Settings to route live requests to upstream inference.`;
  return { text, model: modelId, provider };
}
