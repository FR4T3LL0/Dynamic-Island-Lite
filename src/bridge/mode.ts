/** A detected native host must never fall back to demonstration data. */
export function bridgeMode(host: object): 'native' | 'preview' {
  return '__TAURI_INTERNALS__' in host ? 'native' : 'preview';
}
