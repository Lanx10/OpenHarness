export function isNearBottom(element: Pick<HTMLElement, 'scrollTop' | 'clientHeight' | 'scrollHeight'>, threshold = 72): boolean {
  return element.scrollHeight - element.scrollTop - element.clientHeight <= threshold;
}
