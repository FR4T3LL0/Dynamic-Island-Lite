//! Deterministic media selection and capability policy. System events, rather
//! than optimistic commands, confirm playback state. No platform COM pointers.
use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaCapabilities {
    pub play: bool,
    pub pause: bool,
    pub toggle: bool,
    pub previous: bool,
    pub next: bool,
    pub seek: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTimeline {
    pub position_ms: u64,
    pub duration_ms: u64,
    pub seek_start_ms: u64,
    pub seek_end_ms: u64,
    pub sampled_at_ms: u64,
    pub rate: f64,
}
impl MediaTimeline {
    pub fn validate(&self) -> AppResult<()> {
        if !self.rate.is_finite()
            || self.position_ms > self.duration_ms
            || self.seek_start_ms > self.seek_end_ms
            || self.seek_end_ms > self.duration_ms
        {
            return Err(AppError::new(
                "invalid_argument",
                "Die Medienzeitachse ist ungültig.",
            ));
        }
        Ok(())
    }
    pub fn position_at(&self, now_ms: u64, playing: bool) -> u64 {
        if !playing || !self.rate.is_finite() {
            return self.position_ms.min(self.duration_ms);
        }
        (self.position_ms as f64 + now_ms.saturating_sub(self.sampled_at_ms) as f64 * self.rate)
            .clamp(0., self.duration_ms as f64) as u64
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSession {
    pub id: String,
    pub app: String,
    #[serde(default)]
    pub app_label: Option<String>,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub cover_key: Option<String>,
    pub cover_tint: Option<[u8; 3]>,
    pub timeline: MediaTimeline,
    pub capabilities: MediaCapabilities,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaAction {
    Toggle,
    Previous,
    Next,
    Seek(u64),
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaCommand {
    Toggle,
    Play,
    Pause,
    Previous,
    Next,
    Seek(u64),
}
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    pub enabled: bool,
    pub generation: u64,
    pub revision: u64,
    pub sessions: Vec<MediaSession>,
    pub selected_id: Option<String>,
    #[serde(skip)]
    preferred_id: Option<String>,
}
fn conflict(message: &str) -> AppError {
    AppError::new("conflict", message)
}
impl MediaState {
    pub fn set_enabled(&mut self, enabled: bool) -> AppResult<bool> {
        if self.enabled == enabled {
            return Ok(false);
        }
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| conflict("Die Generation des Mediendienstes ist ausgeschöpft."))?;
        let revision = self.next_revision()?;
        self.enabled = enabled;
        self.generation = generation;
        self.revision = revision;
        self.sessions.clear();
        self.selected_id = None;
        self.preferred_id = None;
        Ok(true)
    }
    fn next_revision(&self) -> AppResult<u64> {
        self.revision
            .checked_add(1)
            .ok_or_else(|| conflict("Die Medienrevision ist ausgeschöpft."))
    }
    pub fn update(
        &mut self,
        generation: u64,
        sessions: Vec<MediaSession>,
        current_id: Option<&str>,
    ) -> AppResult<bool> {
        if !self.enabled || generation != self.generation {
            return Ok(false);
        }
        let mut ids = HashSet::new();
        for session in &sessions {
            if session.id.is_empty() || !ids.insert(session.id.as_str()) {
                return Err(AppError::new(
                    "invalid_argument",
                    "Mediensitzungen benötigen eindeutige Kennungen.",
                ));
            }
            session.timeline.validate()?;
        }
        let preferred = self
            .preferred_id
            .as_deref()
            .filter(|id| ids.contains(id))
            .map(str::to_owned);
        let current = sessions
            .iter()
            .find(|session| Some(session.id.as_str()) == current_id);
        let previous = sessions
            .iter()
            .find(|session| Some(&session.id) == self.selected_id.as_ref());
        let selected = preferred.clone().or_else(|| {
            // Windows can make a paused source current when its window gains focus.
            current
                .filter(|session| session.playing)
                .or(previous.filter(|session| session.playing))
                .or_else(|| sessions.iter().find(|session| session.playing))
                .or(previous)
                .or(current)
                .or_else(|| sessions.first())
                .map(|session| session.id.clone())
        });
        if sessions == self.sessions
            && selected == self.selected_id
            && preferred == self.preferred_id
        {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        self.sessions = sessions;
        self.selected_id = selected;
        self.preferred_id = preferred;
        self.revision = revision;
        Ok(true)
    }
    pub fn select(&mut self, id: &str) -> AppResult<bool> {
        if !self.enabled || !self.sessions.iter().any(|session| session.id == id) {
            return Err(AppError::new(
                "not_found",
                "Diese Mediensitzung ist nicht mehr verfügbar.",
            ));
        }
        if self.preferred_id.as_deref() == Some(id) && self.selected_id.as_deref() == Some(id) {
            return Ok(false);
        }
        let revision = self.next_revision()?;
        self.preferred_id = Some(id.into());
        self.selected_id = Some(id.into());
        self.revision = revision;
        Ok(true)
    }
    pub fn command(
        &self,
        generation: u64,
        id: &str,
        action: MediaAction,
    ) -> AppResult<MediaCommand> {
        if !self.enabled || generation != self.generation || self.selected_id.as_deref() != Some(id)
        {
            return Err(conflict("Die ausgewählte Mediensitzung hat sich geändert."));
        }
        let session = self
            .sessions
            .iter()
            .find(|session| session.id == id)
            .ok_or_else(|| AppError::new("not_found", "Die Mediensitzung wurde beendet."))?;
        let caps = &session.capabilities;
        let command = match action {
            MediaAction::Toggle if caps.toggle => MediaCommand::Toggle,
            MediaAction::Toggle if session.playing && caps.pause => MediaCommand::Pause,
            MediaAction::Toggle if !session.playing && caps.play => MediaCommand::Play,
            MediaAction::Previous if caps.previous => MediaCommand::Previous,
            MediaAction::Next if caps.next => MediaCommand::Next,
            MediaAction::Seek(position)
                if caps.seek && session.timeline.seek_start_ms < session.timeline.seek_end_ms =>
            {
                if position < session.timeline.seek_start_ms
                    || position > session.timeline.seek_end_ms
                {
                    return Err(AppError::new(
                        "invalid_argument",
                        "Die Wiedergabeposition liegt außerhalb des verfügbaren Bereichs.",
                    ));
                }
                MediaCommand::Seek(position)
            }
            _ => {
                return Err(AppError::new(
                    "unsupported",
                    "Diese Medienaktion wird vom Player nicht angeboten.",
                ))
            }
        };
        Ok(command)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn session(id: &str, playing: bool) -> MediaSession {
        MediaSession {
            id: id.into(),
            app: "Eigener Testplayer".into(),
            app_label: None,
            title: String::new(),
            artist: String::new(),
            playing,
            cover_key: None,
            cover_tint: None,
            timeline: MediaTimeline {
                position_ms: 10000,
                duration_ms: 120000,
                seek_start_ms: 3000,
                seek_end_ms: 90000,
                sampled_at_ms: 1000,
                rate: 1.,
            },
            capabilities: MediaCapabilities::default(),
        }
    }
    fn active() -> MediaState {
        let mut state = MediaState::default();
        state.set_enabled(true).unwrap();
        state
    }
    #[test]
    fn os_selection_manual_preference_and_session_end_have_deterministic_fallbacks() {
        let mut state = active();
        let a = session("a", false);
        let b = session("b", true);
        state
            .update(state.generation, vec![a.clone(), b.clone()], Some("a"))
            .unwrap();
        assert_eq!(state.selected_id.as_deref(), Some("b"));
        state.select("a").unwrap();
        state
            .update(state.generation, vec![a, b.clone()], Some("b"))
            .unwrap();
        assert_eq!(state.selected_id.as_deref(), Some("a"));
        state.update(state.generation, vec![b], Some("a")).unwrap();
        assert_eq!(state.selected_id.as_deref(), Some("b"));
        assert!(state.preferred_id.is_none());
        let before = state.clone();
        assert!(state.select("gone").is_err());
        assert_eq!(state, before);
    }
    #[test]
    fn a_paused_player_remains_selected_when_another_paused_app_takes_focus() {
        let mut state = active();
        let other = session("other", false);
        state
            .update(
                state.generation,
                vec![session("player", true), other.clone()],
                Some("player"),
            )
            .unwrap();
        state
            .update(
                state.generation,
                vec![session("player", false), other],
                Some("other"),
            )
            .unwrap();
        assert_eq!(state.selected_id.as_deref(), Some("player"));
    }
    #[test]
    fn changing_windows_current_to_a_paused_source_retains_the_playing_source() {
        let mut state = active();
        let selected = session("playing", true);
        let other = session("paused", false);
        let another_playing = session("other-playing", true);
        state
            .update(
                state.generation,
                vec![selected.clone(), other.clone()],
                Some("playing"),
            )
            .unwrap();
        state
            .update(
                state.generation,
                vec![other, another_playing, selected],
                Some("paused"),
            )
            .unwrap();
        assert_eq!(state.selected_id.as_deref(), Some("playing"));
    }
    #[test]
    fn missing_current_prefers_playing_then_previous_and_empty_clears_metadata() {
        let mut state = active();
        let a = session("a", false);
        let b = session("b", true);
        state
            .update(state.generation, vec![a.clone(), b], Some("gone"))
            .unwrap();
        assert_eq!(state.selected_id.as_deref(), Some("b"));
        state
            .update(state.generation, vec![a.clone(), session("b", false)], None)
            .unwrap();
        assert_eq!(state.selected_id.as_deref(), Some("b"));
        assert_eq!(state.sessions[0].title, "");
        assert!(state.sessions[0].cover_key.is_none());
        state.update(state.generation, vec![], None).unwrap();
        assert!(state.selected_id.is_none());
    }
    #[test]
    fn ten_lifecycles_reject_old_callbacks_and_repeated_start_stop_is_idempotent() {
        let mut state = MediaState::default();
        for _ in 0..10 {
            state.set_enabled(true).unwrap();
            let generation = state.generation;
            let before = state.clone();
            assert!(!state.set_enabled(true).unwrap());
            assert_eq!(state, before);
            state
                .update(generation, vec![session("reused", true)], None)
                .unwrap();
            state.set_enabled(false).unwrap();
            let stopped = state.clone();
            assert!(!state
                .update(generation, vec![session("reused", false)], None)
                .unwrap());
            assert!(!state.set_enabled(false).unwrap());
            assert_eq!(state, stopped);
            state.set_enabled(true).unwrap();
            let restarted = state.clone();
            assert!(!state
                .update(generation, vec![session("reused", true)], None)
                .unwrap());
            assert_eq!(state, restarted);
            state.set_enabled(false).unwrap();
        }
    }
    #[test]
    fn commands_use_capabilities_without_optimistically_changing_playback() {
        let mut state = active();
        let mut player = session("a", false);
        player.capabilities.play = true;
        state
            .update(state.generation, vec![player.clone()], None)
            .unwrap();
        let before = state.clone();
        assert_eq!(
            state
                .command(state.generation, "a", MediaAction::Toggle)
                .unwrap(),
            MediaCommand::Play
        );
        assert!(state
            .command(state.generation, "a", MediaAction::Next)
            .is_err());
        assert_eq!(state, before);
        player.playing = true;
        player.capabilities.pause = true;
        state
            .update(state.generation, vec![player.clone()], None)
            .unwrap();
        assert_eq!(
            state
                .command(state.generation, "a", MediaAction::Toggle)
                .unwrap(),
            MediaCommand::Pause
        );
        player.capabilities.toggle = true;
        state.update(state.generation, vec![player], None).unwrap();
        assert_eq!(
            state
                .command(state.generation, "a", MediaAction::Toggle)
                .unwrap(),
            MediaCommand::Toggle
        );
    }
    #[test]
    fn seeking_uses_actual_seek_range_and_stale_selection_never_controls_another_player() {
        let mut state = active();
        let mut a = session("a", true);
        a.capabilities.seek = true;
        state
            .update(state.generation, vec![a, session("b", false)], None)
            .unwrap();
        assert_eq!(
            state
                .command(state.generation, "a", MediaAction::Seek(3000))
                .unwrap(),
            MediaCommand::Seek(3000)
        );
        assert!(state
            .command(state.generation, "a", MediaAction::Seek(2999))
            .is_err());
        assert!(state
            .command(state.generation, "a", MediaAction::Seek(90001))
            .is_err());
        state.select("b").unwrap();
        assert!(state
            .command(state.generation, "a", MediaAction::Seek(3000))
            .is_err());
        assert!(state
            .command(state.generation - 1, "b", MediaAction::Toggle)
            .is_err());
    }
    #[test]
    fn progress_is_monotonic_time_based_bounded_and_frozen_while_paused() {
        let mut timeline = session("a", true).timeline;
        assert_eq!(timeline.position_at(2500, true), 11500);
        assert_eq!(timeline.position_at(2500, false), 10000);
        assert_eq!(timeline.position_at(0, true), 10000);
        assert_eq!(timeline.position_at(u64::MAX, true), 120000);
        timeline.rate = 2.;
        assert_eq!(timeline.position_at(2500, true), 13000);
        timeline.rate = -1.;
        assert_eq!(timeline.position_at(50000, true), 0);
    }
    #[test]
    fn invalid_replacement_preserves_confirmed_state() {
        let mut state = active();
        state
            .update(state.generation, vec![session("a", true)], None)
            .unwrap();
        let before = state.clone();
        let mut invalid = session("b", false);
        invalid.timeline.rate = f64::NAN;
        assert!(state.update(state.generation, vec![invalid], None).is_err());
        assert_eq!(state, before);
        assert!(state
            .update(
                state.generation,
                vec![session("same", true), session("same", false)],
                None
            )
            .is_err());
        assert_eq!(state, before);
        assert!(state
            .update(state.generation, vec![session("", false)], None)
            .is_err());
        assert_eq!(state, before);
    }
    #[test]
    fn exhausted_ordering_counters_do_not_partially_change_service_state() {
        let mut state = MediaState {
            generation: u64::MAX,
            ..Default::default()
        };
        let before = state.clone();
        assert!(state.set_enabled(true).is_err());
        assert_eq!(state, before);
        let mut state = active();
        state.revision = u64::MAX;
        let before = state.clone();
        assert!(state
            .update(state.generation, vec![session("a", true)], None)
            .is_err());
        assert_eq!(state, before);
    }
}
