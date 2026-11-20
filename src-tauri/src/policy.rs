use std::time::{Duration, Instant};

pub const UNLOCK_HOLD: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WindowId {
    pub hwnd: isize,
    pub pid: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Decision {
    Ignore,
    Allow(WindowId),
    Block,
}

pub fn evaluate(
    allowed: &[WindowId],
    foreground: WindowId,
    owners: &[WindowId],
    ignored: bool,
) -> Decision {
    if ignored {
        return Decision::Ignore;
    }
    std::iter::once(&foreground)
        .chain(owners)
        .find(|window| allowed.contains(window))
        .map_or(Decision::Block, |window| Decision::Allow(*window))
}

pub fn hold_complete(started: Option<Instant>, now: Instant) -> bool {
    started.is_some_and(|start| now.saturating_duration_since(start) >= UNLOCK_HOLD)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALLOWED: WindowId = WindowId { hwnd: 10, pid: 1 };
    const OTHER: WindowId = WindowId { hwnd: 20, pid: 2 };

    #[test]
    fn matches_window_and_pid_and_accepts_owned_dialogs() {
        assert_eq!(
            evaluate(&[ALLOWED], ALLOWED, &[], false),
            Decision::Allow(ALLOWED)
        );
        assert_eq!(
            evaluate(&[ALLOWED], OTHER, &[ALLOWED], false),
            Decision::Allow(ALLOWED)
        );
        assert_eq!(
            evaluate(&[ALLOWED], WindowId { hwnd: 10, pid: 2 }, &[], false),
            Decision::Block
        );
        assert_eq!(
            evaluate(&[ALLOWED], WindowId { hwnd: 30, pid: 1 }, &[], false),
            Decision::Block
        );
        assert_eq!(evaluate(&[ALLOWED], OTHER, &[], true), Decision::Ignore);
    }

    #[test]
    fn unlock_requires_a_continuous_ten_second_hold() {
        let now = Instant::now();
        assert!(!hold_complete(Some(now), now + Duration::from_millis(9999)));
        assert!(hold_complete(Some(now), now + UNLOCK_HOLD));
        assert!(!hold_complete(None, now + UNLOCK_HOLD));
    }
}
