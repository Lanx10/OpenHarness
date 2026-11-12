export type ProviderUsageLike = {
  input_tokens?: number | null;
  output_tokens?: number | null;
  total_tokens?: number | null;
  cached_input_tokens?: number | null;
  reasoning_tokens?: number | null;
};

export type TokenUsageRecordLike = {request_id?: string; source: string; usage: ProviderUsageLike};
export type TokenUsageTotals = {
  input: number | null;
  output: number | null;
  total: number | null;
  cached: number | null;
  reasoning: number | null;
};

export const emptyTokenUsage = (): TokenUsageTotals => ({input: null, output: null, total: null, cached: null, reasoning: null});

export function tokenUsageTotals(records: TokenUsageRecordLike[] = []): TokenUsageTotals {
  const totals = emptyTokenUsage();
  for (const {source, usage} of records) {
    if (source !== 'provider_reported') continue;
    const fields = [
      ['input', usage.input_tokens],
      ['output', usage.output_tokens],
      ['total', usage.total_tokens],
      ['cached', usage.cached_input_tokens],
      ['reasoning', usage.reasoning_tokens],
    ] as const;
    for (const [key, value] of fields) {
      if (typeof value === 'number' && Number.isFinite(value)) totals[key] = (totals[key] ?? 0) + value;
    }
  }
  return totals;
}

export function addUsageOnce(
  totals: TokenUsageTotals,
  seenRequestIds: Set<string>,
  requestId: string,
  usage: ProviderUsageLike,
): TokenUsageTotals {
  if (seenRequestIds.has(requestId)) return totals;
  seenRequestIds.add(requestId);
  if (!Object.values(usage).some(value => typeof value === 'number' && Number.isFinite(value))) return totals;
  const delta = tokenUsageTotals([{source: 'provider_reported', usage}]);
  return Object.fromEntries((Object.keys(totals) as (keyof TokenUsageTotals)[]).map(key => [
    key,
    delta[key] === null ? totals[key] : (totals[key] ?? 0) + delta[key]!,
  ])) as TokenUsageTotals;
}

export function isCurrentThreadLoad(currentRequest: number, request: number): boolean {
  return currentRequest === request;
}

export function compactionWindow<T>(messages: T[], recentCount: number): {summarize: T[]; retain: T[]} {
  const boundary = Math.max(0, messages.length - Math.max(0, recentCount));
  return {summarize: messages.slice(0, boundary), retain: messages.slice(boundary)};
}
