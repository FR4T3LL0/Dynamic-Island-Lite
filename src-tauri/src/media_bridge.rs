use crate::state::AppState;
use island_core::{
    error::{AppError, AppResult},
    media::MediaAction,
};
use island_win::media::{MediaSnapshot, SessionScope};
use serde::Serialize;
use std::sync::{atomic::Ordering, Arc};
use tauri::{AppHandle, Emitter, Manager};
pub fn scope() -> SessionScope {
    #[cfg(all(debug_assertions, feature = "native-e2e"))]
    if crate::diagnostic_root().is_some() {
        let ids = std::env::var("DYNAMIC_ISLAND_TEST_MEDIA_SOURCES")
            .ok()
            .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok());
        return match ids {
            Some(ids)
                if !ids.is_empty()
                    && ids.len() <= 8
                    && ids.iter().all(|id| valid_test_source(id)) =>
            {
                SessionScope::Only(ids)
            }
            _ => SessionScope::None,
        };
    }
    SessionScope::All
}
#[cfg(any(test, all(debug_assertions, feature = "native-e2e")))]
fn valid_test_source(id: &str) -> bool {
    id.len() <= 128
        && id.starts_with("app.dynamicisland.lite.media-e2e-")
        && (id.ends_with(".one") || id.ends_with(".two"))
        && id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'.' || c == b'-')
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    id: String,
    app: String,
    title: String,
    artist: String,
    playing: bool,
    position: f64,
    duration: f64,
    rate: f64,
    seek_start: f64,
    seek_end: f64,
    can_seek: bool,
    can_toggle: bool,
    can_previous: bool,
    can_next: bool,
    cover: Option<String>,
    cover_tint: Option<[u8; 3]>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaStatus {
    enabled: bool,
    generation: u64,
    revision: u64,
    selected_id: Option<String>,
    sessions: Vec<MediaInfo>,
    error: Option<AppError>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaView {
    pub media: Option<MediaInfo>,
    pub media_state: MediaStatus,
}
pub fn view(snapshot: &MediaSnapshot) -> MediaView {
    project(snapshot, unsafe {
        windows::Win32::System::SystemInformation::GetTickCount64()
    })
}
fn project(snapshot: &MediaSnapshot, now_ms: u64) -> MediaView {
    let sessions: Vec<_> = snapshot
        .state
        .sessions
        .iter()
        .map(|session| {
            let time = &session.timeline;
            let caps = &session.capabilities;
            let cover_key = session
                .cover_key
                .as_ref()
                .filter(|key| snapshot.covers.contains_key(*key));
            MediaInfo {
                id: session.id.clone(),
                app: session
                    .app_label
                    .clone()
                    .unwrap_or_else(|| source_label(&session.app)),
                title: session.title.clone(),
                artist: session.artist.clone(),
                playing: session.playing,
                position: time.position_at(now_ms, session.playing) as f64 / 1000.,
                duration: time.duration_ms as f64 / 1000.,
                rate: time.rate,
                seek_start: time.seek_start_ms as f64 / 1000.,
                seek_end: time.seek_end_ms as f64 / 1000.,
                can_seek: caps.seek && time.seek_start_ms < time.seek_end_ms,
                can_toggle: caps.toggle
                    || (session.playing && caps.pause)
                    || (!session.playing && caps.play),
                can_previous: caps.previous,
                can_next: caps.next,
                cover: cover_key.map(|key| format!("http://island-image.localhost/{key}")),
                cover_tint: cover_key.and(session.cover_tint),
            }
        })
        .collect();
    let media = sessions
        .iter()
        .find(|session| Some(&session.id) == snapshot.state.selected_id.as_ref())
        .cloned();
    MediaView {
        media,
        media_state: MediaStatus {
            enabled: snapshot.state.enabled,
            generation: snapshot.state.generation,
            revision: snapshot.state.revision,
            selected_id: snapshot.state.selected_id.clone(),
            sessions,
            error: snapshot.error.clone(),
        },
    }
}
fn source_label(id: &str) -> String {
    if id.contains(['\\', '/']) || id.to_ascii_lowercase().ends_with(".exe") {
        return std::path::Path::new(id)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
    }
    if id.starts_with("app.dynamicisland.lite.media-e2e-") {
        return "Lokaler Testplayer".into();
    }
    let (package, application) = id.split_once('!').unwrap_or((id, id));
    let name = if application.eq_ignore_ascii_case("app") {
        package.split('_').next().unwrap_or(package)
    } else {
        application
    };
    name.rsplit('.')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("Medienplayer")
        .to_owned()
}
pub fn attach(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.state::<AppState>().media.attach(
        Arc::new(move |snapshot| {
            let revision = handle
                .state::<AppState>()
                .revision
                .fetch_add(1, Ordering::AcqRel)
                + 1;
            let _ = handle.emit(
                "media-updated",
                serde_json::json!({"view":view(&snapshot),"revision":revision}),
            );
        }),
        true,
    );
}
async fn action(app: AppHandle, generation: u64, id: String, action: MediaAction) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.media.command(generation, id, action)
    })
    .await
    .map_err(|e| AppError::new("io_error", e.to_string()))?
}
#[tauri::command]
pub async fn media_toggle(app: AppHandle, generation: u64, id: String) -> AppResult<()> {
    action(app, generation, id, MediaAction::Toggle).await
}
#[tauri::command]
pub async fn media_previous(app: AppHandle, generation: u64, id: String) -> AppResult<()> {
    action(app, generation, id, MediaAction::Previous).await
}
#[tauri::command]
pub async fn media_next(app: AppHandle, generation: u64, id: String) -> AppResult<()> {
    action(app, generation, id, MediaAction::Next).await
}
#[tauri::command]
pub async fn media_seek(
    app: AppHandle,
    generation: u64,
    id: String,
    position: f64,
) -> AppResult<()> {
    if !position.is_finite() || position < 0. || position > u64::MAX as f64 / 1000. {
        return Err(AppError::new(
            "invalid_argument",
            "Die Wiedergabeposition ist ungültig.",
        ));
    }
    action(
        app,
        generation,
        id,
        MediaAction::Seek((position * 1000.).round() as u64),
    )
    .await
}
#[tauri::command]
pub async fn refresh_media(app: AppHandle) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.media.refresh()
    })
    .await
    .map_err(|e| AppError::new("io_error", e.to_string()))?
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_uses_boot_clock_and_omits_uncached_covers() {
        use island_core::media::{MediaCapabilities, MediaSession, MediaTimeline};
        let mut snapshot = MediaSnapshot::default();
        snapshot.state.set_enabled(true).unwrap();
        snapshot
            .state
            .update(
                1,
                vec![MediaSession {
                    id: "one".into(),
                    app: r"C:\Player\player.exe".into(),
                    app_label: None,
                    title: String::new(),
                    artist: "Künstler ü".into(),
                    playing: true,
                    cover_key: Some("missing-cover".into()),
                    cover_tint: Some([80, 120, 180]),
                    timeline: MediaTimeline {
                        position_ms: 1000,
                        duration_ms: 90000,
                        seek_start_ms: 500,
                        seek_end_ms: 80000,
                        sampled_at_ms: 1000000,
                        rate: 2.,
                    },
                    capabilities: MediaCapabilities {
                        pause: true,
                        seek: true,
                        ..Default::default()
                    },
                }],
                Some("one"),
            )
            .unwrap();
        let info = project(&snapshot, 1000500).media.unwrap();
        assert_eq!(info.position, 2.);
        assert_eq!(info.app, "player");
        assert!(info.can_toggle && info.can_seek);
        assert!(!info.can_next);
        assert!(info.cover.is_none());
        snapshot.state.sessions[0].playing = false;
        assert_eq!(project(&snapshot, 1000500).media.unwrap().position, 1.);
    }
    #[test]
    fn test_source_filter_rejects_personal_sources_and_malformed_ids() {
        assert!(valid_test_source(
            "app.dynamicisland.lite.media-e2e-1234.one"
        ));
        for id in [
            "Spotify",
            "chrome.exe",
            "app.dynamicisland.lite.media-e2e-1234.other",
            "app.dynamicisland.lite.media-e2e-x/../one",
        ] {
            assert!(!valid_test_source(id));
        }
    }
}
