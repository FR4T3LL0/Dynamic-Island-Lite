import { describe, expect, it } from 'vitest';
import { bridgeMode } from './mode';

describe('Datenquellen', () => {
  it('erlaubt Beispieldaten nur ohne nativen Host', () => {
    expect(bridgeMode({})).toBe('preview');
    expect(bridgeMode({ __TAURI_INTERNALS__: {} })).toBe('native');
    expect(bridgeMode({ __TAURI_INTERNALS__: undefined })).toBe('native');
  });
});
