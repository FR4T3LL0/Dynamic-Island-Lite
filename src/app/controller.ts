import { type ModuleId, type Snapshot } from '../bridge/contracts';

export type ShellMode = 'compact' | 'preview' | 'open' | 'hidden';
export interface ShellState {
  mode: ShellMode; active: ModuleId; fullscreen: boolean;
  hiddenReason: 'fullscreen' | 'peek' | 'smart' | null;
  smartCovered: boolean;
  peekEnabled: boolean;
  peekUntil: number | null;
}
export const initialShell: ShellState = {
  mode: 'compact', active: 'media', fullscreen: false,
  peekEnabled: false, peekUntil: null,
  smartCovered: false,
  hiddenReason: null,
};
export type ShellEvent =
  | { type: 'hover' | 'leave' | 'close' | 'player' | 'timer' }
  | { type: 'open' | 'toggle'; module?: ModuleId }
  | { type: 'visibility'; peekEnabled: boolean; smartCovered: boolean }
  | { type: 'peek'; until: number; edge?: boolean }
  | { type: 'peekExpired'; until: number }
  | { type: 'fullscreen'; active: boolean };

function close(state: ShellState): ShellState {
  const reason = state.fullscreen ? 'fullscreen' : state.peekEnabled ? 'peek' : state.smartCovered ? 'smart' : null;
  return { ...state, mode: reason ? 'hidden' : 'compact', hiddenReason: reason, peekUntil: null };
}
function open(state: ShellState, module?: ModuleId): ShellState {
  if (state.mode === 'open') return state;
  return { ...state, mode: 'open', active: module ?? 'media', hiddenReason: null, peekUntil: null };
}
export function shellReducer(state: ShellState, event: ShellEvent): ShellState {
  switch (event.type) {
    case 'visibility': {
      if (event.peekEnabled === state.peekEnabled && event.smartCovered === state.smartCovered) return state;
      const next = { ...state, peekEnabled: event.peekEnabled, smartCovered: event.smartCovered, peekUntil: null };
      return state.mode === 'compact' || state.mode === 'preview' || state.hiddenReason === 'peek' || state.hiddenReason === 'smart'
        ? close(next) : next;
    }
    case 'peek':
      if (!(state.peekEnabled || (event.edge && state.smartCovered)) || state.fullscreen || state.mode === 'open'
        || (state.mode === 'hidden' && state.hiddenReason !== 'peek' && state.hiddenReason !== 'smart')) return state;
      return { ...state, mode: state.mode === 'hidden' ? 'compact' : state.mode, hiddenReason: null, peekUntil: event.until };
    case 'peekExpired':
      if (event.until !== state.peekUntil) return state;
      return close(state);
    case 'hover': return state.mode === 'compact' ? { ...state, mode: 'preview' } : state;
    case 'leave': return state.mode === 'preview' ? { ...state, mode: 'compact' } : state;
    case 'open': return open(state, event.module);
    case 'close': return close(state);
    case 'toggle': return state.mode === 'open' ? close(state) : open(state, event.module);
    case 'player':
    case 'timer': return { ...state, mode: 'open', active: event.type === 'player' ? 'media' : 'timer', hiddenReason: null, peekUntil: null };
    case 'fullscreen': {
      // Repeated observations preserve a deliberate temporary opening.
      if (event.active === state.fullscreen) return state;

      const next = { ...state, fullscreen: event.active };
      return close(next);
    }
  }
}

export function compactPresentation(snapshot: Snapshot, notice: string | null) {
  const media = snapshot.media;
  const music = Boolean(media && (media.playing || media.title || media.cover));
  return {
    label: notice ? notice : music ? media?.title || 'Medienwiedergabe' : 'Dynamic Island Lite',
    music,
    timerMinutes: snapshot.timer?.status === 'running' ? Math.ceil(snapshot.timer.remainingMs / 60000) : null,
  };
}
