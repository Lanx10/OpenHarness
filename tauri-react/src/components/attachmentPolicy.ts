export type AttachmentKind = 'image' | 'text';
export type AttachmentValidation = {kind: AttachmentKind; mime: string} | {error: string};

const imageTypes: Record<string, string> = {png: 'image/png', jpg: 'image/jpeg', jpeg: 'image/jpeg', webp: 'image/webp', gif: 'image/gif'};
const textExtensions = new Set(['txt', 'md', 'json', 'ts', 'tsx', 'js', 'jsx', 'rs', 'py', 'css', 'html', 'xml', 'yaml', 'yml', 'toml', 'csv', 'sh', 'sql']);
export const MAX_IMAGE_BYTES = 4 * 1024 * 1024;
export const MAX_TEXT_BYTES = 256 * 1024;
export const MAX_ATTACHMENTS = 10;
export const MAX_TOTAL_BYTES = 12 * 1024 * 1024;

export function inspectAttachment(name: string, declaredType: string, bytes: Uint8Array): AttachmentValidation {
  const extension = name.split('.').pop()?.toLowerCase() ?? '';
  if (extension === 'svg' || declaredType === 'image/svg+xml') return {error: `Unsupported image: ${name}. SVG is not supported.`};
  const imageType = imageTypes[extension];
  if (imageType) {
    if (bytes.length > MAX_IMAGE_BYTES) return {error: `Attachment too large: ${name} (max 4MB)`};
    const detected = bytes[0] === 0x89 && bytes.slice(1, 4).toString() === '80,78,71' ? 'image/png'
      : bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff ? 'image/jpeg'
      : String.fromCharCode(...bytes.slice(0, 6)) === 'GIF87a' || String.fromCharCode(...bytes.slice(0, 6)) === 'GIF89a' ? 'image/gif'
      : bytes.length >= 12 && String.fromCharCode(...bytes.slice(0, 4)) === 'RIFF' && String.fromCharCode(...bytes.slice(8, 12)) === 'WEBP' ? 'image/webp' : '';
    return detected === imageType ? {kind: 'image', mime: imageType} : {error: `Invalid or unsupported image: ${name}.`};
  }
  if (!textExtensions.has(extension)) return {error: `Unsupported file type: ${name}.`};
  if (bytes.length > MAX_TEXT_BYTES) return {error: `Attachment too large: ${name} (max 256KB)`};
  try {
    new TextDecoder('utf-8', {fatal: true}).decode(bytes);
  } catch {
    return {error: `File is not valid UTF-8 text: ${name}.`};
  }
  return {kind: 'text', mime: 'text/plain'};
}
