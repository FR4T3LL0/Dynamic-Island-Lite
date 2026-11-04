import { useEffect, useRef, useState } from 'react';
import { Volume2 } from 'lucide-react';
import type { AudioInfo } from '../../bridge/contracts';
import { perform } from '../../bridge/store';
import { ElasticSlider } from '../../components/ElasticSlider';

export function VolumeControl({ audio }: { audio: AudioInfo }) {
  const [draft, setDraft] = useState<number | null>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const sequence = useRef(0);
  useEffect(() => () => { clearTimeout(timer.current); sequence.current++; }, []);
  const change = (value: number) => {
    setDraft(value); clearTimeout(timer.current);
    const request = ++sequence.current;
    timer.current = setTimeout(async () => {
      await perform('set_volume', { id: audio.defaultDevice, volume: value });
      if (sequence.current === request) setDraft(null);
    }, 80);
  };
  const value = draft ?? Math.round(audio.volume);
  const id = 'player-volume';
  return <div className="volume-row player-volume">
    <label htmlFor={id} title="Systemlautstärke"><Volume2 size={15} aria-hidden="true" /></label>
    <ElasticSlider id={id} value={value} disabled={!audio.defaultDevice} onChange={change} label="Systemlautstärke" />
    <output htmlFor={id}>{value}%</output>
  </div>;
}
