import { useEffect, useState, type CSSProperties, type ReactNode } from 'react';
import { Music2, Pause, Play, SkipBack, SkipForward } from 'lucide-react';
import type { AudioInfo, CoverTransition, MediaInfo, Snapshot } from '../../bridge/contracts';
import { mediaPosition } from '../../bridge/media';
import { perform } from '../../bridge/store';
import { EmptyState, IconButton } from '../../components/Primitives';
import { VisualizerBars } from './VisualizerBars';
import { VolumeControl } from './VolumeControl';
import { MediaArtwork } from './MediaArtwork';

export const formatTime = (seconds: number) => {
  const value = Math.floor(Math.max(0, Number.isFinite(seconds) ? seconds : 0));
  const minutes = Math.floor(value / 60), remainder = String(value % 60).padStart(2, '0');
  return minutes >= 60 ? `${Math.floor(minutes / 60)}:${String(minutes % 60).padStart(2, '0')}:${remainder}` : `${minutes}:${remainder}`;
};
function MediaControls({ media, generation, receivedAtMs, visualizer, audio, coverTransition }: { media: MediaInfo; generation: number; receivedAtMs: number; visualizer: ReactNode; audio: AudioInfo | null; coverTransition: CoverTransition }) {
  const [now, setNow] = useState(() => performance.now());
  const [draft, setDraft] = useState<number | null>(null);
  const [pending, setPending] = useState(false);
  useEffect(() => {
    if (!media.playing) return;
    let timer: ReturnType<typeof setInterval> | undefined;
    const visibility = () => {
      clearInterval(timer);
      if (!document.hidden) timer = setInterval(() => setNow(performance.now()), 250);
    };
    visibility(); document.addEventListener('visibilitychange', visibility);
    return () => { clearInterval(timer); document.removeEventListener('visibilitychange', visibility); };
  }, [media.playing]);
  const position = mediaPosition(media, receivedAtMs, now);
  const busy = pending;
  const action = async (command: string, extra: Record<string, unknown> = {}) => {
    setPending(true);
    await perform(command, { id: media.id, generation, ...extra });
    setDraft(null); setPending(false);
  };
  const seek = (value: number) => { if (draft !== null && media.canSeek && !busy) void action('media_seek', { position: value }); };
  const max = media.canSeek ? media.seekEnd : media.duration || 1;
  const min = media.canSeek ? media.seekStart : 0;
  const value = Math.max(min, Math.min(max, draft ?? position));
  return <div className="media-view" aria-busy={busy}>
    <div className="player-artwork"><MediaArtwork source={media.cover} transition={coverTransition} /></div>
    <div className="media-track"><div className="item-copy"><strong title={media.title}>{media.title || 'Unbekannter Titel'}</strong><span title={media.artist || media.app}>{media.artist || media.app}</span></div>{visualizer}</div>
    <div className="transport"><IconButton icon={SkipBack} label="Voriger Titel" disabled={!media.canPrevious || busy} onClick={() => void action('media_previous')} /><button className="play-button" aria-label={media.playing ? 'Pausieren' : 'Abspielen'} disabled={!media.canToggle || busy} onClick={() => void action('media_toggle')}>{media.playing ? <Pause size={27} fill="currentColor" /> : <Play size={27} fill="currentColor" />}</button><IconButton icon={SkipForward} label="Nächster Titel" disabled={!media.canNext || busy} onClick={() => void action('media_next')} /></div>
    <div className="player-timeline">
    <input aria-label="Wiedergabeposition" aria-valuetext={`${formatTime(draft ?? position)} von ${formatTime(media.duration)}`} type="range" min={min} max={max} step={0.1} value={value} disabled={!media.canSeek || busy}
      style={{ '--media-progress': `${100 * (value - min) / Math.max(1, max - min)}%` } as CSSProperties}
      onChange={event => setDraft(Number(event.target.value))} onPointerUp={event => seek(Number(event.currentTarget.value))}
      onKeyUp={event => { if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown', 'Home', 'End', 'PageUp', 'PageDown'].includes(event.key)) seek(Number(event.currentTarget.value)); }} onPointerCancel={() => setDraft(null)} />
    <div className="time-labels"><span data-testid="media-position">{formatTime(draft ?? position)}</span><span title="Gesamtdauer">{media.duration ? formatTime(media.duration) : '–:–'}</span></div>
    </div>
    {audio?.defaultDevice ? <VolumeControl key={audio.defaultDevice} audio={audio} /> : null}
  </div>;
}
export function MediaPanel({ snapshot, receivedAtMs = 0, visualizerControl }: { snapshot: Snapshot; receivedAtMs?: number; visualizerControl?: ReactNode }) {
  const state = snapshot.mediaState;
  return <>
    <div className="module-scroll">
    {state.error ? <div className="module-error" role="alert"><p title={state.error.message}>{state.error.message}</p><button className="secondary-button" onClick={() => void perform('refresh_media')}>Erneut laden</button></div> : null}
    {snapshot.media ? <MediaControls key={`${state.generation}:${snapshot.media.id}`} media={snapshot.media} generation={state.generation} receivedAtMs={receivedAtMs} visualizer={snapshot.settings.visualizer ? visualizerControl ?? (snapshot.media.playing ? <VisualizerBars /> : null) : null} audio={snapshot.audio} coverTransition={snapshot.settings.reducedMotion ? 'off' : snapshot.settings.coverTransition} /> : <EmptyState icon={Music2}>Keine aktive Mediensitzung.</EmptyState>}
    </div>
  </>;
}
