import { Channel, invoke, isTauri } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
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

export async function apiTestProviderConnection(
  provider: string,
  modelId: string,
  baseUrl: string | null,
  apiKey: string
): Promise<string> {
  if (isTauri()) {
    return invoke('test_provider_connection', { provider, modelId, baseUrl, apiKey });
  }
  return 'Web demo: connection simulated successfully';
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
export async function apiCancelChatStream(streamId?: string): Promise<void> {
  if (isTauri()) await invoke('cancel_chat_stream', { streamId: streamId || null });
}

export type ApprovalRequestLike = {
  id: string;
  tool: string;
  arguments: Record<string, unknown>;
};

import type { StepEventLike } from '../components/StepEvent';
export type StreamChunkLike = {
  text: string;
  done: boolean;
  error?: string | null;
  approval?: ApprovalRequestLike | null;
  step?: StepEventLike | null;
};

export async function apiChatStream(
  prompt: string,
  modelId: string,
  provider: string,
  projectId?: string,
  threadId?: string,
  attachments?: string[],
  onToken?: (token: string) => void,
  onDone?: () => void,
  onError?: (error: string) => void,
  onApproval?: (approval: ApprovalRequestLike) => void,
  onStep?: (step: StepEventLike) => void,
): Promise<void> {
  if (isTauri()) {
    const channel = new Channel<StreamChunkLike>();
    channel.onmessage = (message) => {
      if (message.error) onError?.(message.error);
      if (message.text) onToken?.(message.text);
      if (message.approval) onApproval?.(message.approval);
      if (message.step) onStep?.(message.step);
      if (message.done) onDone?.();
    };
    await invoke('chat_send_stream', {
      prompt,
      modelId,
      provider,
      projectId: projectId ?? null,
      threadId: threadId ?? null,
      attachments: attachments ?? null,
      channel,
    });
    return;
  }
  const text = `Local OpenHarness Assistant (${modelId}):\n\nProcessing "${prompt.trim()}". Configure a provider for live inference.`;
  if (threadId) {
    await apiAppendMessage(threadId, 'user', prompt.trim());
    await apiAppendMessage(threadId, 'assistant', text);
  }
  onToken?.(text);
  onDone?.();
}

export type ChatMessageLike = { id: string; role: string; text: string; attachments?: string[] };
export type ChatThreadLike = { id: string; project_id: string; title: string; messages: ChatMessageLike[] };

let localThreads: ChatThreadLike[] = [];

export async function apiListThreads(projectId?: string): Promise<ChatThreadLike[]> {
  if (isTauri()) return invoke('list_threads', { projectId: projectId ?? null });
  return localThreads.filter(thread => !projectId || thread.project_id === projectId);
}

export async function apiCreateThread(projectId: string): Promise<ChatThreadLike> {
  if (isTauri()) return invoke('create_thread', { projectId });
  const thread = { id: `thread-${Date.now()}`, project_id: projectId, title: 'New chat', messages: [] };
  localThreads = [thread, ...localThreads];
  return thread;
}

export async function apiDeleteThread(id: string): Promise<ChatThreadLike[]> {
  if (isTauri()) return invoke('delete_thread', { id });
  localThreads = localThreads.filter(thread => thread.id !== id);
  return [...localThreads];
}

export async function apiRenameThread(id: string, title: string): Promise<ChatThreadLike[]> {
  if (isTauri()) return invoke('rename_thread', { id, title });
  localThreads = localThreads.map(thread => thread.id === id ? {...thread, title} : thread);
  return [...localThreads];
}

export async function apiGetThread(id: string): Promise<ChatThreadLike> {
  if (isTauri()) return invoke('get_thread', { id });
  const thread = localThreads.find(item => item.id === id);
  if (thread) return thread;
  return { id, project_id: 'default', title: 'Chat', messages: [] };
}

export async function apiAppendMessage(threadId: string, role: string, text: string): Promise<ChatMessageLike> {
  if (isTauri()) return invoke('append_thread_message', { threadId, role, text });
  const msg = { id: `msg-${Date.now()}`, role, text };
  localThreads = localThreads.map(thread => {
    if (thread.id !== threadId) return thread;
    const title = thread.title === 'New chat' && role === 'user' ? text.slice(0, 40) : thread.title;
    return { ...thread, title, messages: [...thread.messages, msg] };
  });
  return msg;
}

export async function apiEditMessage(
  threadId: string,
  messageId: string,
  text: string
): Promise<ChatMessageLike> {
  if (isTauri()) return invoke('edit_thread_message', { threadId, messageId, text });
  return { id: messageId, role: 'user', text };
}

export async function apiDeleteMessage(
  threadId: string,
  messageId: string
): Promise<ChatMessageLike[]> {
  if (isTauri()) return invoke('delete_thread_message', { threadId, messageId });
  return [];
}

export async function apiRenameProject(id: string, name: string): Promise<ProjectInfo[]> {
  if (isTauri()) return invoke('rename_project', { id, name });
  return [];
}

export async function apiDeleteProject(id: string): Promise<ProjectInfo[]> {
  if (isTauri()) return invoke('delete_project', { id });
  return [];
}

export async function apiRelinkProject(id: string, path: string | null): Promise<ProjectInfo[]> {
  if (isTauri()) return invoke('relink_project', { id, path });
  return [];
}

export async function apiWorkspaceReadFile(projectId: string, path: string, offset?: number, limit?: number): Promise<string> {
  if (!isTauri()) throw new Error('Workspace tools require the desktop app');
  return invoke('workspace_read_file', { projectId, path, offset: offset ?? null, limit: limit ?? null });
}

export async function apiWorkspaceListDir(projectId: string, path?: string): Promise<FileEntryLike[]> {
  if (!isTauri()) throw new Error('Workspace tools require the desktop app');
  return invoke('workspace_list_dir', { projectId, path: path ?? null });
}

export async function apiWorkspaceGrep(projectId: string, query: string, pattern?: string): Promise<GrepMatchLike[]> {
  if (!isTauri()) throw new Error('Workspace tools require the desktop app');
  return invoke('workspace_grep', { projectId, query, pattern: pattern ?? null });
}

export async function apiGitStatus(projectId: string): Promise<string> {
  if (!isTauri()) throw new Error('Git context requires the desktop app');
  return invoke('workspace_git_status', { projectId });
}

export async function apiGitDiff(projectId: string, staged = false): Promise<string> {
  if (!isTauri()) throw new Error('Git context requires the desktop app');
  return invoke('workspace_git_diff', { projectId, staged });
}

export async function apiGitLog(projectId: string, limit = 10): Promise<string> {
  if (!isTauri()) throw new Error('Git context requires the desktop app');
  return invoke('workspace_git_log', { projectId, limit });
}

export type FileEntryLike = {name: string; path: string; is_dir: boolean; size: number};
export type GrepMatchLike = {file: string; line_number: number; line_text: string};

export async function apiTerminalCreate(projectId: string | null, shell: 'powershell' | 'cmd'): Promise<string> {
  if (!isTauri()) throw new Error('Terminal requires the desktop app');
  return invoke('terminal_create', { projectId, shell });
}

export async function apiTerminalWrite(id: string, data: string): Promise<void> {
  if (!isTauri()) throw new Error('Terminal requires the desktop app');
  await invoke('terminal_write', { id, data });
}

export async function apiTerminalKill(id: string): Promise<void> {
  if (!isTauri()) return;
  await invoke('terminal_kill', { id });
}

export async function apiListenTerminalOutput(handler: (id: string, data: string) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => {};
  return listen<{id: string; data: string}>('terminal-output', event => handler(event.payload.id, event.payload.data));
}

export async function apiListenTerminalExit(handler: (id: string, code: number | null) => void): Promise<UnlistenFn> {
  if (!isTauri()) return () => {};
  return listen<{id: string; code: number | null}>('terminal-exit', event => handler(event.payload.id, event.payload.code));
}

export async function apiRespondToToolApproval(
  id: string,
  allowed: boolean,
  response?: string,
): Promise<void> {
  if (!isTauri()) throw new Error('Tool approval requires the desktop app');
  await invoke('respond_to_tool_approval', { reply: { id, allowed, response: response ?? null } });
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
