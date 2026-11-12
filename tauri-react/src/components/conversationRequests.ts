export type ConversationRequest = {generation: number; projectId: string | null; threadId: string | null};

export function createRequestOwner() {
  let generation = 0;
  return {
    begin(projectId: string | null, threadId: string | null): ConversationRequest {
      return {generation: ++generation, projectId, threadId};
    },
    invalidate(): void {
      generation++;
    },
    isCurrent(request: ConversationRequest, projectId: string | null, threadId: string | null): boolean {
      return request.generation === generation && request.projectId === projectId && request.threadId === threadId;
    },
  };
}
