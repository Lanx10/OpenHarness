export type ProviderModel = {id: string; provider: string};

export function parseSavedModel(value: string | null): ProviderModel | null {
  if (!value) return null;
  try {
    const parsed: unknown = JSON.parse(value);
    if (!parsed || typeof parsed !== 'object') return null;
    const {id, provider} = parsed as Record<string, unknown>;
    return typeof id === 'string' && id.trim() && typeof provider === 'string' && provider.trim()
      ? {id, provider}
      : null;
  } catch {
    return null;
  }
}

export function reconcileModelSelection<T extends ProviderModel>(
  current: T | null,
  models: T[],
  saved: ProviderModel | null,
): {models: T[]; selected: T | null; unavailable: boolean} {
  const exists = (candidate: ProviderModel | null): T | null =>
    candidate ? models.find(model => model.id === candidate.id && model.provider === candidate.provider) ?? null : null;
  if (current) {
    const selected = exists(current);
    return {models, selected, unavailable: !selected};
  }
  if (saved) {
    const selected = exists(saved);
    return {models, selected, unavailable: !selected};
  }
  return {models, selected: models[0] ?? null, unavailable: false};
}
