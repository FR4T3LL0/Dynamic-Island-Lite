import type { CSSProperties, KeyboardEvent } from 'react';

export function SegmentedControl<T extends string>({ label, options, value, onChange, disabled = false }: {
  label: string;
  options: readonly { value: T; label: string }[];
  value: T;
  onChange: (value: T) => void;
  disabled?: boolean;
}) {
  const selected = options.findIndex((option) => option.value === value);
  function navigate(event: KeyboardEvent<HTMLButtonElement>, index: number) {
    const step = event.key === 'ArrowLeft' ? -1 : event.key === 'ArrowRight' ? 1 : 0;
    if (disabled || !step || event.altKey || event.ctrlKey || event.metaKey) return;
    event.preventDefault();
    const next = (index + step + options.length) % options.length;
    event.currentTarget.parentElement?.querySelectorAll('button')[next]?.focus();
    if (next !== selected) onChange(options[next].value);
  }

  return <div className="segments sliding-segments" role="group" aria-label={label}
    style={{ '--segment-count': options.length, '--segment-index': selected } as CSSProperties}>
    {options.map((option, index) => <button key={option.value} type="button" aria-pressed={option.value === value}
      disabled={disabled} tabIndex={index === selected ? 0 : -1}
      onClick={() => onChange(option.value)}
      onKeyDown={(event) => navigate(event, index)}>{option.label}</button>)}
  </div>;
}
