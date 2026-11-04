import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from './app/App';
import { useClockTime } from './app/useClockTime';
import { initialize, mode } from './bridge/store';
import './styles/app.css';
import './styles/layout.css';
import './styles/apple-dark.css';

document.documentElement.dataset.host = mode;
const settings = window.location.hash === '#settings';
const clockOverlay = window.location.hash === '#clock-overlay';
document.documentElement.dataset.surface = settings ? 'settings' : clockOverlay ? 'clock-overlay' : 'island';
createRoot(document.getElementById('root')!).render(<StrictMode>{clockOverlay ? <ClockOverlay /> : <App />}</StrictMode>);
if (!clockOverlay) void initialize();

function ClockOverlay() {
  const time = useClockTime();
  return <time className="clock-overlay-text" aria-hidden="true">{time}</time>;
}
