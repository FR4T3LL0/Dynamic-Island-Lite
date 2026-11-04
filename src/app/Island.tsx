import { useEffect, useReducer, useRef, useState, useSyncExternalStore, type CSSProperties, type PointerEvent as ReactPointerEvent } from 'react';
import { ArrowRight, ChevronDown, ChevronUp, Music2, Settings2, Timer, X } from 'lucide-react';
import { clearError, desktopReady, getView, mode, saveSettings, setNativeShell, showSettings, subscribe } from '../bridge/store';
import { IconButton } from '../components/Primitives';
import { formatTime, MediaPanel } from '../modules/media/MediaPanel';
import { MediaArtwork } from '../modules/media/MediaArtwork';
import { TimerPanel } from '../modules/timer/TimerPanel';
import { compactPresentation, initialShell, shellReducer } from './controller';
import { useClockTime } from './useClockTime';
import { VisualizerAccentButton, VisualizerBars, useVisualizer } from '../modules/media/VisualizerBars';

const nativeMotionActive = () => mode === 'native' && document.documentElement.hasAttribute('data-shell-motion');
export function Island() {
  const { snapshot, loading, error, activity, mediaReceivedAtMs } = useSyncExternalStore(subscribe, getView);
  const [shell, dispatch] = useReducer(shellReducer, initialShell);
  const hover = useRef<ReturnType<typeof setTimeout> | null>(null);
  const leave = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pointerInside = useRef(false);
  const hoverIntent = useRef(false);
  const lastScreenPoint = useRef<{ x: number; y: number } | null>(null);
  const compactButton = useRef<HTMLButtonElement>(null);
  const previewKeyboardClose = useRef(false);
  const savingAccent = useRef(false);
  const settings = snapshot.settings;
  const smartCovered = settings.smart && snapshot.display.smartCovered;
  const active = shell.active;
  const compact = compactPresentation(snapshot, activity);
  const timerShortcut = compact.timerMinutes !== null || snapshot.timer?.status === 'paused' || Boolean(activity);
  const smartHidden = shell.mode === 'hidden' && shell.hiddenReason === 'smart';
  const displayMode = loading || smartHidden ? 'compact' : shell.mode;
  const showClock = !loading && displayMode !== 'hidden';
  const time = useClockTime(showClock);
  const monitor = snapshot.display.monitors.find(({ id }) => id === snapshot.display.selectedId);
  const canShowBubbles = !loading && displayMode === 'compact'
    && (!monitor || (monitor.workArea.right - monitor.workArea.left) / monitor.scale >= 160);
  const detachedMedia = canShowBubbles && compact.music && timerShortcut;
  const previewing = displayMode === 'preview';
  const launcherVisualizer = settings.visualizer && compact.music && !loading && (displayMode === 'compact' || previewing);
  useVisualizer(Boolean(snapshot.media?.playing && (launcherVisualizer || (settings.visualizer && displayMode === 'open' && active === 'media'))), snapshot.audio?.defaultDevice);
  const mediaTitle = snapshot.media?.title || 'Medienwiedergabe';
  const cover = snapshot.media?.cover ?? null;
  const coverTransition = settings.reducedMotion ? 'off' : settings.coverTransition;
  const colorEnabled = settings.mediaColorAccent;
  const coverTint = colorEnabled ? snapshot.media?.coverTint : undefined;
  const timerDetail = activity ?? (snapshot.timer?.status === 'paused' ? 'Pausiert' : snapshot.timer ? formatTime(Math.ceil(snapshot.timer.remainingMs / 1000)) : '');
  const nativeMode = smartHidden ? 'smart-hidden' : detachedMedia ? 'compact-media' : shell.mode;
  const compactLabel = detachedMedia ? activity ?? (snapshot.timer?.status === 'paused' ? 'Timer pausiert' : 'Timer') : compact.label;
  const inlinePlayer = displayMode === 'open' && active === 'media';
  const liteTimerOpen = displayMode === 'open' && active === 'timer';
  const [transition, setTransition] = useState({ mode: displayMode, liteTimerOpen, closingTimer: false, closingPreview: false });
  if (transition.mode !== displayMode || transition.liteTimerOpen !== liteTimerOpen) {
    setTransition({
      mode: displayMode,
      liteTimerOpen,
      closingTimer: displayMode === 'compact' && transition.liteTimerOpen,
      closingPreview: displayMode === 'compact' && transition.mode === 'preview',
    });
  }
  const closingLiteTimer = transition.closingTimer && !settings.reducedMotion;
  useEffect(() => {
    if (!closingLiteTimer) return;
    const timeout = setTimeout(() => setTransition(current => ({ ...current, closingTimer: false })), 200);
    return () => clearTimeout(timeout);
  }, [closingLiteTimer]);
  const utilityBar = displayMode === 'open';
  const nativeModule = shell.mode === 'open' ? inlinePlayer ? 'media-player' : active : undefined;
  useEffect(() => {
    // Native shell closure restores the previous Windows foreground owner.
    // The browser demo instead needs a live DOM target after its panel unmounts.
    if (mode === 'preview' && displayMode === 'compact' && previewKeyboardClose.current) compactButton.current?.focus();
    previewKeyboardClose.current = false;
  }, [displayMode]);
  useEffect(() => { if (!loading) void desktopReady(); }, [loading]);
  useEffect(() => { dispatch({type:'fullscreen',active:snapshot.display.fullscreen}); }, [snapshot.display.fullscreen]);
  useEffect(() => { dispatch({ type: 'visibility', peekEnabled: settings.peek, smartCovered }); }, [settings.peek, smartCovered]);
  useEffect(() => {
    const until = shell.peekUntil;
    if (until === null) return;
    const timer = setTimeout(() => dispatch({ type: 'peekExpired', until }), Math.max(0, until - performance.now()));
    return () => clearTimeout(timer);
  }, [shell.peekUntil]);

  useEffect(() => {
    if (!loading && shell.peekEnabled === settings.peek && shell.smartCovered === smartCovered) void setNativeShell(nativeMode, nativeModule);
  }, [nativeMode, nativeModule, shell.peekEnabled, shell.smartCovered, loading, settings.peek, smartCovered, settings.fixedAppbar, settings.appearance, settings.width, settings.topGap, settings.monitor, settings.reducedMotion, settings.clockFollowsIsland]);

  useEffect(() => {
    if (shell.mode !== 'compact' && shell.mode !== 'preview') {
      if (hover.current) clearTimeout(hover.current);
      if (leave.current) clearTimeout(leave.current);
      hover.current = null;
      leave.current = null;
      hoverIntent.current = false;
    }
  }, [shell.mode]);
  useEffect(() => {
    if (mode !== 'native' || shell.mode !== 'compact') return;
    const observer = new MutationObserver(() => {
      if (nativeMotionActive()) {
        hoverIntent.current = false;
        if (hover.current) clearTimeout(hover.current);
        hover.current = null;
        return;
      }
      if (document.documentElement.dataset.shellMotionPointerMoved === 'true') hoverIntent.current = true;
      if (!nativeMotionActive() && pointerInside.current && hoverIntent.current && !hover.current) {
        hover.current = setTimeout(() => {
          hover.current = null;
          if (pointerInside.current && hoverIntent.current && !nativeMotionActive()) dispatch({ type: 'hover' });
        }, 180);
      }
    });
    observer.observe(document.documentElement, { attributes: true, attributeFilter: ['data-shell-motion'] });
    return () => observer.disconnect();
  }, [shell.mode]);

  useEffect(() => {
    const toggle = () => dispatch({ type: 'toggle' });
    const open = () => {
      dispatch({ type: 'open' });
      void setNativeShell('open', nativeModule ?? 'media-player');
    };
    window.addEventListener('island:toggle', toggle);
    window.addEventListener('island:open', open);
    return () => {
      window.removeEventListener('island:toggle', toggle);
      window.removeEventListener('island:open', open);
    };
  }, [nativeModule]);

  useEffect(() => {
    const close = () => {
      if (hover.current) clearTimeout(hover.current);
      hover.current = null;
      hoverIntent.current = false;
      dispatch({ type: 'close' });
    };
    const fullscreen = (event: Event) => dispatch({ type: 'fullscreen', active: (event as CustomEvent<boolean>).detail });
    const peek = (event: Event) => {
      const { snapshot: current, loading: pending } = getView();
      const edge = event.type === 'island:edge-peek';
      if (pending || !(current.settings.peek || (edge && current.settings.smart && current.display.smartCovered)) || current.display.fullscreen) return;
      dispatch({ type: 'peek', edge, until: performance.now() + 4000 });
    };
    const keyboard = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { previewKeyboardClose.current = mode === 'preview'; close(); }
      if (mode === 'preview' && event.ctrlKey && event.altKey && event.key.toLowerCase() === 'i' && event.shiftKey) { event.preventDefault(); window.dispatchEvent(new Event('island:toggle')); }
    };
    const outside = (event: PointerEvent) => {
      if (event.target instanceof Element && !event.target.closest('.island, .activity-bubble')) close();
    };
    window.addEventListener('island:close', close);
    window.addEventListener('island:fullscreen', fullscreen);
    window.addEventListener('island:peek', peek);
    window.addEventListener('island:edge-peek', peek);
    window.addEventListener('keydown', keyboard);
    document.addEventListener('pointerdown', outside);
    return () => {
      window.removeEventListener('island:close', close);
      window.removeEventListener('island:fullscreen', fullscreen);
      window.removeEventListener('island:peek', peek);
      window.removeEventListener('island:edge-peek', peek);
      window.removeEventListener('keydown', keyboard); document.removeEventListener('pointerdown', outside);
      if (hover.current) clearTimeout(hover.current); if (leave.current) clearTimeout(leave.current);
    };
  }, []);

  function scheduleHover() {
    pointerInside.current = true;
    if (leave.current) clearTimeout(leave.current);
    if (hover.current) clearTimeout(hover.current);
    hover.current = null;
    if (shell.mode === 'compact' && !nativeMotionActive() && (mode !== 'native' || hoverIntent.current)) hover.current = setTimeout(() => {
      hover.current = null;
      if (pointerInside.current && !nativeMotionActive() && (mode !== 'native' || hoverIntent.current)) dispatch({ type: 'hover' });
    }, 180);
  }
  function pointerEnter(event: ReactPointerEvent<HTMLElement>) {
    // Resize/region changes can produce a fresh enter beneath a stationary
    // cursor. Native hover needs actual movement after a deliberate close.
    if (mode === 'native' && !lastScreenPoint.current && Number.isFinite(event.screenX) && Number.isFinite(event.screenY)) {
      lastScreenPoint.current = { x: event.screenX, y: event.screenY };
      if (!nativeMotionActive()) hoverIntent.current = true;
    }
    scheduleHover();
  }
  function pointerMove(event: ReactPointerEvent<HTMLElement>) {
    const previous = lastScreenPoint.current;
    lastScreenPoint.current = { x: event.screenX, y: event.screenY };
    // WebView2 can report small screen-coordinate deltas from moving its own
    // frame. The native completion supplies actual cursor movement instead.
    if (nativeMotionActive()) return;
    if (mode === 'native' && shell.mode === 'compact' && previous
      && (event.screenX !== previous.x || event.screenY !== previous.y)) {
      hoverIntent.current = true;
      if (pointerInside.current && !hover.current && !nativeMotionActive()) scheduleHover();
    }
  }
  function pointerLeave() {
    pointerInside.current = false;
    hoverIntent.current = false;
    if (hover.current) clearTimeout(hover.current);
    hover.current = null;
    if (leave.current) clearTimeout(leave.current);
    if (shell.mode === 'preview') leave.current = setTimeout(() => { if (!pointerInside.current) dispatch({ type: 'leave' }); }, 350);
  }
  function openIsland() { dispatch({ type: 'open' }); }
  function openPlayer() { dispatch({ type: 'player' }); }
  function openTimer() {
    dispatch(liteTimerOpen ? { type: 'close' } : { type: 'timer' });
  }
  async function toggleAccent() {
    if (savingAccent.current) return;
    savingAccent.current = true;
    const current = getView().snapshot.settings;
    try { await saveSettings({ ...current, mediaColorAccent: !current.mediaColorAccent }); }
    catch { /* The settings store reports the error. */ }
    finally { savingAccent.current = false; }
  }
  function accentControl(className?: string) {
    return <VisualizerAccentButton pressed={settings.mediaColorAccent} onToggle={() => void toggleAccent()} className={className}
      disabled={snapshot.readOnly || !snapshot.capabilities.includes('settings')} />;
  }
  let timerSize: { '--lite-timer-width': string; '--lite-timer-height': string } | undefined;
  let shortTimerTarget = false;
  if (mode === 'native' && monitor && (liteTimerOpen || closingLiteTimer)) {
    const workWidth = (monitor.workArea.right - monitor.workArea.left) / monitor.scale;
    const workHeight = (monitor.workArea.bottom - monitor.workArea.top) / monitor.scale;
    const gap = settings.appearance === 'notch' ? 0 : settings.topGap;
    const timerHeight = Math.min(308, Math.max(1, workHeight - Math.max(32, gap + 12)));
    shortTimerTarget = timerHeight < 270;
    timerSize = {
      '--lite-timer-width': `${Math.min(600, Math.max(1, workWidth - 32))}px`,
      '--lite-timer-height': `${timerHeight}px`,
    };
  }
  const style = { '--accent': settings.accent, '--compact-width': `${settings.width + (settings.appearance === 'notch' ? 20 : 0)}px`, '--top-gap': settings.appearance === 'notch' ? '0px' : `${settings.topGap}px`,
    '--visualizer-tint': coverTint ? `rgb(${coverTint.join(' ')})` : undefined,
    ...timerSize,
  } as CSSProperties;
  const compactMedia = !loading && compact.music && !detachedMedia;
  const playing = Boolean(snapshot.media?.playing);
  const mediaAccent = colorEnabled && playing && ((previewing && compact.music) || (displayMode === 'compact' && compactMedia)
    || (displayMode === 'open' && active === 'media'));
  const compactClock = displayMode === 'compact' && showClock;
  const expandedClock = showClock && displayMode !== 'compact';
  const clockAction = liteTimerOpen ? 'Timer schließen' : 'Timer öffnen';
  const clock = showClock ? <button key="clock" type="button" className="status-widget status-clock clock-shortcut" aria-label={`${clockAction} (${time})`} aria-expanded={liteTimerOpen} title={clockAction} onClick={openTimer}>{time}</button> : null;
  const previewCopy = <span className="peek-copy"><span className="peek-eyebrow">{snapshot.media?.playing ? 'Wiedergabe' : 'Medien'}</span><strong title={mediaTitle}>{mediaTitle}</strong>{snapshot.media?.artist ? <span className="peek-caption">{snapshot.media.artist}</span> : null}</span>;
  return <div className={`${mode}-host ${settings.reducedMotion ? 'reduce-motion' : ''} ${detachedMedia ? 'has-activity-bubbles' : ''}`} style={style}>
    {mode === 'preview' ? <aside className="preview-tools"><strong>Browser-Vorschau</strong><span>Beispieldaten · keine Windows-Aktionen</span><button onClick={() => void showSettings()}><Settings2 size={15} />Einstellungen</button></aside> : null}
    <section aria-label="Dynamic Island Lite" aria-busy={loading} inert={smartHidden} className={`island ${displayMode} ${settings.appearance}${smartHidden ? ' smart-hidden' : ''}${expandedClock ? ' with-clock' : ''}${inlinePlayer ? ' inline-player' : ''} lite-mode${liteTimerOpen ? ' lite-timer' : ''}${mediaAccent ? ' media-accent' : ''}`} onPointerEnter={pointerEnter} onPointerMove={pointerMove} onPointerLeave={pointerLeave}>
      {showClock ? <div className="island-clock-row fixed-clock-row"
        style={mode === 'native' && monitor ? { '--clock-pixel': `${1 / monitor.scale}px` } as CSSProperties : undefined}>
        {clock}
        {utilityBar ? <div className="island-actions">
          {active === 'timer' ? <IconButton icon={Music2} label="Player öffnen" onClick={openPlayer} /> : null}
          <IconButton icon={Settings2} label="Einstellungen öffnen" onClick={() => void showSettings()} />
          <IconButton icon={ChevronUp} label="Insel einklappen" onClick={() => dispatch({ type: 'close' })} />
        </div> : null}
      </div> : null}
      {liteTimerOpen || closingLiteTimer ? <div className={`lite-timer-content${closingLiteTimer ? ' timer-closing' : ''}${shortTimerTarget ? ' timer-short-target' : ''}`} inert={closingLiteTimer}><TimerPanel snapshot={snapshot} /></div> : null}
      {displayMode === 'compact' || previewing ? <div className={`island-launcher${compactClock ? ' compact-clock-layout' : ''}`}>
        {displayMode === 'compact' ? <button ref={compactButton} className={`compact-content${compactMedia ? ' compact-playing' : ''}`} disabled={loading} onClick={openIsland} aria-label="Dynamic Island öffnen" title={compactClock ? compactLabel : undefined}>
          {compactMedia ? <MediaArtwork source={cover} transition={coverTransition} /> : compact.music || timerShortcut || compactClock ? null : <Music2 className="compact-symbol" size={14} aria-hidden="true" />}
          {compactMedia && transition.closingPreview ? <span className="compact-preview-copy" aria-hidden="true">{previewCopy}</span> : null}
          <span className="compact-label">{loading ? 'Wird geladen' : compactLabel}</span>
          {compactMedia || compactClock ? null : <ChevronDown className="compact-chevron" size={12} aria-hidden="true" />}
        </button> : compact.music ? <div className="media-peek">
          <button className="peek-content media-peek-link" aria-label={`Player öffnen – ${mediaTitle}`} onClick={openPlayer}>
          <MediaArtwork source={cover} transition={coverTransition} />
          {previewCopy}
          </button>
          {launcherVisualizer ? accentControl('peek-media-signal')
            : <span className="peek-media-signal" aria-hidden="true">{launcherVisualizer ? <VisualizerBars /> : <ArrowRight size={16} />}</span>}
        </div> : <button className="peek-content" aria-label={'Player öffnen'} onClick={openPlayer}>
          <span className="peek-module-icon"><Music2 size={18} aria-hidden="true" /></span>
          <span className="peek-copy"><span className="peek-caption">Player öffnen</span><strong>Medien</strong></span>
          <ArrowRight className="peek-arrow" size={16} aria-hidden="true" />
        </button>}
        {!loading && (previewing ? !compact.music && timerShortcut : compact.music || timerShortcut) ? <div className="activity-links">
          {compact.music && !detachedMedia ? launcherVisualizer ? accentControl('media-visualizer-link')
            : <button className={launcherVisualizer ? 'media-visualizer-link' : undefined} aria-label="Medien direkt öffnen" title="Medien direkt öffnen" onClick={openPlayer}>
            {launcherVisualizer ? <VisualizerBars /> : <Music2 size={16} aria-hidden="true" />}
          </button> : null}
          {timerShortcut ? <button className="activity-timer" aria-label="Timer direkt öffnen" title="Timer direkt öffnen" onClick={openTimer}>
            {previewing ? <><Timer size={16} aria-hidden="true" /><span className="peek-activity-label">{timerDetail}</span></>
              : compact.timerMinutes !== null ? <span aria-label="Timer läuft">{compact.timerMinutes}m</span> : <Timer size={16} />}
          </button> : null}
        </div> : null}
      </div>
        : displayMode === 'open' && inlinePlayer ? <div className="panel-body">
          <MediaPanel snapshot={snapshot} receivedAtMs={mediaReceivedAtMs} visualizerControl={accentControl()} />
        </div> : null}
      {error && displayMode === 'open' ? <div className="error-toast" role="alert"><span title={error}>{error}</span><IconButton icon={X} label="Fehlermeldung schließen" onClick={clearError} /></div> : null}
    </section>
    {detachedMedia ? launcherVisualizer ? accentControl(`icon-button activity-bubble media-bubble${playing && colorEnabled ? ' media-accent' : ''}`)
      : <button className={`icon-button activity-bubble media-bubble${playing && colorEnabled ? ' media-accent' : ''}`} aria-label="Medien direkt öffnen"
      title={snapshot.media?.title || 'Medien direkt öffnen'} onClick={() => dispatch({ type: 'player' })}>
      {launcherVisualizer ? <VisualizerBars /> : <Music2 size={16} aria-hidden="true" />}
    </button> : null}
  </div>;
}
