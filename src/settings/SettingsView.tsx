import { useState, useSyncExternalStore } from 'react';
import { Check } from 'lucide-react';
import { getView, mode, perform, saveSettings, subscribe } from '../bridge/store';
import type { CoverTransition, Settings } from '../bridge/contracts';

export function SettingsView() {
  const { snapshot, error, loading } = useSyncExternalStore(subscribe, getView);
  const settings = snapshot.settings;
  const display = snapshot.display;
  const savedMonitor = settings.monitor?.toLowerCase();
  const chosen = savedMonitor ? display.monitors.find((monitor) => [monitor.id, ...monitor.aliases, monitor.device].some((id) => id.toLowerCase() === savedMonitor)) : null;
  const [saved, setSaved] = useState(false);
  const [pending,setPending]=useState(false);
  const disabled = pending || loading || snapshot.readOnly || !snapshot.capabilities.includes('settings');
  async function update(patch: Partial<Settings>) {
    setSaved(false);
    setPending(true);
    try { await saveSettings({ ...settings, ...patch }); setSaved(true); } catch { /* Shared error is rendered below. */ }
    finally {setPending(false);}
  }
  return <main className={`settings-page${settings.reducedMotion ? ' reduce-motion' : ''}`} aria-busy={loading}><header><h1>Einstellungen</h1><span>{saved ? <><Check size={14} />Gespeichert</> : mode === 'preview' ? 'Browser-Vorschau · Beispieldaten' : 'Dynamic Island Lite'}</span></header>
    {error ? <p role="alert" className="error-message">{error}</p> : null}
    {snapshot.warning ? <p className="warning-message">{snapshot.warning}</p> : null}
    <fieldset disabled={disabled}><legend>Darstellung</legend>
      <div className="setting-row"><span>Form</span><div className="segments"><button aria-pressed={settings.appearance === 'capsule'} onClick={() => void update({ appearance: 'capsule' })}>Kapsel</button><button aria-pressed={settings.appearance === 'notch'} onClick={() => void update({ appearance: 'notch' })}>Notch</button></div></div>
      <label className="setting-row"><span>Breite der Kapsel</span><input type="range" min={140} max={320} value={settings.width} onChange={(event) => void update({ width: Number(event.target.value) })} /><output>{settings.width} px</output></label>
      <label className="setting-row"><span>Abstand oben</span><input type="range" min={0} max={48} disabled={settings.appearance === 'notch'} value={settings.topGap} onChange={(event) => void update({ topGap: Number(event.target.value) })} /><output>{settings.topGap} px</output></label>
      <label className="setting-row"><span>Akzentfarbe</span><input type="color" value={settings.accent} onChange={(event) => void update({ accent: event.target.value })} /></label>
      <label className="setting-row"><span>Bewegung reduzieren</span><input type="checkbox" role="switch" checked={settings.reducedMotion} onChange={(event) => void update({ reducedMotion: event.target.checked })} /></label>
      <label className="setting-row"><span>Sichtbarkeit</span><select value={settings.peek ? 'peek' : settings.smart ? 'smart' : settings.fixedAppbar ? 'fixed' : 'always'} onChange={(event) => void update({ peek: event.target.value === 'peek', smart: event.target.value === 'smart', fixedAppbar: event.target.value === 'fixed' })}>
        <option value="always">Immer sichtbar</option>
        <option value="fixed">Fixiert mit reserviertem Bildschirmrand</option>
        <option value="smart">Smart</option>
        <option value="peek">Peek</option>
      </select></label>
      {settings.smart || settings.peek ? <label className="setting-row"><span>Randverzögerung</span><input type="range" min={0} max={1500} step={50} value={settings.edgeDelayMs} onChange={(event) => void update({ edgeDelayMs: Number(event.target.value) })} /><output>{settings.edgeDelayMs} ms</output></label> : null}
      {settings.smart ? <p className="setting-hint">Blendet die kompakte Insel bei Überdeckung aus. Verweilen oben in der Bildschirmmitte blendet sie kurz wieder ein.</p> : settings.peek ? <p className="setting-hint">Blendet die Insel im Leerlauf aus. Sie erscheint kurz beim Verweilen oben in der Bildschirmmitte, bei Medienstart, Titelwechsel oder Timerablauf.</p> : null}
    </fieldset>
    <fieldset disabled={disabled}><legend>Uhr und Musik</legend>
      <label className="setting-row"><span>Uhrbewegung</span><select value={settings.clockFollowsIsland ? 'follow' : 'fixed'} onChange={(event) => void update({ clockFollowsIsland: event.target.value === 'follow' })}>
        <option value="fixed">Fest verankert</option>
        <option value="follow">Mit der Insel mitfedern</option>
      </select></label>
      <p className="setting-hint">„Bewegung reduzieren“ hat Vorrang.</p>
      <label className="setting-row"><span>Musikvisualizer</span><input type="checkbox" role="switch" checked={settings.visualizer} onChange={(event) => void update({ visualizer: event.target.checked })} /></label>
      <label className="setting-row"><span>Cover-Übergang</span><select value={settings.coverTransition} onChange={(event) => void update({ coverTransition: event.target.value as CoverTransition })}>
        <option value="fade">Überblenden</option>
        <option value="slide">Seitlich gleiten</option>
        <option value="zoom">Sanft einzoomen</option>
        <option value="off">Aus</option>
      </select></label>
    </fieldset>
    <fieldset disabled={disabled}><legend>Desktop</legend>
      <label className="setting-row"><span>Monitor</span><select aria-label="Monitor" value={chosen?.id ?? settings.monitor ?? ''} disabled={!display.monitors.length} onChange={(event) => void update({ monitor: event.target.value || null })}><option value="">Hauptmonitor (automatisch)</option>{settings.monitor && !chosen ? <option value={settings.monitor}>Gespeicherter Monitor (nicht verbunden)</option> : null}{display.monitors.map((monitor,index) => <option key={monitor.id} value={monitor.id}>{index+1}: {monitor.name} · {Math.round(monitor.scale*100)}%{monitor.primary ? ' · Hauptmonitor' : ''}</option>)}</select></label>
      {display.fallback ? <p className="setting-hint">Der gewählte Monitor ist nicht verbunden. Die Insel verwendet vorübergehend den Hauptmonitor und kehrt bei seiner Rückkehr zurück.</p> : null}
      {display.error ? <p role="alert" className="error-message">{display.error.message}</p> : null}
      <button onClick={() => void perform('refresh_displays')}>Bildschirme aktualisieren</button>
      <ShortcutEditor key={settings.shortcut} value={settings.shortcut} save={async (shortcut) => { setSaved(false); await saveSettings({ ...settings, shortcut }); setSaved(true); }} />
      <p id="shortcut-hint" className="setting-hint">Strg oder Alt, optional Umschalt, dazu Buchstabe, Ziffer oder F1–F24 (außer F12). Öffnet und schließt die Insel.</p>
      {mode === 'native' ? <p className="setting-hint">{snapshot.shortcut.registered ? `Aktiv: ${snapshot.shortcut.registered}` : 'Kein globales Tastenkürzel aktiv. Die Insel bleibt über das Tray erreichbar.'}</p> : <p className="setting-hint">Globale Tastenkürzel sind nur in der Windows-App verfügbar.</p>}
      {snapshot.shortcut.error ? <p role="alert" className="error-message">{snapshot.shortcut.error.message}</p> : null}
      {mode === 'native' && !snapshot.shortcut.registered ? <button onClick={() => void perform('retry_shortcut')}>Tastenkürzel erneut registrieren</button> : null}
      <label className="setting-row"><span>Mit Windows starten</span><input type="checkbox" role="switch" checked={settings.autostart} disabled={!snapshot.capabilities.includes('autostart')} onChange={(event) => void update({ autostart: event.target.checked })} /></label>
      {snapshot.autostart?.error ? <p role="alert" className="error-message">{snapshot.autostart.error.message}</p> : null}
      {snapshot.autostart && settings.autostart !== snapshot.autostart.registered ? <p className="setting-hint">Die gespeicherte Autostartwahl stimmt nicht mit dem Windows-Eintrag überein. Bitte die Einstellung erneut setzen. Windows kann den Start zusätzlich in den Autostart-Apps deaktivieren.</p> : null}
      <label className="setting-row"><span>Ton bei Timerablauf</span><input type="checkbox" role="switch" checked={settings.timerSound} onChange={(event) => void update({ timerSound: event.target.checked })} /></label>
    </fieldset>
  </main>;
}

function ShortcutEditor({ value, save }: { value: string; save: (value: string) => Promise<void> }) {
  const [draft, setDraft] = useState(value);
  const [pending, setPending] = useState(false);
  return <form onSubmit={(event) => {
    event.preventDefault();
    setPending(true);
    void save(draft).catch(() => { /* The shared error remains visible. */ }).finally(() => setPending(false));
  }}>
    <label className="setting-row"><span>Tastenkürzel</span><input aria-label="Tastenkürzel" type="text" maxLength={80} value={draft} disabled={pending} onChange={(event) => setDraft(event.target.value)} aria-describedby="shortcut-hint" /></label>
    <button type="submit" disabled={pending || draft.trim() === value}>Tastenkürzel speichern</button>
  </form>;
}
