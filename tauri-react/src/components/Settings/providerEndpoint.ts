export type EndpointClassification =
  | {kind: 'invalid'; message: string}
  | {kind: 'local_http'; label: 'HTTP (Local)'}
  | {kind: 'local_https'; label: 'HTTPS (Local)'}
  | {kind: 'remote_http'; label: 'HTTP (Unencrypted)'}
  | {kind: 'remote_https'; label: 'HTTPS (Remote)'};

export function classifyProviderEndpoint(value: string): EndpointClassification {
  if (!value.trim()) return {kind: 'invalid', message: 'Enter an endpoint URL.'};
  try {
    const url = new URL(value.trim());
    if (!['http:', 'https:'].includes(url.protocol)) return {kind: 'invalid', message: 'Endpoint must use HTTP or HTTPS.'};
    if (!url.hostname || url.username || url.password) return {kind: 'invalid', message: 'Endpoint must have a hostname and must not contain embedded credentials.'};
    const host = url.hostname.toLowerCase().replace(/^\[|\]$/g, '');
    const local = host === 'localhost' || host.endsWith('.localhost') || host === '::1' || (host.startsWith('127.') && host.split('.').length === 4 && host.split('.').every(part => /^\d{1,3}$/.test(part) && Number(part) <= 255));
    if (url.protocol === 'http:') return local ? {kind: 'local_http', label: 'HTTP (Local)'} : {kind: 'remote_http', label: 'HTTP (Unencrypted)'};
    return local ? {kind: 'local_https', label: 'HTTPS (Local)'} : {kind: 'remote_https', label: 'HTTPS (Remote)'};
  } catch {
    return {kind: 'invalid', message: 'Endpoint must be a valid HTTP(S) URL with a valid port.'};
  }
}
