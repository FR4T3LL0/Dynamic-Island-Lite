use super::*;
use island_core::media::{MediaCapabilities, MediaTimeline};
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
use windows::{
    core::Interface,
    Foundation::TypedEventHandler,
    Media::Control::*,
    Storage::Streams::IRandomAccessStreamReference,
    Win32::System::{
        SystemInformation::{GetSystemTimePreciseAsFileTime, GetTickCount64},
        WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    },
};
type Session = GlobalSystemMediaTransportControlsSession;
type Manager = GlobalSystemMediaTransportControlsSessionManager;
const COVER_BUDGET: usize = 16 * 1024 * 1024;

/// Tests can observe only their own provider or no provider. Production uses All.
#[derive(Clone)]
pub enum SessionScope {
    All,
    Only(Vec<String>),
    None,
}
pub(super) struct Apartment(PhantomData<Rc<()>>);
impl Apartment {
    pub(super) fn new() -> AppResult<Self> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }.map_err(media_error)?;
        Ok(Self(PhantomData))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe {
            RoUninitialize();
        }
    }
}
struct ManagerEvents {
    native: Manager,
    sessions: Option<i64>,
    current: Option<i64>,
}
fn revoke(
    token: &mut Option<i64>,
    remove: impl FnOnce(i64) -> windows::core::Result<()>,
) -> AppResult<()> {
    if let Some(value) = *token {
        remove(value).map_err(media_error)?;
        *token = None;
    }
    Ok(())
}
impl ManagerEvents {
    fn close(&mut self) -> AppResult<()> {
        let sessions = revoke(&mut self.sessions, |token| {
            self.native.RemoveSessionsChanged(token)
        });
        let current = revoke(&mut self.current, |token| {
            self.native.RemoveCurrentSessionChanged(token)
        });
        sessions.and(current)
    }
}
impl Drop for ManagerEvents {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
struct Entry {
    native: Session,
    id: String,
    app: String,
    app_label: Option<String>,
    tokens: [Option<i64>; 3],
    metadata_dirty: Arc<AtomicBool>,
    title: String,
    artist: String,
    cover: Option<super::cover::Cover>,
    pending_cover: Option<IRandomAccessStreamReference>,
    cover_revision: u64,
}
impl Entry {
    fn close(&mut self) -> AppResult<()> {
        let media = revoke(&mut self.tokens[0], |token| {
            self.native.RemoveMediaPropertiesChanged(token)
        });
        let playback = revoke(&mut self.tokens[1], |token| {
            self.native.RemovePlaybackInfoChanged(token)
        });
        let timeline = revoke(&mut self.tokens[2], |token| {
            self.native.RemoveTimelinePropertiesChanged(token)
        });
        media.and(playback).and(timeline)
    }
}
impl Drop for Entry {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
struct ActiveCover {
    id: String,
    revision: u64,
    result: mpsc::Receiver<Option<super::cover::Cover>>,
}

pub struct MediaAdapter {
    entries: Vec<Entry>,
    manager: ManagerEvents,
    scope: SessionScope,
    changed: MediaChanged,
    cancel: Arc<AtomicBool>,
    next_id: u64,
    next_cover: usize,
    active_cover: Option<ActiveCover>,
    sessions_dirty: Arc<AtomicBool>,
    _apartment: Apartment,
}
impl MediaAdapter {
    fn receive_cover(&mut self) {
        let Some(active) = &self.active_cover else {
            return;
        };
        let result = match active.result.try_recv() {
            Ok(cover) => cover,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => None,
        };
        if let Some(entry) = self.entries.iter_mut().find(|entry| {
            entry.id == active.id && entry.cover_revision == active.revision
        }) {
            entry.cover = result;
        }
        self.active_cover = None;
    }

    fn schedule_cover(&mut self) {
        if self.active_cover.is_some() || self.cancel.load(Ordering::Acquire) {
            return;
        }
        let count = self.entries.len();
        let next = (0..count)
            .map(|offset| (self.next_cover + offset) % count)
            .find(|&index| self.entries[index].pending_cover.is_some());
        let Some(index) = next else {
            return;
        };
        let entry = &mut self.entries[index];
        let Some(reference) = entry.pending_cover.as_ref() else {
            return;
        };
        match super::cover::read_in_background(
            reference,
            self.cancel.clone(),
            self.changed.clone(),
        ) {
            Ok(Some(result)) => {
                self.active_cover = Some(ActiveCover {
                    id: entry.id.clone(),
                    revision: entry.cover_revision,
                    result,
                });
            }
            Ok(None) => return,
            Err(_) => entry.cover = None,
        }
        entry.pending_cover = None;
        self.next_cover = (index + 1) % count;
    }

    pub fn new(
        scope: SessionScope,
        changed: MediaChanged,
        cancel: Arc<AtomicBool>,
    ) -> AppResult<Self> {
        let apartment = Apartment::new()?;
        let operation = Manager::RequestAsync().map_err(media_error)?;
        let native = wait(&operation, &cancel, Instant::now() + Duration::from_secs(3))?;
        let mut manager = ManagerEvents {
            native,
            sessions: None,
            current: None,
        };
        let callback = changed.clone();
        let sessions_dirty = Arc::new(AtomicBool::new(true));
        let dirty = sessions_dirty.clone();
        manager.sessions = Some(
            manager
                .native
                .SessionsChanged(&TypedEventHandler::new(move |_, _| {
                    dirty.store(true, Ordering::Release);
                    callback();
                    Ok(())
                }))
                .map_err(media_error)?,
        );
        let callback = changed.clone();
        manager.current = Some(
            manager
                .native
                .CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
                    callback();
                    Ok(())
                }))
                .map_err(media_error)?,
        );
        Ok(Self {
            entries: Vec::new(),
            manager,
            scope,
            changed,
            cancel,
            next_id: 0,
            next_cover: 0,
            active_cover: None,
            sessions_dirty,
            _apartment: apartment,
        })
    }
    pub fn subscriptions(&self) -> usize {
        usize::from(self.manager.sessions.is_some())
            + usize::from(self.manager.current.is_some())
            + self
                .entries
                .iter()
                .map(|e| e.tokens.iter().flatten().count())
                .sum::<usize>()
    }
    fn subscribe(&mut self, native: Session, app: String) -> AppResult<Entry> {
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or_else(|| AppError::new("conflict", "Die Medienkennungen sind ausgeschöpft."))?;
        let app_label = windows::ApplicationModel::AppInfo::GetFromAppUserModelId(
            &windows::core::HSTRING::from(&app),
        )
        .and_then(|info| info.DisplayInfo())
        .and_then(|info| info.DisplayName())
        .ok()
        .map(limited)
        .filter(|name| !name.is_empty());
        let mut entry = Entry {
            native,
            id: format!("media-{}", self.next_id),
            app,
            app_label,
            tokens: [None; 3],
            metadata_dirty: Arc::new(AtomicBool::new(true)),
            title: String::new(),
            artist: String::new(),
            cover: None,
            pending_cover: None,
            cover_revision: 0,
        };
        let dirty = entry.metadata_dirty.clone();
        let callback = self.changed.clone();
        entry.tokens[0] = Some(
            entry
                .native
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    dirty.store(true, Ordering::Release);
                    callback();
                    Ok(())
                }))
                .map_err(media_error)?,
        );
        let callback = self.changed.clone();
        entry.tokens[1] = Some(
            entry
                .native
                .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                    callback();
                    Ok(())
                }))
                .map_err(media_error)?,
        );
        let callback = self.changed.clone();
        entry.tokens[2] = Some(
            entry
                .native
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    callback();
                    Ok(())
                }))
                .map_err(media_error)?,
        );
        Ok(entry)
    }
    fn reconcile(&mut self) -> AppResult<()> {
        // GetSessions can return fresh COM wrappers. Retain live session objects
        // between list events, and use the public source ID only when unique.
        let dead = self.entries.iter().any(|entry| !live(&entry.native));
        if !self.sessions_dirty.swap(false, Ordering::AcqRel) && !dead {
            return Ok(());
        }
        let result = self.reconcile_list();
        if result.is_err() {
            self.sessions_dirty.store(true, Ordering::Release);
        }
        result
    }
    fn reconcile_list(&mut self) -> AppResult<()> {
        let sessions = self.manager.native.GetSessions().map_err(media_error)?;
        let count = sessions.Size().map_err(media_error)?;
        if count > 256 {
            return Err(AppError::new(
                "media_error",
                "Windows meldet zu viele Mediensitzungen.",
            ));
        }
        let mut found = Vec::new();
        for index in 0..count {
            let session = sessions.GetAt(index).map_err(media_error)?;
            let app = session
                .SourceAppUserModelId()
                .map_err(media_error)?
                .to_string_lossy();
            if match &self.scope {
                SessionScope::All => true,
                SessionScope::Only(ids) => ids.contains(&app),
                SessionScope::None => false,
            } && live(&session)
            {
                found.push((session, app));
            }
        }
        let mut old_counts = std::collections::HashMap::new();
        let mut new_counts = std::collections::HashMap::new();
        for entry in &self.entries {
            *old_counts.entry(entry.app.clone()).or_insert(0_usize) += 1;
        }
        for (_, app) in &found {
            *new_counts.entry(app.clone()).or_insert(0_usize) += 1;
        }
        let keep = |entry: &Entry| {
            live(&entry.native)
                && found.iter().any(|(native, app)| {
                    same_session(native, &entry.native)
                        || (entry.app == *app && unique_source(old_counts[app], new_counts[app]))
                })
        };
        for entry in &mut self.entries {
            if !keep(entry) {
                entry.close()?;
            }
        }
        self.entries.retain(keep);
        for (native, app) in found {
            if !self.entries.iter().any(|entry| {
                same_session(&entry.native, &native) || (entry.app == app && new_counts[&app] == 1)
            }) {
                let entry = self.subscribe(native, app)?;
                self.entries.push(entry);
            }
        }
        Ok(())
    }
}
fn unique_source(old: usize, new: usize) -> bool {
    old == 1 && new == 1
}
fn live(native: &Session) -> bool {
    native
        .GetPlaybackInfo()
        .and_then(|info| info.PlaybackStatus())
        .is_ok_and(|status| {
            status != GlobalSystemMediaTransportControlsSessionPlaybackStatus::Closed
        })
}
#[test]
fn source_fallback_never_merges_ambiguous_or_replaced_sessions() {
    assert!(unique_source(1, 1));
    for counts in [(0, 1), (1, 0), (2, 1), (1, 2), (2, 2)] {
        assert!(!unique_source(counts.0, counts.1));
    }
}
fn same_session(a: &Session, b: &Session) -> bool {
    a.cast::<windows::core::IUnknown>()
        .ok()
        .zip(b.cast::<windows::core::IUnknown>().ok())
        .is_some_and(|(a, b)| a == b)
}
fn limited(value: windows::core::HSTRING) -> String {
    value.to_string_lossy().chars().take(1024).collect()
}
fn capabilities(native: &Session) -> AppResult<MediaCapabilities> {
    let controls = native
        .GetPlaybackInfo()
        .and_then(|info| info.Controls())
        .map_err(media_error)?;
    Ok(MediaCapabilities {
        play: controls.IsPlayEnabled().map_err(media_error)?,
        pause: controls.IsPauseEnabled().map_err(media_error)?,
        toggle: controls.IsPlayPauseToggleEnabled().map_err(media_error)?,
        previous: controls.IsPreviousEnabled().map_err(media_error)?,
        next: controls.IsNextEnabled().map_err(media_error)?,
        seek: controls.IsPlaybackPositionEnabled().map_err(media_error)?,
    })
}
fn relative_ms(ticks: i64, start: i64, duration: u64) -> u64 {
    ((i128::from(ticks) - i128::from(start)).max(0) / 10_000).min(i128::from(duration)) as u64
}
#[test]
fn native_ticks_are_relative_to_start_and_saturate_without_overflow() {
    assert_eq!(relative_ms(150_000, 100_000, 100), 5);
    assert_eq!(relative_ms(-50_000, -100_000, 100), 5);
    assert_eq!(relative_ms(-150_000, -100_000, 100), 0);
    assert_eq!(relative_ms(i64::MAX, i64::MIN, 100), 100);
}
fn timeline(native: &Session, playing: bool) -> AppResult<(MediaTimeline, i64)> {
    let native_time = native.GetTimelineProperties().map_err(media_error)?;
    let start = native_time.StartTime().map_err(media_error)?.Duration;
    let end = native_time.EndTime().map_err(media_error)?.Duration;
    let duration = ((i128::from(end) - i128::from(start)).max(0) / 10_000) as u64;
    let min = relative_ms(
        native_time.MinSeekTime().map_err(media_error)?.Duration,
        start,
        duration,
    );
    let max = relative_ms(
        native_time.MaxSeekTime().map_err(media_error)?.Duration,
        start,
        duration,
    )
    .max(min);
    let rate = native
        .GetPlaybackInfo()
        .ok()
        .and_then(|info| info.PlaybackRate().ok())
        .and_then(|value| value.Value().ok())
        .filter(|rate| rate.is_finite())
        .unwrap_or(1.);
    let updated = native_time
        .LastUpdatedTime()
        .map_err(media_error)?
        .UniversalTime;
    let now = unsafe { GetSystemTimePreciseAsFileTime() };
    let utc = ((u64::from(now.dwHighDateTime) << 32) | u64::from(now.dwLowDateTime)) as i64;
    let elapsed = if updated > 0 {
        utc.saturating_sub(updated).max(0) as f64 / 10_000.
    } else {
        0.
    };
    let position = relative_ms(
        native_time.Position().map_err(media_error)?.Duration,
        start,
        duration,
    ) as f64;
    let position = if playing {
        position + elapsed * rate
    } else {
        position
    };
    Ok((
        MediaTimeline {
            position_ms: position.clamp(0., duration as f64) as u64,
            duration_ms: duration,
            seek_start_ms: min,
            seek_end_ms: max,
            sampled_at_ms: unsafe { GetTickCount64() },
            rate,
        },
        start,
    ))
}
impl MediaBackend for MediaAdapter {
    fn subscriptions(&self) -> usize {
        MediaAdapter::subscriptions(self)
    }
    fn close(&mut self) -> AppResult<()> {
        self.cancel.store(true, Ordering::Release);
        self.active_cover = None;
        let mut result = Ok(());
        for entry in &mut self.entries {
            if let Err(error) = entry.close() {
                result = Err(error);
            }
        }
        // Failed tokens survive until Drop retries. Never report their
        // unconfirmed unregistration as a count of zero.
        self.entries
            .retain(|entry| entry.tokens.iter().any(Option::is_some));
        result.and(self.manager.close())
    }
    fn has_pending_cover(&self) -> bool {
        self.active_cover.is_none()
            && self.entries.iter().any(|entry| entry.pending_cover.is_some())
    }

    fn snapshot(&mut self, refresh_cover: bool) -> AppResult<BackendSnapshot> {
        self.reconcile()?;
        let current = self.manager.native.GetCurrentSession().ok();
        let current_id = current.as_ref().and_then(|current| {
            self.entries
                .iter()
                .find(|entry| same_session(current, &entry.native))
                .map(|entry| entry.id.clone())
                .or_else(|| {
                    let app = current.SourceAppUserModelId().ok()?.to_string_lossy();
                    let matches = self
                        .entries
                        .iter()
                        .filter(|entry| entry.app == app)
                        .collect::<Vec<_>>();
                    (matches.len() == 1).then(|| matches[0].id.clone())
                })
        });
        let mut snapshot = BackendSnapshot {
            current_id,
            ..Default::default()
        };
        let deadline = Instant::now() + Duration::from_secs(2);
        for entry in &mut self.entries {
            if entry.metadata_dirty.swap(false, Ordering::AcqRel) {
                entry.cover_revision = entry.cover_revision.wrapping_add(1);
                match entry
                    .native
                    .TryGetMediaPropertiesAsync()
                    .map_err(media_error)
                    .and_then(|op| wait(&op, &self.cancel, deadline))
                {
                    Ok(properties) => {
                        let title = properties.Title().map(limited).unwrap_or_default();
                        let artist = properties.Artist().map(limited).unwrap_or_default();
                        entry.pending_cover = properties.Thumbnail().ok();
                        if title != entry.title
                            || artist != entry.artist
                            || entry.pending_cover.is_none()
                        {
                            entry.cover = None;
                        }
                        entry.title = title;
                        entry.artist = artist;
                    }
                    Err(_) => {
                        entry.metadata_dirty.store(true, Ordering::Release);
                        entry.title.clear();
                        entry.artist.clear();
                        entry.cover = None;
                        entry.pending_cover = None;
                    }
                }
            }
        }
        self.receive_cover();
        if refresh_cover {
            self.schedule_cover();
        }
        let mut cover_bytes = 0;
        for entry in &mut self.entries {
            if let Some(cover) = &mut entry.cover {
                if let Some(bytes) = snapshot.covers.get(&cover.key) {
                    cover.bytes = bytes.clone();
                } else if cover_bytes + cover.bytes.len() <= COVER_BUDGET {
                    cover_bytes += cover.bytes.len();
                    snapshot.covers.insert(cover.key.clone(), cover.bytes.clone());
                } else {
                    entry.cover = None;
                }
            }
            let playing = entry
                .native
                .GetPlaybackInfo()
                .and_then(|info| info.PlaybackStatus())
                .map_err(media_error)?
                == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
            let (time, _) = timeline(&entry.native, playing)?;
            let caps = capabilities(&entry.native)?;
            snapshot.sessions.push(MediaSession {
                id: entry.id.clone(),
                app: entry.app.clone(),
                app_label: entry.app_label.clone(),
                title: entry.title.clone(),
                artist: entry.artist.clone(),
                playing,
                cover_key: entry.cover.as_ref().map(|cover| cover.key.clone()),
                cover_tint: entry.cover.as_ref().and_then(|cover| cover.tint),
                timeline: time,
                capabilities: caps,
            });
        }
        Ok(snapshot)
    }
    fn execute(&mut self, id: &str, command: MediaCommand) -> AppResult<()> {
        self.reconcile()?;
        let entry = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| AppError::new("not_found", "Diese Mediensitzung wurde beendet."))?;
        let caps = capabilities(&entry.native)?;
        let operation = match command {
            MediaCommand::Toggle if caps.toggle => entry.native.TryTogglePlayPauseAsync(),
            MediaCommand::Play if caps.play => entry.native.TryPlayAsync(),
            MediaCommand::Pause if caps.pause => entry.native.TryPauseAsync(),
            MediaCommand::Previous if caps.previous => entry.native.TrySkipPreviousAsync(),
            MediaCommand::Next if caps.next => entry.native.TrySkipNextAsync(),
            MediaCommand::Seek(ms) if caps.seek => {
                let (time, start) = timeline(&entry.native, false)?;
                if ms < time.seek_start_ms
                    || ms > time.seek_end_ms
                    || time.seek_start_ms >= time.seek_end_ms
                {
                    return Err(AppError::new(
                        "invalid_argument",
                        "Die Wiedergabeposition liegt außerhalb des verfügbaren Bereichs.",
                    ));
                }
                let ticks =
                    i64::try_from(i128::from(start) + i128::from(ms) * 10_000).map_err(|_| {
                        AppError::new("invalid_argument", "Die Wiedergabeposition ist zu groß.")
                    })?;
                entry.native.TryChangePlaybackPositionAsync(ticks)
            }
            _ => {
                return Err(AppError::new(
                    "unsupported",
                    "Diese Medienaktion wird vom Player nicht angeboten.",
                ))
            }
        }
        .map_err(media_error)?;
        if !wait(
            &operation,
            &self.cancel,
            Instant::now() + Duration::from_secs(3),
        )? {
            return Err(AppError::new(
                "media_error",
                "Der Player hat die Medienaktion nicht bestätigt.",
            ));
        }
        Ok(())
    }
}
impl Drop for MediaAdapter {
    fn drop(&mut self) {
        let _ = MediaBackend::close(self);
    }
}
