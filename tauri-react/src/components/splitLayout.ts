export const MIN_CONVERSATION_WIDTH = 480;
export const MIN_SECONDARY_PANEL_WIDTH = 280;
export const PANEL_GAP = 1;
export const CHAT_PANEL_GAP = 18;

export function getSplitLayout(
  canvasWidth: number,
  requested: boolean,
  fileWidth: number,
  gitWidth: number,
) {
  const usableWidth = Number.isFinite(canvasWidth) ? Math.max(0, Math.floor(canvasWidth)) : 0;
  const canSplit = requested && usableWidth >= MIN_CONVERSATION_WIDTH + MIN_SECONDARY_PANEL_WIDTH * 2 + PANEL_GAP;
  if (!canSplit) return {enabled: false, filesWidth: fileWidth, gitWidth, sidePanelWidth: 0};

  const panelBudget = usableWidth - MIN_CONVERSATION_WIDTH - PANEL_GAP;
  const safeFileWidth = Number.isFinite(fileWidth) ? Math.max(MIN_SECONDARY_PANEL_WIDTH, fileWidth) : 420;
  const safeGitWidth = Number.isFinite(gitWidth) ? Math.max(MIN_SECONDARY_PANEL_WIDTH, gitWidth) : 420;
  const baseWidth = safeFileWidth + safeGitWidth;
  const scale = Math.min(1, panelBudget / baseWidth);
  const filesWidth = Math.max(MIN_SECONDARY_PANEL_WIDTH, Math.floor(safeFileWidth * scale));
  const panelsWidth = Math.min(panelBudget, filesWidth + Math.max(MIN_SECONDARY_PANEL_WIDTH, Math.floor(safeGitWidth * scale)));
  const adjustedGitWidth = panelsWidth - filesWidth;
  return {enabled: true, filesWidth, gitWidth: adjustedGitWidth, sidePanelWidth: panelsWidth};
}
