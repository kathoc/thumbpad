//! Time-based wheel momentum; touching the pad cancels both queued and running motion.
use crate::motion::{Effect, Mode};
use std::time::{Duration, Instant};
#[derive(Default)]
pub struct Inertia {
    axis: Option<Mode>,
    velocity: f64,
    sample_at: Option<Instant>,
    motion_at: Option<Instant>,
    tick_at: Option<Instant>,
    started_at: Option<Instant>,
    remainder: f64,
}
impl Inertia {
    pub fn active(&self) -> bool {
        self.tick_at.is_some()
    }
    pub fn stop(&mut self) {
        *self = Self::default();
    }
    pub fn record(&mut self, axis: Option<Mode>, delta: f64, now: Instant) {
        let previous = self.sample_at;
        if self.axis != axis {
            self.velocity = 0.0;
            self.motion_at = None;
        }
        let changed = self.axis != axis;
        self.axis = axis;
        self.sample_at = Some(now);
        if axis.is_none() {
            self.velocity = 0.0;
            self.motion_at = None;
            return;
        }
        if delta != 0.0 {
            self.motion_at = Some(now);
        }
        if changed || previous.is_none() {
            return;
        }
        let dt = now
            .saturating_duration_since(previous.unwrap())
            .as_secs_f64();
        if dt > 0.15 {
            self.velocity = 0.0;
            return;
        }
        if dt < 0.001 {
            return;
        }
        let instantaneous = (delta / dt).clamp(-24000.0, 24000.0);
        let alpha = 1.0 - (-dt / 0.03).exp();
        self.velocity = if self.velocity * instantaneous < 0.0 {
            instantaneous
        } else {
            self.velocity + alpha * (instantaneous - self.velocity)
        };
    }
    pub fn release(&mut self, now: Instant) -> bool {
        if self.axis.is_none()
            || self.velocity.abs() < 120.0
            || self
                .motion_at
                .is_none_or(|t| now.saturating_duration_since(t) > Duration::from_millis(80))
        {
            self.stop();
            return false;
        }
        self.tick_at = Some(now);
        self.started_at = Some(now);
        self.remainder = 0.0;
        true
    }
    pub fn tick(&mut self, now: Instant, decay: f64) -> Option<Effect> {
        let last = self.tick_at?;
        let dt = now.saturating_duration_since(last).as_secs_f64();
        if dt > 0.25
            || self
                .started_at
                .is_none_or(|t| now.saturating_duration_since(t) > Duration::from_secs(3))
        {
            self.stop();
            return None;
        }
        self.tick_at = Some(now);
        let damping = (-decay * dt).exp();
        self.remainder += self.velocity * (1.0 - damping) / decay;
        self.velocity *= damping;
        let delta = self.remainder.round() as i32;
        self.remainder -= delta as f64;
        let axis = self.axis;
        if self.velocity.abs() < 12.0 {
            self.stop();
        }
        if delta == 0 {
            None
        } else {
            match axis {
                Some(Mode::Scroll) => Some(Effect::Wheel(delta)),
                Some(Mode::HorizontalScroll) => Some(Effect::HorizontalWheel(delta)),
                _ => None,
            }
        }
    }
}
#[derive(Default)]
pub struct PointerInertia {
    x: Inertia,
    y: Inertia,
}
impl PointerInertia {
    pub fn active(&self) -> bool {
        self.x.active() || self.y.active()
    }
    pub fn stop(&mut self) {
        self.x.stop();
        self.y.stop();
    }
    pub fn record(&mut self, dx: f64, dy: f64, now: Instant) {
        self.x.record(Some(Mode::HorizontalScroll), dx, now);
        self.y.record(Some(Mode::Scroll), dy, now);
    }
    pub fn release(&mut self, now: Instant) -> bool {
        let x = self.x.release(now);
        let y = self.y.release(now);
        x || y
    }
    pub fn tick(&mut self, now: Instant, decay: f64) -> Option<Effect> {
        let dx = match self.x.tick(now, decay) {
            Some(Effect::HorizontalWheel(d)) => d,
            _ => 0,
        };
        let dy = match self.y.tick(now, decay) {
            Some(Effect::Wheel(d)) => d,
            _ => 0,
        };
        (dx != 0 || dy != 0).then_some(Effect::Move { dx, dy })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pointer_trackball_preserves_2d_direction_and_stops_on_retouch() {
        let t = Instant::now();
        let mut pointer = PointerInertia::default();
        for i in 0..6 {
            pointer.record(24.0, -24.0, t + Duration::from_millis(i * 8));
        }
        assert!(pointer.release(t + Duration::from_millis(40)));
        let first = pointer.tick(t + Duration::from_millis(56), 5.0).unwrap();
        let next = pointer.tick(t + Duration::from_millis(72), 5.0).unwrap();
        assert!(matches!(first, Effect::Move { dx, dy } if dx > 0 && dy < 0));
        assert!(
            matches!((first, next), (Effect::Move { dx: a, .. }, Effect::Move { dx: b, .. }) if a > b)
        );
        pointer.stop();
        assert!(!pointer.active());
        assert_eq!(pointer.tick(t + Duration::from_millis(88), 5.0), None);
    }
    fn flick(axis: Mode, delta: f64, t: Instant) -> Inertia {
        let mut inertia = Inertia::default();
        for i in 0..6 {
            inertia.record(
                Some(axis),
                if i == 0 { 0.0 } else { delta },
                t + Duration::from_millis(i * 8),
            );
        }
        assert!(inertia.release(t + Duration::from_millis(40)));
        inertia
    }
    #[test]
    fn flick_decelerates_and_stops() {
        let t = Instant::now();
        let mut inertia = flick(Mode::Scroll, 24.0, t);
        let first = match inertia.tick(t + Duration::from_millis(56), 5.0).unwrap() {
            Effect::Wheel(d) => d,
            _ => panic!(),
        };
        let second = match inertia.tick(t + Duration::from_millis(72), 5.0).unwrap() {
            Effect::Wheel(d) => d,
            _ => panic!(),
        };
        assert!(first > second && second > 0);
        for i in 3..200 {
            inertia.tick(t + Duration::from_millis(40 + i * 16), 5.0);
        }
        assert!(!inertia.active());
    }
    #[test]
    fn any_retouch_cancels_and_stationary_or_stale_release_does_not_fling() {
        let t = Instant::now();
        let mut inertia = flick(Mode::Scroll, 24.0, t);
        inertia.stop();
        assert_eq!(inertia.tick(t + Duration::from_millis(56), 5.0), None);
        inertia.record(Some(Mode::Scroll), 0.0, t);
        assert!(!inertia.release(t));
        let mut stale = flick(Mode::Scroll, 24.0, t);
        stale.stop();
        for i in 0..6 {
            stale.record(Some(Mode::Scroll), 24.0, t + Duration::from_millis(i * 8));
        }
        assert!(!stale.release(t + Duration::from_millis(200)));
    }
    #[test]
    fn horizontal_momentum_preserves_direction_and_stalled_timer_does_not_burst() {
        let t = Instant::now();
        let mut inertia = flick(Mode::HorizontalScroll, -24.0, t);
        assert!(
            matches!(inertia.tick(t + Duration::from_millis(56), 5.0), Some(Effect::HorizontalWheel(d)) if d < 0)
        );
        assert_eq!(inertia.tick(t + Duration::from_secs(2), 5.0), None);
        assert!(!inertia.active());
    }
    #[test]
    fn changing_axis_resets_velocity() {
        let t = Instant::now();
        let mut inertia = Inertia::default();
        for i in 0..6 {
            inertia.record(Some(Mode::Scroll), 24.0, t + Duration::from_millis(i * 8));
        }
        assert!(inertia.velocity > 120.0);
        inertia.record(
            Some(Mode::HorizontalScroll),
            0.0,
            t + Duration::from_millis(48),
        );
        assert!(!inertia.release(t + Duration::from_millis(48)));
    }
}
