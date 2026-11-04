export const MODULE_IDS = ['media', 'timer'] as const;
export type ModuleId = typeof MODULE_IDS[number];
export type CoverTransition = 'fade' | 'slide' | 'zoom' | 'off';
export interface Settings {
  schemaVersion: number; appearance: 'capsule' | 'notch'; width: number; topGap: number;
  accent: string; reducedMotion: boolean; monitor: string | null; shortcut: string;
  clockFollowsIsland: boolean;
  peek: boolean;
  edgeDelayMs: number;
  smart: boolean;
  fixedAppbar: boolean;
  visualizer: boolean;
  mediaColorAccent: boolean;
  coverTransition: CoverTransition;
  autostart: boolean; timerSound: boolean;
}
export const defaultSettings: Settings = {
  schemaVersion: 1, appearance: 'capsule', width: 180, topGap: 12, accent: '#2997FF',
  reducedMotion: false, monitor: null, shortcut: 'Ctrl+Alt+Shift+I',
  autostart: true, timerSound: true,
  clockFollowsIsland: false,
  peek: false,
  edgeDelayMs: 300,
  smart: true,
  fixedAppbar: false,
  visualizer: true,
  mediaColorAccent: true,
  coverTransition: 'fade',
};
export interface MediaInfo {
  id: string; app: string; title: string; artist: string; playing: boolean;
  position: number; duration: number; rate: number; seekStart: number; seekEnd: number;
  canSeek: boolean; canToggle: boolean; canPrevious: boolean; canNext: boolean; cover: string | null;
  coverTint?: [number, number, number] | null;
}
export interface MediaStatus { enabled: boolean; generation: number; revision: number; selectedId: string | null; sessions: MediaInfo[]; error: AppFailure | null }
export const emptyMediaState: MediaStatus = { enabled: false, generation: 0, revision: 0, selectedId: null, sessions: [], error: null };
export interface AudioInfo { volume: number; muted: boolean; defaultDevice: string; devices: { id: string; name: string }[] }
export interface TimerInfo {
  schemaVersion: number; mode: 'countdown' | 'pomodoro'; phase: 'work' | 'short' | 'long';
  status: 'idle' | 'running' | 'paused' | 'completed'; durationMs: number; remainingMs: number;
  deadlineUtcMs: number | null; completedWork: number; notified: boolean;
}
export const initialTimer: TimerInfo = { schemaVersion: 1, mode: 'countdown', phase: 'work', status: 'idle', durationMs: 1_500_000,
  remainingMs: 1_500_000, deadlineUtcMs: null, completedWork: 0, notified: false };
export interface Snapshot {
  autostart: { registered: boolean; error: AppFailure | null } | null;
  shortcut: ShortcutStatus;
  display: DisplayState;
  revision: number; settings: Settings;
  media: MediaInfo | null; audio: AudioInfo | null; timer: TimerInfo | null;
  mediaState: MediaStatus;
  audioGeneration: number; audioError: AppFailure | null;
  timerRevision: number;
  capabilities: string[]; warning: string | null; readOnly: boolean;
}
export interface ShortcutStatus { registered: string | null; error: AppFailure | null }
export interface PhysicalRect { left: number; top: number; right: number; bottom: number }
export interface MonitorInfo { id: string; aliases: string[]; name: string; device: string; bounds: PhysicalRect; workArea: PhysicalRect; scale: number; primary: boolean }
export interface DisplayState { monitors: MonitorInfo[]; selectedId: string | null; fallback: boolean; fullscreen: boolean; smartCovered: boolean; error: AppFailure | null }
export const emptyDisplay: DisplayState = { monitors: [], selectedId: null, fallback: false, fullscreen: false, smartCovered: false, error: null };
export interface AppFailure { code: string; message: string; retryable: boolean }
export function errorMessage(error: unknown): string {
  if (typeof error === 'object' && error !== null && 'message' in error && typeof error.message === 'string') return error.message;
  return typeof error === 'string' ? error : 'Die Aktion konnte nicht ausgeführt werden.';
}
