use std::time::{Duration, Instant};

const HALL_PASS: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    ActiveFocus,
    HallPass,
    Break,
    Completed,
    Aborted,
}

impl Phase {
    pub fn is_running(self) -> bool {
        matches!(self, Self::ActiveFocus | Self::HallPass | Self::Break)
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "Idle",
            Self::ActiveFocus => "ActiveFocus",
            Self::HallPass => "HallPass",
            Self::Break => "Break",
            Self::Completed => "Completed",
            Self::Aborted => "Aborted",
        }
    }
}

pub struct Timing {
    pub phase: Phase,
    pub passes_remaining: u8,
    pub passes_used: u8,
    focus_length: Duration,
    break_length: Duration,
    focus_so_far: Duration,
    phase_started: Instant,
    deadline: Instant,
    after_pass: Duration,
}

impl Default for Timing {
    fn default() -> Self {
        let now = Instant::now();
        let mut idle = Self::new(25, 5, 3, now);
        idle.set(Phase::Idle, Duration::ZERO, now);
        idle
    }
}

impl Timing {
    pub fn new(focus_minutes: u32, break_minutes: u32, passes: u8, now: Instant) -> Self {
        let focus_length = Duration::from_secs(u64::from(focus_minutes) * 60);
        Self {
            phase: Phase::ActiveFocus,
            passes_remaining: passes,
            passes_used: 0,
            focus_length,
            break_length: Duration::from_secs(u64::from(break_minutes) * 60),
            focus_so_far: Duration::ZERO,
            phase_started: now,
            deadline: now + focus_length,
            after_pass: Duration::ZERO,
        }
    }

    fn set(&mut self, next: Phase, duration: Duration, now: Instant) {
        self.focus_so_far = self.actual_focus(now);
        self.phase = next;
        self.phase_started = now;
        self.deadline = now + duration;
    }

    pub fn actual_focus(&self, now: Instant) -> Duration {
        if self.phase == Phase::ActiveFocus {
            self.focus_so_far
                + now
                    .min(self.deadline)
                    .saturating_duration_since(self.phase_started)
        } else {
            self.focus_so_far
        }
    }

    pub fn seconds_left(&self, now: Instant) -> u64 {
        let remaining = self.deadline.saturating_duration_since(now);
        remaining.as_secs() + u64::from(remaining.subsec_nanos() != 0)
    }

    pub fn use_pass(&mut self, now: Instant) -> bool {
        if self.phase != Phase::ActiveFocus || self.passes_remaining == 0 || now >= self.deadline {
            return false;
        }
        self.passes_remaining -= 1;
        self.passes_used += 1;
        self.after_pass = self.deadline.saturating_duration_since(now);
        self.set(Phase::HallPass, HALL_PASS, now);
        true
    }

    pub fn resume(&mut self, now: Instant) {
        let duration = if self.phase == Phase::HallPass {
            self.after_pass
        } else {
            self.focus_length
        };
        self.set(Phase::ActiveFocus, duration, now);
    }

    pub fn tick(&mut self, now: Instant) {
        if now < self.deadline {
            return;
        }
        match self.phase {
            Phase::ActiveFocus => self.set(Phase::Break, self.break_length, now),
            Phase::HallPass | Phase::Break => self.resume(now),
            _ => {}
        }
    }

    pub fn stop(&mut self, phase: Phase, now: Instant) {
        self.set(phase, Duration::ZERO, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hall_pass_pauses_focus_then_resumes_and_enters_break() {
        let now = Instant::now();
        let t = |secs| now + Duration::from_secs(secs);
        let mut timing = Timing::new(1, 1, 2, now);
        assert!(timing.use_pass(t(10)));
        assert_eq!(timing.phase, Phase::HallPass);
        assert_eq!(timing.seconds_left(t(11)), 59);
        assert_eq!(timing.actual_focus(t(47)).as_secs(), 10);
        timing.tick(t(70));
        assert_eq!(timing.phase, Phase::ActiveFocus);
        assert_eq!(timing.seconds_left(t(70)), 50);
        assert!(timing.use_pass(t(75)));
        timing.resume(t(80));
        assert!(!timing.use_pass(t(81)));
        assert_eq!(timing.passes_remaining, 0);
        assert_eq!(timing.passes_used, 2);
        timing.tick(t(125));
        assert_eq!(timing.phase, Phase::Break);
        assert_eq!(timing.actual_focus(t(133)).as_secs(), 60);
        timing.tick(t(185));
        assert_eq!(timing.phase, Phase::ActiveFocus);
        assert_eq!(timing.seconds_left(t(185)), 60);
    }

    #[test]
    fn late_ticks_cap_focus_time_and_finish_keeps_totals() {
        let now = Instant::now();
        let mut timing = Timing::new(1, 1, 0, now);
        let late = now + Duration::from_secs(200);
        timing.tick(late);
        timing.stop(Phase::Completed, late);
        assert_eq!(timing.actual_focus(late).as_secs(), 60);
        assert!(!timing.phase.is_running());
        assert_eq!(timing.seconds_left(late), 0);
        assert_eq!(timing.phase.name(), "Completed");
    }
}
