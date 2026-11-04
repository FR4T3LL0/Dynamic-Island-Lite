import { useEffect, useSyncExternalStore } from 'react';
import { Island } from './Island';
import { SettingsView } from '../settings/SettingsView';

const subscribeRoute = (listener: () => void) => { window.addEventListener('hashchange', listener); return () => window.removeEventListener('hashchange', listener); };
const getRoute = () => window.location.hash;

export function App() {
  const route = useSyncExternalStore(subscribeRoute, getRoute);
  const settings = route === '#settings';
  useEffect(() => { document.documentElement.dataset.surface = settings ? 'settings' : 'island'; }, [settings]);
  return settings ? <SettingsView /> : <Island />;
}
