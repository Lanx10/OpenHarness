export const DEFAULT_BROWSER_URL = 'http://localhost:5173';

function validateHttpUrl(value: string): string {
  let parsed: URL;
  try { parsed = new URL(value); }
  catch { throw new Error('Enter a valid HTTP or HTTPS address.'); }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
    throw new Error('Only HTTP and HTTPS addresses are supported.');
  }
  if (parsed.username || parsed.password) throw new Error('Addresses with embedded credentials are not supported.');
  const normalized = parsed.toString();
  return value.endsWith('/') && parsed.pathname === '/' && !parsed.search && !parsed.hash
    ? normalized
    : parsed.pathname === '/' && !parsed.search && !parsed.hash ? normalized.slice(0, -1) : normalized;
}

export function normalizeBrowserAddress(input: string): string {
  const value = input.trim();
  if (!value) return DEFAULT_BROWSER_URL;
  if (/^https?:\/\//i.test(value)) return validateHttpUrl(value);
  if (/^(localhost|127(?:\.\d{1,3}){3}|\[::1\]|\d{1,3}(?:\.\d{1,3}){3})(:\d+)?(\/.*)?$/i.test(value)) {
    return validateHttpUrl(`http://${value}`);
  }
  if (/^[a-z][a-z0-9+.-]*:/i.test(value)) throw new Error('Only HTTP and HTTPS addresses are supported.');
  if (/^[a-z0-9-]+(?:\.[a-z0-9-]+)+(?:\:\d+)?(?:\/.*)?$/i.test(value)) return validateHttpUrl(`https://${value}`);
  return `${DEFAULT_SEARCH_URL}?q=${encodeURIComponent(value)}`;
}

const DEFAULT_SEARCH_URL = 'https://duckduckgo.com/';
