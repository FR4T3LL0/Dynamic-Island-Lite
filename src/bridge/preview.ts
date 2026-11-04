import { defaultSettings, initialTimer, type Snapshot, type MediaInfo } from './contracts';

/** Deliberately isolated, browser-only demonstration data. */
export function previewSnapshot(): Snapshot {
  const media: MediaInfo = { id: 'preview-player', app: 'Beispielplayer', title: 'Abendlicht', artist: 'Lokale Testmedien', playing: false, position: 58, duration: 204, rate: 1, seekStart: 0, seekEnd: 204, canSeek: true, canToggle: true, canPrevious: false, canNext: false, cover: null };
  return {
    revision: 0, settings: structuredClone(defaultSettings),
    shortcut: { registered: null, error: null },
    autostart: null,
    display: { monitors: [{ id: 'preview-display', aliases: [], name: 'Beispielmonitor', device: 'preview', bounds: { left: 0, top: 0, right: 1920, bottom: 1080 }, workArea: { left: 0, top: 0, right: 1920, bottom: 1040 }, scale: 1, primary: true }], selectedId: 'preview-display', fallback: false, fullscreen: false, smartCovered: false, error: null },
    media, mediaState: { enabled: true, generation: 1, revision: 0, selectedId: media.id, sessions: [media], error: null },
    audio: { volume: 42, muted: false, defaultDevice: 'preview-speakers', devices: [{ id: 'preview-speakers', name: 'Lautsprecher' }] },
    audioGeneration: 1, audioError: null, timerRevision: 0,
    timer: structuredClone(initialTimer), capabilities: ['shell', 'settings', 'media', 'audio', 'timer'], warning: null, readOnly: false,
  };
}
