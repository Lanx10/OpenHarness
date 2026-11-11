import type { StepEventLike } from './StepEvent';

export type QueuedPrompt = {
  id: string;
  text: string;
  attachments: string[];
  modelId: string;
  provider: string;
  supportsVision: boolean;
  modelContextTokens: number;
  modelOutputTokens: number;
  projectId?: string | null;
  threadId?: string | null;
  selectedSkills: string[];
};

export function readQueuedPrompts(storage: Pick<Storage, 'getItem'>, key: string): QueuedPrompt[] {
  try {
    const value: unknown = JSON.parse(storage.getItem(key) ?? '[]');
    if (!Array.isArray(value)) return [];
    return value.filter((item): item is QueuedPrompt => Boolean(
      item && typeof item === 'object' &&
      typeof item.id === 'string' && typeof item.text === 'string' &&
      Array.isArray(item.attachments) && item.attachments.every((attachment: unknown) => typeof attachment === 'string') &&
      typeof item.modelId === 'string' && typeof item.provider === 'string' &&
      typeof item.supportsVision === 'boolean' &&
      typeof item.modelContextTokens === 'number' && Number.isFinite(item.modelContextTokens) && item.modelContextTokens > 0 &&
      typeof item.modelOutputTokens === 'number' && Number.isFinite(item.modelOutputTokens) && item.modelOutputTokens > 0 &&
      (item.projectId === null || typeof item.projectId === 'string') &&
      (item.threadId === null || typeof item.threadId === 'string') &&
      Array.isArray(item.selectedSkills) && item.selectedSkills.every((skill: unknown) => typeof skill === 'string')
    ));
  } catch {
    return [];
  }
}

export function estimateMessageTokens(text: string, attachments: string[] = []): number {
  const content = [text, ...attachments].join('\n');
  return content.trim() ? Math.ceil(content.length / 4) : 0;
}

export function markPromptActive(prompt: QueuedPrompt): QueuedPrompt {
  const baseId = prompt.id.replace(/^active:/, '');
  return {...prompt, id: `active:${baseId}`};
}

export function upsertStep(steps: StepEventLike[], step: StepEventLike): StepEventLike[] {
  const index = steps.findIndex(item => item.id === step.id);
  if (index === -1) return [...steps, step];
  return steps.map((item, position) => position === index ? step : item);
}
