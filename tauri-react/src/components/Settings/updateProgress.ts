export type DownloadProgress = {downloaded: number; total: number | null; percent: number | null};

export function createDownloadProgress() {
  let downloaded = 0;
  let total: number | null = null;
  return {
    add(chunkLength: number, contentLength?: number): DownloadProgress {
      if (Number.isFinite(chunkLength) && chunkLength > 0) downloaded += chunkLength;
      if (Number.isFinite(contentLength) && contentLength! > 0) total = contentLength!;
      return {downloaded, total, percent: total === null ? null : Math.min(100, Math.max(0, downloaded / total * 100))};
    },
  };
}
