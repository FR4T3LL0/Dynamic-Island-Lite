# Dynamic Island Lite

Eigenständige Windows-x64-App auf Basis von Dynamic Island. Die Uhr öffnet und schließt den Kalender mit Countdown und Pomodoro. Das Cover oder die Medienvorschau öffnet den Player mit Titelsteuerung, Lautstärke, Visualizer und optionalem Cover-Farbakzent. Smart, Peek, feste oder federnde Uhr und die vorhandenen Statusoptionen gehören zu Lite.

Lite verwendet das Profil `app.dynamicisland.lite`, eine eigene Installation und ein eigenes Standardkürzel `Strg+Alt+Umschalt+I`. Einstellungen werden beim ersten Start mit Lite-Standardwerten angelegt.

## Installieren und aktualisieren

Für die Installation muss die [Microsoft Edge WebView2 Evergreen Runtime](https://learn.microsoft.com/de-de/microsoft-edge/webview2/concepts/distribution) bereits verfügbar sein. Setup und Deinstallation werden im eigenen Windows-Konto mit normalen Benutzerrechten ausgeführt.

Beim ersten Start der installierten App mit einem neuen Lite-Profil ist der Windows-Autostart standardmäßig aktiv. Du kannst ihn in den Einstellungen unter **Mit Windows starten** deaktivieren.

Dieses Setup registriert die Lite-App-Kennung für spätere Updates. Solche Updates ersetzen die Dateien im selben Installationsordner und behalten App-Daten und Autostart bei. Wähle einen zuvor selbst gewählten Ordner beim Update erneut aus. Deinstalliere ältere Lite-Versionen aus der Zeit vor dieser Registrierung und Installationen mit einer anderen App-Kennung zuerst über Windows. Für einen Ordnerwechsel ist ebenfalls eine vorherige Deinstallation nötig. Die Deinstallation bietet eine eigene Auswahl zum Löschen der App-Daten.

## Lokal bauen

Unter Windows x64 mit Node.js 22.12 oder neuer, Rust 1.96 und den MSVC-Buildtools im Projektordner ausführen.

```powershell
npm.cmd ci
npm.cmd run app:build
```

Der Installer liegt anschließend unter `src-tauri/target/release/bundle/nsis`.

```powershell
npm.cmd run lint
npm.cmd run test:unit
cargo test --manifest-path src-tauri/Cargo.toml --workspace --locked --features custom-protocol
```

## Lizenz

Der Projektcode steht unter der [MIT-Lizenz](LICENSE). Die [Drittanbieterhinweise](src-tauri/windows/third-party/THIRD-PARTY-NOTICES.md) dokumentieren die gesonderten Lizenzen.
