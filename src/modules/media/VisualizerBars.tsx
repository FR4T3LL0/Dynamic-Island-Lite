import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { mode } from '../../bridge/store';

const silence = [0, 0, 0, 0, 0];
let workerQueue = Promise.resolve();

function requestCapture(active: boolean) {
  workerQueue = workerQueue.then(() => invoke<void>('set_visualizer', { active })).catch(() => {});
}

export function useVisualizer(active: boolean, output: string | undefined) {
  useEffect(() => {
    if (!active || mode !== 'native') return;
    requestCapture(true);
    return () => requestCapture(false);
  }, [active, output]);
}

export function VisualizerBars() {
  const [bands, setBands] = useState(silence);
  useEffect(() => {
    if (mode !== 'native') return;
    let subscribed = true;
    const remove = import('@tauri-apps/api/event').then(({ listen }) => listen<number[]>('visualizer-bands', ({ payload }) => {
      if (subscribed && payload.length === 5) setBands(payload);
    })).catch(() => undefined);
    return () => { subscribed = false; void remove.then((unlisten) => unlisten?.()); };
  }, []);
  return <span className="visualizer-bars" aria-hidden="true">
    {bands.map((level, index) => {
      const height = Math.min(1, 0.16 + 1.15 * Math.pow(Math.max(0, level), 0.45));
      return <i key={index} style={{ transform: `scaleY(${height})` }} />;
    })}
  </span>;
}

export function VisualizerAccentButton({ pressed, onToggle, className = '', disabled }: {
  pressed: boolean; onToggle: () => void; className?: string; disabled: boolean;
}) {
  return <button type="button" className={`visualizer-toggle ${className}`} aria-label="Medienfarbakzent" aria-pressed={pressed}
    title={pressed ? 'Medienfarbakzent ausschalten' : 'Medienfarbakzent einschalten'} disabled={disabled} onClick={onToggle}><VisualizerBars /></button>;
}
