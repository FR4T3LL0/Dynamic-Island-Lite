import { useState } from 'react';
import { Pause, Play, RotateCcw, Timer } from 'lucide-react';
import { EmptyState } from '../../components/Primitives';
import type { Snapshot, TimerInfo } from '../../bridge/contracts';
import { perform, reportError } from '../../bridge/store';
import { MonthCalendar } from './MonthCalendar';
import { SegmentedControl } from '../../components/SegmentedControl';

const timerModes = [{ value: 'countdown', label: 'Countdown' }, { value: 'pomodoro', label: 'Pomodoro' }] as const;

function TimerControls({ timer, enabled }: { timer: TimerInfo; enabled: boolean }) {
  const [seconds, setSeconds] = useState(String(timer.durationMs / 1000));
  const running = timer.status === 'running';
  const editableCountdown = timer.mode === 'countdown' && !running && timer.status !== 'paused';
  const draft = Number(seconds);
  const remaining = editableCountdown && timer.status === 'idle' && Number.isInteger(draft) && draft >= 1 && draft <= 86400
    ? draft : Math.ceil(timer.remainingMs / 1000);
  const hours = Math.floor(remaining / 3600);
  const minutes = Math.floor(remaining / 60) % 60;
  const display = `${hours ? `${hours}:` : ''}${String(minutes).padStart(2, '0')}:${String(remaining % 60).padStart(2, '0')}`;
  const progress = timer.durationMs ? (1 - timer.remainingMs / timer.durationMs) * 100 : 0;
  async function start() {
    if (timer.status === 'paused' || (timer.mode === 'pomodoro' && timer.status === 'completed')) { await perform('timer_start'); return; }
    const duration = Number(seconds);
    if (!Number.isInteger(duration) || duration < 1 || duration > 86400) { reportError('Der Countdown muss zwischen einer Sekunde und 24 Stunden liegen.'); return; }
    const configured = await perform('timer_configure', { mode: timer.mode, seconds: duration });
    if (configured) await perform('timer_start');
  }
  return <div className="timer-view"><SegmentedControl label="Timerart" options={timerModes} value={timer.mode}
    disabled={running || !enabled} onChange={(mode) => void perform('timer_configure', { mode, seconds: 1500 })} />
    {editableCountdown ? <label className="timer-duration">Dauer in Sekunden<input aria-label="Countdown in Sekunden" type="number" min={1} max={86400} step={1} value={seconds} disabled={!enabled} onChange={(event) => setSeconds(event.target.value)} /></label>
      : <p className="timer-phase">{timer.mode === 'pomodoro' ? timer.phase === 'work' ? 'Fokuszeit' : timer.phase === 'short' ? 'Kurze Pause' : 'Lange Pause' : running ? 'Countdown läuft' : 'Countdown pausiert'}</p>}
    <div className="timer-ring"><span>{display}</span></div>
    <progress className="timer-progress" data-active={running || timer.status === 'paused' ? '' : undefined} max={100} value={progress} aria-label="Timerfortschritt" />
    <div className="timer-actions"><button className="primary" disabled={!enabled} onClick={() => running ? void perform('timer_pause') : void start()}>{running ? <Pause size={16} /> : <Play size={16} />}{running ? 'Pausieren' : timer.status === 'paused' ? 'Fortsetzen' : timer.status === 'completed' && timer.mode === 'pomodoro' ? 'Nächste Phase' : 'Starten'}</button><button disabled={!enabled || timer.status === 'idle'} onClick={() => void perform('timer_reset')}><RotateCcw size={15} />Zurücksetzen</button></div>
    {editableCountdown ? <div className="timer-presets" role="group" aria-label="Countdown-Schnellvorgaben">{[5, 15, 25, 45].map((minutes) => <button key={minutes} disabled={!enabled} aria-pressed={Number(seconds) === minutes * 60} onClick={() => setSeconds(String(minutes * 60))}>{minutes} min</button>)}</div> : null}
    <span className="availability" aria-live="polite">{timer.status === 'completed' ? 'Timer abgelaufen' : timer.mode === 'pomodoro' ? `${timer.completedWork} Arbeitsphasen abgeschlossen` : '\u00a0'}</span>
  </div>;
}
export function TimerPanel({ snapshot }: { snapshot: Snapshot }) {
  const [calendar, setCalendar] = useState(false);
  return <><div className="module-scroll timer-module">
    <div className="segments timer-layout-toggle" role="group" aria-label="Timeransicht">
      <button aria-pressed={!calendar} onClick={() => setCalendar(false)}>Countdown &amp; Pomodoro</button>
      <button aria-pressed={calendar} onClick={() => setCalendar(true)}>Kalender</button>
    </div>
    <div className={`timer-calendar-layout ${calendar ? 'show-calendar' : ''}`}>
      <MonthCalendar />
      <div className="timer-controls-column">{snapshot.timer ? <TimerControls timer={snapshot.timer} enabled={snapshot.capabilities.includes('timer')} /> : <EmptyState icon={Timer}>Timer zurzeit nicht verfügbar.</EmptyState>}</div>
  </div></div></>;
}
