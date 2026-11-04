import { useEffect, useState } from 'react';

const clock = new Intl.DateTimeFormat('de-DE', { hour: '2-digit', minute: '2-digit' });

export function useClockTime(visible = true) {
  const [time, setTime] = useState(() => clock.format(new Date()));
  useEffect(() => {
    if (!visible) return;
    const tick = () => setTime(clock.format(new Date()));
    tick();
    const id = setInterval(tick, 1000);
    document.addEventListener('visibilitychange', tick);
    return () => { clearInterval(id); document.removeEventListener('visibilitychange', tick); };
  }, [visible]);
  return time;
}
