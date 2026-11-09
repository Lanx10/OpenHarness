import type { StepEventLike } from './StepEvent';

export function upsertStep(steps: StepEventLike[], step: StepEventLike): StepEventLike[] {
  const index = steps.findIndex(item => item.id === step.id);
  if (index === -1) return [...steps, step];
  return steps.map((item, position) => position === index ? step : item);
}
