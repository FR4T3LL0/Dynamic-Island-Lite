import { invoke } from '@tauri-apps/api/core';
import { bridgeMode } from './mode';
import { mediaPosition } from './media';
import { defaultSettings, emptyDisplay, emptyMediaState, errorMessage, initialTimer, type Settings, type ModuleId, type Snapshot, type TimerInfo, type AudioInfo, type AppFailure, type DisplayState } from './contracts';

type View = { snapshot: Snapshot; loading: boolean; error: string | null; activity: string | null; mediaReceivedAtMs: number };
export const mode = bridgeMode(window);
const empty: Snapshot = { revision: 0, settings: defaultSettings, autostart: null, shortcut: { registered: null, error: null }, display: emptyDisplay, media: null, mediaState: emptyMediaState, audio: null, audioGeneration: 0, audioError: null, timerRevision: 0, timer: null, capabilities: [], warning: null, readOnly: false };
let view: View = { snapshot: empty, loading: true, error: null, activity: null, mediaReceivedAtMs: 0 };
const moduleRevisions = { timer: 0, settings: 0, audio: 0, media: 0, display: 0 };
let activityTimeout: ReturnType<typeof setTimeout> | undefined;
let lastExpiryRevision = -1;
let previewTimerInterval: ReturnType<typeof setInterval> | undefined;
const listeners = new Set<() => void>();
function publish(next: View) {
  const previous = view.snapshot.media;
  const media = next.snapshot.media;
  const mediaPeek = !view.loading && !next.loading && next.snapshot.settings.peek
    && media?.playing && (!previous?.playing || previous.title !== media.title || previous.id !== media.id);
  view = next;
  for (const listener of listeners) listener();
  if (mediaPeek) window.dispatchEvent(new Event('island:peek'));
}
export const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener); }; };
export const getView = () => view;
export function clearError() { publish({ ...view, error: null }); }
export function reportError(error: unknown) { publish({ ...view, loading: false, error: errorMessage(error) }); }
function timerExpired(revision: number) {
  if (!Number.isSafeInteger(revision) || revision <= lastExpiryRevision) return;
  lastExpiryRevision = revision;
  if (view.snapshot.timer?.status !== 'completed' || !view.snapshot.timer.notified) return;
  if (activityTimeout) clearTimeout(activityTimeout);
  publish({ ...view, activity: 'Timer abgelaufen' });
  window.dispatchEvent(new Event('island:peek'));
  activityTimeout = setTimeout(() => publish({ ...view, activity: null }), 4_000);
}
function acceptSnapshot(snapshot: Snapshot, preserveError = false) {
  const acceptMedia = snapshot.mediaState.generation > view.snapshot.mediaState.generation || (snapshot.mediaState.generation === view.snapshot.mediaState.generation && snapshot.revision >= moduleRevisions.media);
  const combined = { ...snapshot,
    display: snapshot.revision < moduleRevisions.display ? view.snapshot.display : snapshot.display,
    settings: snapshot.revision < moduleRevisions.settings ? view.snapshot.settings : snapshot.settings,
    shortcut: snapshot.revision < moduleRevisions.settings ? view.snapshot.shortcut : snapshot.shortcut,
    autostart: snapshot.revision < moduleRevisions.settings ? view.snapshot.autostart : snapshot.autostart,
    timer: snapshot.revision < moduleRevisions.timer ? view.snapshot.timer : snapshot.timer,
    timerRevision: snapshot.revision < moduleRevisions.timer ? view.snapshot.timerRevision : snapshot.timerRevision,
    audio: snapshot.revision < moduleRevisions.audio ? view.snapshot.audio : snapshot.audio,
    audioGeneration: snapshot.revision < moduleRevisions.audio ? view.snapshot.audioGeneration : snapshot.audioGeneration,
    audioError: snapshot.revision < moduleRevisions.audio ? view.snapshot.audioError : snapshot.audioError,
    media: acceptMedia ? snapshot.media : view.snapshot.media,
    mediaState: acceptMedia ? snapshot.mediaState : view.snapshot.mediaState,
    revision: Math.max(snapshot.revision, view.snapshot.revision),
  };
  moduleRevisions.timer = Math.max(moduleRevisions.timer, snapshot.revision);
  moduleRevisions.settings = Math.max(moduleRevisions.settings, snapshot.revision);
  moduleRevisions.audio = Math.max(moduleRevisions.audio, snapshot.revision);
  moduleRevisions.display = Math.max(moduleRevisions.display, snapshot.revision);
  if (acceptMedia) moduleRevisions.media = snapshot.revision;
  publish({ ...view, snapshot: combined, loading: false, error: preserveError ? view.error : null, mediaReceivedAtMs: acceptMedia ? performance.now() : view.mediaReceivedAtMs });
}

let initialization: Promise<void> | undefined;
export function initialize(): Promise<void> {
  initialization ??= (async () => {
    try {
      if (mode === 'native') {
        const { listen } = await import('@tauri-apps/api/event');
        await listen<{display:DisplayState;revision:number}>('display-updated',({payload})=>{
          if(payload.revision < moduleRevisions.display)return;
          moduleRevisions.display=payload.revision;
          publish({...view,snapshot:{...view.snapshot,display:payload.display,revision:Math.max(view.snapshot.revision,payload.revision)}});
        });
        await listen<{ audio: AudioInfo | null; generation: number; revision: number; error: AppFailure | null }>('audio-updated', ({ payload }) => {
          if (payload.generation < view.snapshot.audioGeneration || payload.revision < moduleRevisions.audio) return;
          moduleRevisions.audio = payload.revision;
          publish({ ...view, snapshot: { ...view.snapshot, audio: payload.audio, audioGeneration: payload.generation, audioError: payload.error, revision: Math.max(payload.revision, view.snapshot.revision) } });
        });
        await listen<{view: Pick<Snapshot, 'media' | 'mediaState'>; revision: number}>('media-updated', ({payload}) => {
          const generation = payload.view.mediaState.generation, current = view.snapshot.mediaState.generation;
          if (generation < current || (generation === current && payload.revision < moduleRevisions.media)) return;
          moduleRevisions.media = payload.revision;
          publish({...view, mediaReceivedAtMs: performance.now(), snapshot: {...view.snapshot, ...payload.view, revision: Math.max(payload.revision, view.snapshot.revision)}});
        });
        await listen<{ timer: TimerInfo; timerRevision: number; revision: number }>('timer-updated', ({ payload }) => {
          if (payload.revision < moduleRevisions.timer) return;
          moduleRevisions.timer = payload.revision;
          publish({ ...view, snapshot: { ...view.snapshot, timer: payload.timer, timerRevision: payload.timerRevision, revision: Math.max(payload.revision, view.snapshot.revision) } });
        });
        await listen<{revision: number}>('timer-expired', ({payload}) => timerExpired(payload.revision));
        await listen('service-error', ({ payload }) => reportError(payload));
        await listen<{ settings: Settings; shortcut: Snapshot['shortcut']; autostart: Snapshot['autostart']; revision: number }>('settings-changed', ({ payload }) => {
          if (payload.revision < moduleRevisions.settings) return;
          moduleRevisions.settings = payload.revision;
          publish({ ...view, snapshot: { ...view.snapshot, settings: payload.settings, shortcut: payload.shortcut, autostart: payload.autostart, revision: Math.max(payload.revision, view.snapshot.revision) } });
        });
        await listen('shell-toggle', () => window.dispatchEvent(new CustomEvent('island:toggle')));
        await listen('shell-open', () => window.dispatchEvent(new CustomEvent('island:open')));
        await listen('shell-close', () => window.dispatchEvent(new CustomEvent('island:close')));
        await listen('shell-peek', () => window.dispatchEvent(new Event('island:edge-peek')));
        await listen<boolean>('shell-fullscreen', ({payload}) => window.dispatchEvent(new CustomEvent('island:fullscreen', {detail:payload})));
        acceptSnapshot(await invoke<Snapshot>('get_snapshot'));
      } else {
        const { previewSnapshot } = await import('./preview');
        acceptSnapshot(previewSnapshot());
      }
    } catch (error) { reportError(error); }
  })();
  return initialization;
}

let settingsQueue: Promise<void> = Promise.resolve();
export function saveSettings(settings: Settings): Promise<void> {
  const patch = Object.fromEntries(Object.entries(settings).filter(([key, value]) => JSON.stringify(value) !== JSON.stringify(view.snapshot.settings[key as keyof Settings])));
  return queueSettings(patch);
}
function queueSettings(patch: Partial<Settings>): Promise<void> {
  const operation = settingsQueue.then(async () => {
    try {
      const rebased = { ...view.snapshot.settings, ...patch };
      const confirmed = mode === 'native' ? await invoke<{ settings: Settings; shortcut: Snapshot['shortcut']; autostart: Snapshot['autostart']; revision: number }>('save_settings', { settings: rebased }) : { settings: structuredClone(rebased), shortcut: view.snapshot.shortcut, autostart: view.snapshot.autostart, revision: view.snapshot.revision + 1 };
      if (confirmed.revision >= moduleRevisions.settings) {
        moduleRevisions.settings = confirmed.revision;
        publish({ ...view, snapshot: { ...view.snapshot, settings: confirmed.settings, shortcut: confirmed.shortcut, autostart: confirmed.autostart, revision: Math.max(confirmed.revision, view.snapshot.revision) }, error: null });
      }
    } catch (error) { if (mode === 'native') { try { acceptSnapshot(await invoke<Snapshot>('get_snapshot'), true); } catch { /* Preserve the original settings failure. */ } } reportError(error); throw error; }
  });
  settingsQueue = operation.catch(() => {});
  return operation;
}

export async function perform(command: string, args: Record<string, unknown> = {}): Promise<boolean> {
  try {
    if (mode === 'native') {
      await invoke(command, args);
      acceptSnapshot(await invoke<Snapshot>('get_snapshot'));
      return true;
    }
    const snapshot = structuredClone(view.snapshot);
    const mediaCommand = command.startsWith('media_') || command === 'refresh_media';
    if (mediaCommand) {
      snapshot.mediaState.sessions = snapshot.mediaState.sessions.map(session => ({...session, position: mediaPosition(session, view.mediaReceivedAtMs, performance.now())}));
      snapshot.media = snapshot.mediaState.sessions.find(session => session.id === snapshot.mediaState.selectedId) ?? null;
    }
    switch (command) {
      case 'media_toggle': if (snapshot.media?.canToggle) snapshot.media.playing = !snapshot.media.playing; break;
      case 'media_seek': if (snapshot.media?.canSeek) snapshot.media.position = Number(args.position); break;
      case 'media_previous': case 'media_next': case 'refresh_media': break;
      case 'set_volume': if (snapshot.audio) snapshot.audio.volume = Number(args.volume); break;
      case 'refresh_displays': break;
      case 'timer_configure': {
        if (snapshot.timer?.status === 'running') throw new Error('Bitte den laufenden Timer zuerst pausieren.');
        const seconds = Number(args.seconds);
        if (!Number.isInteger(seconds) || seconds < 1 || seconds > 86_400) throw new Error('Der Countdown muss zwischen einer Sekunde und 24 Stunden liegen.');
        const pomodoro = args.mode === 'pomodoro';
        snapshot.timer = { ...initialTimer, mode: pomodoro ? 'pomodoro' : 'countdown', durationMs: pomodoro ? 1_500_000 : seconds * 1000, remainingMs: pomodoro ? 1_500_000 : seconds * 1000 };
        break;
      }
      case 'timer_start': {
        const timer = snapshot.timer;
        if (!timer || timer.status === 'running') throw new Error('Der Timer läuft bereits.');
        if (timer.status === 'completed') {
          if (timer.mode === 'pomodoro') {
            timer.phase = timer.phase === 'work' ? timer.completedWork % 4 === 0 ? 'long' : 'short' : 'work';
            timer.durationMs = timer.phase === 'work' ? 1_500_000 : timer.phase === 'short' ? 300_000 : 900_000;
          }
          timer.remainingMs = timer.durationMs;
        }
        timer.status = 'running'; timer.notified = false; timer.deadlineUtcMs = Date.now() + timer.remainingMs;
        if (previewTimerInterval) clearInterval(previewTimerInterval);
        previewTimerInterval = setInterval(() => {
          if (!view.snapshot.timer || view.snapshot.timer.status !== 'running') return;
          const state = structuredClone(view.snapshot);
          const timer = state.timer!;
          timer.remainingMs = Math.max(0, timer.deadlineUtcMs! - Date.now());
          const expired = timer.remainingMs === 0;
          if (expired) {
            timer.status = 'completed'; timer.deadlineUtcMs = null; timer.notified = true;
            if (timer.mode === 'pomodoro' && timer.phase === 'work') timer.completedWork++;
            clearInterval(previewTimerInterval); previewTimerInterval = undefined;
          }
          state.revision++; publish({ ...view, snapshot: state });
          if (expired) timerExpired(state.revision);
        }, 250);
        break;
      }
      case 'timer_pause':
        if (snapshot.timer) { snapshot.timer.remainingMs = Math.max(0, (snapshot.timer.deadlineUtcMs ?? Date.now()) - Date.now()); snapshot.timer.status = 'paused'; snapshot.timer.deadlineUtcMs = null; }
        clearInterval(previewTimerInterval); previewTimerInterval = undefined; break;
      case 'timer_reset':
        if (snapshot.timer) snapshot.timer = { ...initialTimer, mode: snapshot.timer.mode, durationMs: snapshot.timer.mode === 'pomodoro' ? 1_500_000 : snapshot.timer.durationMs, remainingMs: snapshot.timer.mode === 'pomodoro' ? 1_500_000 : snapshot.timer.durationMs };
        clearInterval(previewTimerInterval); previewTimerInterval = undefined; break;
      default: throw new Error('Diese Aktion benötigt die Windows-App.');
    }
    snapshot.revision++;
    if (command.startsWith('timer_')) snapshot.timerRevision++;
    if (mediaCommand) snapshot.mediaState.revision++;
    publish({ ...view, snapshot, error: null, mediaReceivedAtMs: mediaCommand ? performance.now() : view.mediaReceivedAtMs });
    return true;
  } catch (error) {
    const mediaAction = command.startsWith('media_') || command === 'refresh_media';
    if (mode === 'native' && mediaAction) {
      try { acceptSnapshot(await invoke<Snapshot>('get_snapshot')); if (mediaAction && view.snapshot.mediaState.error) return false; } catch { /* Keep the original action error. */ }
    }
    reportError(error); return false;
  }
}

export async function setNativeShell(shell: string, module?: ModuleId | 'media-player') {
  if (mode === 'native') { try { await invoke('set_shell', { mode: shell, module }); } catch (error) { reportError(error); } }
}
export async function desktopReady() {
  if (mode === 'native') { try { await invoke('desktop_ready'); } catch (error) { reportError(error); } }
}
export async function showSettings() {
  if (mode === 'native') { try { await invoke('open_settings'); } catch (error) { reportError(error); } }
  else window.open('/#settings', 'island-settings', 'width=640,height=640');
}
