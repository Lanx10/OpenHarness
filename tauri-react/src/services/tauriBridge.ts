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

const DEFAULT_PROJECTS: ProjectInfo[] = [
  { id: 'victory-liner', name: 'Victory Liner', subtitle: '💬 Homepage, Navigation and...', active: true },
  { id: 'experts', name: 'Experts', subtitle: '💬 MT5 Gold# Scalping EA', active: false },
  { id: 'smart-inventory', name: 'Smart Inventory', subtitle: '💬 /impeccable shape the staff...', active: false },
  { id: 'lgu-educ', name: 'LGU EducAssistance', subtitle: '3 archived chats', active: false },
  { id: 'kondukt', name: 'kondukt', subtitle: 'No chats yet', active: false },
  { id: 'prmsu', name: 'prmsu-research-services', subtitle: '1 archived chat', active: false },
  { id: 'lanxbooth', name: 'LanxBooth', subtitle: 'No chats yet', active: false },
  { id: 'ai-trading', name: 'AI Trading', subtitle: '2 archived chats', active: false },
];

let localProjects: ProjectInfo[] = [...DEFAULT_PROJECTS];
let localProviders: ProviderInfo[] = [];

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
