import { useEffect, useState } from 'react';
import { Music2 } from 'lucide-react';
import type { CoverTransition } from '../../bridge/contracts';

type Frame = {
  source: string | null;
  previous: string | null | undefined;
  ready: boolean;
  failed: boolean;
};

export function MediaArtwork({ source, transition }: { source: string | null; transition: CoverTransition }) {
  const [frame, setFrame] = useState<Frame>({ source, previous: undefined, ready: true, failed: false });
  if (source !== frame.source || (transition === 'off' && frame.previous !== undefined)) {
    const visible = frame.ready ? (frame.failed ? null : frame.source) : frame.previous;
    const previous = transition === 'off' || visible === source ? undefined : visible;
    setFrame({ source, previous, ready: previous === undefined, failed: false });
  }

  const { previous, ready, failed } = frame;
  useEffect(() => {
    if (source !== null || ready) return;
    // Windows can briefly clear the thumbnail before publishing the next cover.
    const timeout = setTimeout(() => {
      setFrame(current => current.source === null ? { ...current, ready: true } : current);
    }, 150);
    return () => clearTimeout(timeout);
  }, [source, ready]);

  const moving = previous !== undefined && ready;
  const key = (url: string | null) => url ?? 'placeholder';
  const confirm = (failed: boolean) => setFrame(current => current.source === source && (!current.ready || current.failed !== failed)
    ? { ...current, ready: true, failed }
    : current);

  return <span className="album-placeholder" aria-hidden="true" data-cover-transition={moving ? transition : undefined}>
    {previous !== undefined ? <span key={key(previous)} className="cover-layer cover-previous">
      {previous ? <img className="media-cover" src={previous} alt="" /> : <Music2 size={32} strokeWidth={1.5} />}
    </span> : null}
    <span key={key(source)} className={`cover-layer cover-current${ready ? '' : ' cover-pending'}`}
      onAnimationEnd={(event) => {
        if (event.target !== event.currentTarget) return;
        setFrame(current => current.source === source ? { ...current, previous: undefined } : current);
      }}>
      {source && !failed ? <img className="media-cover" src={source} alt="" onLoad={() => confirm(false)} onError={() => confirm(true)} />
        : <Music2 size={32} strokeWidth={1.5} />}
    </span>
  </span>;
}
