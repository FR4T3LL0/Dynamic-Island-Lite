use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};

const SECOND: u64 = 1_000;
const MINUTE: u64 = 60 * SECOND;
const MAX_DURATION: u64 = 24 * 60 * MINUTE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimerMode {
    #[default]
    Countdown,
    Pomodoro,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimerPhase {
    #[default]
    Work,
    Short,
    Long,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TimerStatus {
    #[default]
    Idle,
    Running,
    Paused,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TimerState {
    pub schema_version: u32,
    pub mode: TimerMode,
    pub phase: TimerPhase,
    pub status: TimerStatus,
    pub duration_ms: u64,
    pub remaining_ms: u64,
    pub deadline_utc_ms: Option<i64>,
    pub completed_work: u32,
    pub notified: bool,
}

impl Default for TimerState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            mode: TimerMode::Countdown,
            phase: TimerPhase::Work,
            status: TimerStatus::Idle,
            duration_ms: 25 * MINUTE,
            remaining_ms: 25 * MINUTE,
            deadline_utc_ms: None,
            completed_work: 0,
            notified: false,
        }
    }
}

impl TimerState {
    pub fn validate(&self) -> AppResult<()> {
        if self.schema_version != 1
            || !(SECOND..=MAX_DURATION).contains(&self.duration_ms)
            || self.remaining_ms > self.duration_ms
            || (self.status == TimerStatus::Running && self.deadline_utc_ms.is_none())
        {
            return Err(AppError::new(
                "invalid_input",
                "Die gespeicherten Timerdaten sind ungültig.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Clock {
    pub utc_ms: i64,
    pub monotonic_ms: u64,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct Tick {
    pub expired: bool,
    pub persist: bool,
}

/// Runtime deadlines use monotonic time. Only the UTC deadline crosses a restart.
#[derive(Debug, Clone)]
pub struct TimerEngine {
    state: TimerState,
    deadline_mono: Option<u64>,
    previous_clock: Clock,
}

impl TimerEngine {
    pub fn restore(mut state: TimerState, clock: Clock) -> AppResult<Self> {
        state.validate()?;
        let deadline_mono = if state.status == TimerStatus::Running {
            let remaining = state
                .deadline_utc_ms
                .unwrap()
                .saturating_sub(clock.utc_ms)
                .clamp(0, state.duration_ms as i64) as u64;
            state.remaining_ms = remaining;
            Some(clock.monotonic_ms.saturating_add(remaining))
        } else {
            None
        };
        Ok(Self {
            state,
            deadline_mono,
            previous_clock: clock,
        })
    }

    pub fn state(&self) -> &TimerState {
        &self.state
    }

    pub fn configure(&mut self, mode: TimerMode, seconds: u64) -> AppResult<()> {
        if self.state.status == TimerStatus::Running {
            return Err(AppError::new(
                "conflict",
                "Bitte den laufenden Timer zuerst pausieren.",
            ));
        }
        if !(1..=86_400).contains(&seconds) {
            return Err(AppError::new(
                "invalid_input",
                "Der Countdown muss zwischen einer Sekunde und 24 Stunden liegen.",
            ));
        }
        let duration = if mode == TimerMode::Pomodoro {
            25 * MINUTE
        } else {
            seconds * SECOND
        };
        self.state = TimerState {
            mode,
            duration_ms: duration,
            remaining_ms: duration,
            ..TimerState::default()
        };
        self.deadline_mono = None;
        Ok(())
    }

    pub fn start(&mut self, clock: Clock) -> AppResult<()> {
        if self.state.status == TimerStatus::Running {
            return Err(AppError::new("conflict", "Der Timer läuft bereits."));
        }
        if self.state.status == TimerStatus::Completed {
            if self.state.mode == TimerMode::Pomodoro {
                self.state.phase = match self.state.phase {
                    TimerPhase::Work if self.state.completed_work.is_multiple_of(4) => {
                        TimerPhase::Long
                    }
                    TimerPhase::Work => TimerPhase::Short,
                    TimerPhase::Short | TimerPhase::Long => TimerPhase::Work,
                };
                self.state.duration_ms = match self.state.phase {
                    TimerPhase::Work => 25 * MINUTE,
                    TimerPhase::Short => 5 * MINUTE,
                    TimerPhase::Long => 15 * MINUTE,
                };
            }
            self.state.remaining_ms = self.state.duration_ms;
        }
        self.state.status = TimerStatus::Running;
        self.state.notified = false;
        self.state.deadline_utc_ms =
            Some(clock.utc_ms.saturating_add(self.state.remaining_ms as i64));
        self.deadline_mono = Some(clock.monotonic_ms.saturating_add(self.state.remaining_ms));
        self.previous_clock = clock;
        Ok(())
    }

    pub fn pause(&mut self, clock: Clock) -> AppResult<()> {
        if self.state.status != TimerStatus::Running {
            return Err(AppError::new("conflict", "Der Timer läuft nicht."));
        }
        self.tick(clock);
        if self.state.status == TimerStatus::Running {
            self.state.status = TimerStatus::Paused;
            self.state.deadline_utc_ms = None;
            self.deadline_mono = None;
        }
        Ok(())
    }

    pub fn reset(&mut self) {
        let mode = self.state.mode;
        let duration = if mode == TimerMode::Pomodoro {
            25 * MINUTE
        } else {
            self.state.duration_ms
        };
        self.state = TimerState {
            mode,
            duration_ms: duration,
            remaining_ms: duration,
            ..TimerState::default()
        };
        self.deadline_mono = None;
    }

    pub fn tick(&mut self, clock: Clock) -> Tick {
        if self.state.status != TimerStatus::Running {
            return Tick::default();
        }
        let remaining = self
            .deadline_mono
            .unwrap_or(clock.monotonic_ms)
            .saturating_sub(clock.monotonic_ms);
        self.state.remaining_ms = remaining;
        let utc_delta = clock.utc_ms.saturating_sub(self.previous_clock.utc_ms);
        let mono_delta = clock
            .monotonic_ms
            .saturating_sub(self.previous_clock.monotonic_ms) as i64;
        let rebase = utc_delta.saturating_sub(mono_delta).unsigned_abs() > SECOND;
        if rebase {
            self.state.deadline_utc_ms = Some(clock.utc_ms.saturating_add(remaining as i64));
        }
        self.previous_clock = clock;
        if remaining > 0 {
            return Tick {
                expired: false,
                persist: rebase,
            };
        }
        self.state.status = TimerStatus::Completed;
        self.state.deadline_utc_ms = None;
        self.deadline_mono = None;
        if self.state.mode == TimerMode::Pomodoro && self.state.phase == TimerPhase::Work {
            self.state.completed_work = self.state.completed_work.saturating_add(1);
        }
        let expired = !self.state.notified;
        self.state.notified = true;
        Tick {
            expired,
            persist: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn clock(ms: u64) -> Clock {
        Clock {
            utc_ms: 1_000_000 + ms as i64,
            monotonic_ms: ms,
        }
    }
    fn timer() -> TimerEngine {
        TimerEngine::restore(TimerState::default(), clock(0)).unwrap()
    }

    #[test]
    fn count_down_pauses_resumes_and_expires_once() {
        let mut engine = timer();
        engine.configure(TimerMode::Countdown, 10).unwrap();
        engine.start(clock(0)).unwrap();
        engine.pause(clock(3_000)).unwrap();
        assert_eq!(engine.state().remaining_ms, 7_000);
        engine.tick(clock(50_000));
        assert_eq!(engine.state().remaining_ms, 7_000);
        engine.start(clock(50_000)).unwrap();
        assert!(!engine.tick(clock(56_999)).expired);
        assert!(engine.tick(clock(57_000)).expired);
        assert!(!engine.tick(clock(58_000)).expired);
        assert_eq!(engine.state().status, TimerStatus::Completed);
    }
    #[test]
    fn system_clock_changes_do_not_change_running_remaining_time() {
        let mut engine = timer();
        engine.configure(TimerMode::Countdown, 60).unwrap();
        engine.start(clock(0)).unwrap();
        let tick = engine.tick(Clock {
            utc_ms: 8_000_000,
            monotonic_ms: 10_000,
        });
        assert!(!tick.expired);
        assert!(tick.persist);
        assert_eq!(engine.state().remaining_ms, 50_000);
        assert_eq!(engine.state().deadline_utc_ms, Some(8_050_000));
        let tick = engine.tick(Clock {
            utc_ms: 7_000_000,
            monotonic_ms: 20_000,
        });
        assert!(tick.persist);
        assert_eq!(engine.state().remaining_ms, 40_000);
        assert_eq!(engine.state().deadline_utc_ms, Some(7_040_000));
    }
    #[test]
    fn restart_and_sleep_use_elapsed_time_without_repeating_alarm() {
        let mut engine = timer();
        engine.configure(TimerMode::Countdown, 10).unwrap();
        engine.start(clock(0)).unwrap();
        let persisted = engine.state().clone();
        let mut resumed = TimerEngine::restore(persisted, clock(30_000)).unwrap();
        assert!(resumed.tick(clock(30_000)).expired);
        let mut restarted = TimerEngine::restore(resumed.state().clone(), clock(40_000)).unwrap();
        assert!(!restarted.tick(clock(40_000)).expired);
        let mut paused = timer();
        paused.start(clock(0)).unwrap();
        paused.pause(clock(3_000)).unwrap();
        let paused = TimerEngine::restore(paused.state().clone(), clock(100_000)).unwrap();
        assert_eq!(paused.state().remaining_ms, 25 * MINUTE - 3_000);
    }
    #[test]
    fn pomodoro_waits_for_click_and_uses_long_break_after_four_work_sessions() {
        let mut engine = timer();
        engine.configure(TimerMode::Pomodoro, 1).unwrap();
        let mut elapsed = 0;
        for session in 1..=4 {
            engine.start(clock(elapsed)).unwrap();
            assert_eq!(engine.state().phase, TimerPhase::Work);
            elapsed += 25 * MINUTE;
            assert!(engine.tick(clock(elapsed)).expired);
            assert_eq!(engine.state().status, TimerStatus::Completed);
            engine.tick(clock(elapsed + 1_000));
            assert_eq!(engine.state().status, TimerStatus::Completed);
            engine.start(clock(elapsed)).unwrap();
            assert_eq!(engine.state().completed_work, session);
            assert_eq!(
                engine.state().phase,
                if session == 4 {
                    TimerPhase::Long
                } else {
                    TimerPhase::Short
                }
            );
            elapsed += if session == 4 {
                15 * MINUTE
            } else {
                5 * MINUTE
            };
            engine.tick(clock(elapsed));
        }
        engine.reset();
        assert_eq!(engine.state().phase, TimerPhase::Work);
        assert_eq!(engine.state().completed_work, 0);
    }
    #[test]
    fn rejects_invalid_duration_and_conflicting_configuration() {
        let mut engine = timer();
        let original = engine.state().clone();
        assert!(engine.configure(TimerMode::Countdown, 0).is_err());
        assert!(engine.configure(TimerMode::Countdown, 86_401).is_err());
        assert_eq!(engine.state(), &original);
        engine.start(clock(0)).unwrap();
        assert!(engine.configure(TimerMode::Pomodoro, 60).is_err());
        assert!(engine.start(clock(1_000)).is_err());
    }
}
