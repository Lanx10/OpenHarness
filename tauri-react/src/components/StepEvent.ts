export type StepEventLike = {
  id: string;
  kind: 'thinking' | 'read' | 'grep' | 'edit' | 'command' | 'git' | string;
  label: string;
  status: 'running' | 'done' | 'error' | 'approval';
  detail?: string | null;
};
