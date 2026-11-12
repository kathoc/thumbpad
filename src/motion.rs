//! Converts complete HID frames into drag/wheel deltas; a lift is a new baseline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Drag,
    Pointer,
    Scroll,
    HorizontalScroll,
}
#[derive(Clone, Copy)]
pub struct Tuning {
    pub pointer_deadband: f64,
    pub drag_x: f64,
    pub drag_y: f64,
    pub wheel: f64,
    pub invert: bool,
    pub wheel_x: f64,
    pub horizontal_invert: bool,
}
#[derive(Debug, PartialEq, Eq)]
pub enum Effect {
    Move { dx: i32, dy: i32 },
    Wheel(i32),
    HorizontalWheel(i32),
}
#[derive(Default)]
pub struct Motion {
    mode: Mode,
    epoch: u64,
    previous: Option<(u32, f64, f64)>,
    remainder: [f64; 2],
}
impl Motion {
    pub fn feed(
        &mut self,
        mode: Mode,
        epoch: u64,
        point: Option<(u32, f64, f64)>,
        tuning: Tuning,
    ) -> Option<Effect> {
        let old = self.previous;
        let changed = self.mode != mode || self.epoch != epoch;
        self.mode = mode;
        self.epoch = epoch;
        if mode == Mode::Off
            || changed
            || old.is_none()
            || point.is_none()
            || old.unwrap().0 != point.unwrap().0
        {
            self.previous = point;
            self.remainder = [0.0; 2];
            return None;
        }
        let (_, x, y) = point.unwrap();
        let (_, ox, oy) = old.unwrap();
        self.previous = point;
        match mode {
            Mode::Off => None,
            Mode::Drag | Mode::Pointer => {
                let dx = (x - ox) * tuning.drag_x;
                let dy = (y - oy) * tuning.drag_y;
                let distance = dx.hypot(dy);
                if distance <= tuning.pointer_deadband {
                    self.previous = old;
                    return None;
                }
                let scale = 1.0 - tuning.pointer_deadband / distance;
                if tuning.pointer_deadband > 0.0 {
                    // Keep a spatial margin without buffering reports or averaging movement.
                    self.previous = Some((
                        point.unwrap().0,
                        ox + (x - ox) * scale,
                        oy + (y - oy) * scale,
                    ));
                }
                self.remainder[0] += dx * scale;
                self.remainder[1] += dy * scale;
                let dx = self.remainder[0].round() as i32;
                let dy = self.remainder[1].round() as i32;
                self.remainder[0] -= dx as f64;
                self.remainder[1] -= dy as f64;
                (dx != 0 || dy != 0).then_some(Effect::Move { dx, dy })
            }
            Mode::Scroll => {
                self.remainder[1] +=
                    (y - oy) * tuning.wheel * if tuning.invert { 1.0 } else { -1.0 };
                let delta = self.remainder[1].round() as i32;
                self.remainder[1] -= delta as f64;
                (delta != 0).then_some(Effect::Wheel(delta))
            }
            Mode::HorizontalScroll => {
                self.remainder[0] +=
                    (x - ox) * tuning.wheel_x * if tuning.horizontal_invert { -1.0 } else { 1.0 };
                let delta = self.remainder[0].round() as i32;
                self.remainder[0] -= delta as f64;
                (delta != 0).then_some(Effect::HorizontalWheel(delta))
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trackball_jitter_does_not_accumulate_and_motion_responds_in_the_same_report() {
        let mut motion = Motion::default();
        let tuning = Tuning {
            pointer_deadband: 2.0,
            drag_x: 1000.0,
            drag_y: 1000.0,
            ..t()
        };
        let mut feed = |x| motion.feed(Mode::Pointer, 0, Some((1, x, 0.5)), tuning);
        assert_eq!(feed(0.5), None);
        for _ in 0..20 {
            assert_eq!(feed(0.501), None);
            assert_eq!(feed(0.499), None);
        }
        assert_eq!(feed(0.510), Some(Effect::Move { dx: 8, dy: 0 }));
        assert_eq!(feed(0.510), None);
    }
    #[test]
    fn discarded_input_rebases_without_replaying_the_missing_displacement() {
        let mut m = Motion::default();
        m.feed(Mode::Pointer, 0, Some((1, 0.1, 0.5)), t());
        m = Motion::default(); // Recovery discards the old coordinate baseline.
        assert_eq!(m.feed(Mode::Pointer, 0, Some((1, 0.9, 0.5)), t()), None);
        assert_eq!(
            m.feed(Mode::Pointer, 0, Some((1, 0.91, 0.5)), t()),
            Some(Effect::Move { dx: 10, dy: 0 })
        );
    }
    #[test]
    fn trackball_pointer_moves_without_a_click_key_and_rebases_on_lift() {
        let mut m = Motion::default();
        assert_eq!(m.feed(Mode::Pointer, 1, Some((1, 0.5, 0.5)), t()), None);
        assert_eq!(
            m.feed(Mode::Pointer, 1, Some((1, 0.6, 0.4)), t()),
            Some(Effect::Move { dx: 100, dy: -100 })
        );
        m.feed(Mode::Pointer, 1, None, t());
        assert_eq!(m.feed(Mode::Pointer, 1, Some((2, 0.1, 0.9)), t()), None);
        assert_eq!(m.feed(Mode::Off, 2, Some((2, 0.2, 0.8)), t()), None);
    }
    #[test]
    fn horizontal_scroll_rightward_drag_scrolls_left_and_can_be_reversed() {
        let mut m = Motion::default();
        m.feed(Mode::HorizontalScroll, 1, Some((1, 0.5, 0.5)), t());
        assert_eq!(
            m.feed(Mode::HorizontalScroll, 1, Some((1, 0.55, 0.9)), t()),
            Some(Effect::HorizontalWheel(-120))
        );
        let mut reverse = t();
        reverse.horizontal_invert = false;
        assert_eq!(
            m.feed(Mode::HorizontalScroll, 1, Some((1, 0.6, 0.1)), reverse),
            Some(Effect::HorizontalWheel(120))
        );
        assert_eq!(m.feed(Mode::Scroll, 2, Some((1, 0.1, 0.5)), t()), None);
        assert_eq!(
            m.feed(Mode::HorizontalScroll, 3, Some((1, 0.9, 0.1)), t()),
            None
        );
    }
    fn t() -> Tuning {
        Tuning {
            pointer_deadband: 0.0,
            drag_x: 1000.0,
            drag_y: 1000.0,
            wheel: 2400.0,
            invert: false,
            wheel_x: 2400.0,
            horizontal_invert: true,
        }
    }
    #[test]
    fn repeated_clutches_rebase_without_jumps() {
        let mut m = Motion::default();
        for id in 0..3 {
            assert_eq!(m.feed(Mode::Drag, 1, Some((id, 0.2, 0.5)), t()), None);
            assert_eq!(
                m.feed(Mode::Drag, 1, Some((id, 0.3, 0.5)), t()),
                Some(Effect::Move { dx: 100, dy: 0 })
            );
            assert_eq!(m.feed(Mode::Drag, 1, None, t()), None);
        }
    }
    #[test]
    fn scroll_is_vertical_signed_and_resets_on_recontact() {
        let mut m = Motion::default();
        m.feed(Mode::Scroll, 1, Some((1, 0.5, 0.5)), t());
        assert_eq!(
            m.feed(Mode::Scroll, 1, Some((1, 0.7, 0.55)), t()),
            Some(Effect::Wheel(-120))
        );
        assert_eq!(
            m.feed(Mode::Scroll, 1, Some((1, 0.7, 0.5)), t()),
            Some(Effect::Wheel(120))
        );
        m.feed(Mode::Scroll, 1, None, t());
        assert_eq!(m.feed(Mode::Scroll, 1, Some((2, 0.1, 0.1)), t()), None);
    }
    #[test]
    fn key_release_new_gesture_and_contact_switch_do_not_jump() {
        let mut m = Motion::default();
        m.feed(Mode::Drag, 1, Some((1, 0.5, 0.5)), t());
        assert_eq!(m.feed(Mode::Drag, 2, Some((1, 0.9, 0.9)), t()), None);
        assert_eq!(m.feed(Mode::Scroll, 3, Some((1, 0.1, 0.1)), t()), None);
        assert_eq!(m.feed(Mode::Scroll, 3, Some((2, 0.9, 0.9)), t()), None);
        assert_eq!(m.feed(Mode::Off, 4, Some((2, 0.8, 0.8)), t()), None);
    }
    #[test]
    fn fractional_deltas_accumulate_and_inversion_works() {
        let mut m = Motion::default();
        m.feed(Mode::Drag, 1, Some((1, 0.0, 0.0)), t());
        assert_eq!(m.feed(Mode::Drag, 1, Some((1, 0.0002, 0.0)), t()), None);
        assert_eq!(m.feed(Mode::Drag, 1, Some((1, 0.0004, 0.0)), t()), None);
        assert_eq!(
            m.feed(Mode::Drag, 1, Some((1, 0.0006, 0.0)), t()),
            Some(Effect::Move { dx: 1, dy: 0 })
        );
        let mut s = Motion::default();
        let mut tuning = t();
        tuning.invert = true;
        s.feed(Mode::Scroll, 1, Some((1, 0.5, 0.5)), tuning);
        assert_eq!(
            s.feed(Mode::Scroll, 1, Some((1, 0.5, 0.55)), tuning),
            Some(Effect::Wheel(120))
        );
    }
}
