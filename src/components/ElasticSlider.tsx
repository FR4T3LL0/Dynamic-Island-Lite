import type { CSSProperties, PointerEvent } from 'react';

export function ElasticSlider({ id, value, disabled, onChange, label }: {
  id: string;
  value: number;
  disabled: boolean;
  onChange: (value: number) => void;
  label?: string;
}) {
  function reset({ currentTarget }: { currentTarget: HTMLInputElement }) {
    currentTarget.parentElement?.style.removeProperty('--elastic-stretch');
    currentTarget.parentElement?.style.removeProperty('--elastic-origin');
  }

  function stretch(event: PointerEvent<HTMLInputElement>) {
    if (disabled || !(event.buttons & 1)) return;
    const { left, right, width } = event.currentTarget.getBoundingClientRect();
    if (event.clientX >= left && event.clientX <= right) {
      reset(event);
      return;
    }
    const before = event.clientX < left;
    const distance = before ? left - event.clientX : event.clientX - right;
    const rail = event.currentTarget.parentElement?.style;
    rail?.setProperty('--elastic-origin', before ? 'right' : 'left');
    rail?.setProperty('--elastic-stretch', String(1 + Math.min(8, distance * 0.18) / Math.max(1, width)));
  }

  return <div className="elastic-slider" style={{ '--elastic-value': `${value}%` } as CSSProperties}>
    <span className="elastic-rail" aria-hidden="true" />
    <input id={id} type="range" min={0} max={100} value={value} aria-label={label} aria-valuetext={`${value} Prozent`}
      disabled={disabled} onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
      onPointerMove={stretch} onPointerUp={reset} onPointerCancel={reset} onLostPointerCapture={reset} onBlur={reset} />
  </div>;
}
