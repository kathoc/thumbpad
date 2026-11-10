//! A stopping touch freezes pointer output for a fixed interval, then rebases.
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct StopHold {
    until: Option<Instant>,
}
impl StopHold {
    pub fn reset(&mut self) {
        self.until = None;
    }
    pub fn suppress(
        &mut self,
        now: Instant,
        trackball: bool,
        stopped_by_touch: bool,
        duration: Duration,
    ) -> bool {
        if !trackball || duration.is_zero() {
            self.reset();
            return false;
        }
        if stopped_by_touch {
            self.until = Some(now + duration);
        }
        match self.until {
            Some(until) if now < until => true,
            Some(_) => {
                // Discard the first sample after expiry as a fresh baseline,
                // rather than replaying displacement collected during the hold.
                self.until = None;
                true
            }
            None => false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn freezes_for_300ms_then_rebases_and_resumes_without_lifting() {
        let t = Instant::now();
        let hold = Duration::from_millis(300);
        let mut brake = StopHold::default();
        assert!(!brake.suppress(t, true, false, hold));
        assert!(brake.suppress(t, true, true, hold));
        assert!(brake.suppress(t + Duration::from_millis(100), true, false, hold));
        assert!(brake.suppress(t + Duration::from_millis(299), true, false, hold));
        assert!(brake.suppress(t + Duration::from_millis(300), true, false, hold));
        assert!(!brake.suppress(t + Duration::from_millis(308), true, false, hold));
    }
    #[test]
    fn normal_contacts_do_not_freeze_and_mouse_mode_clears_the_hold() {
        let t = Instant::now();
        let duration = Duration::from_millis(300);
        let mut brake = StopHold::default();
        assert!(!brake.suppress(t, true, false, duration));
        assert!(brake.suppress(t, true, true, duration));
        assert!(!brake.suppress(t, false, false, duration));
        assert!(!brake.suppress(t, true, false, duration));
        assert!(!brake.suppress(t, true, true, Duration::ZERO));
    }
    #[test]
    fn later_movement_does_not_extend_deadline_and_long_gaps_do_not_jump() {
        let t = Instant::now();
        let duration = Duration::from_millis(300);
        let mut brake = StopHold::default();
        brake.suppress(t, true, true, duration);
        for i in 1..30 {
            assert!(brake.suppress(t + Duration::from_millis(i * 10), true, false, duration));
        }
        assert!(brake.suppress(t + Duration::from_secs(2), true, false, duration));
        assert!(!brake.suppress(t + Duration::from_millis(2008), true, false, duration));
    }
}
