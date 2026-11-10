//! Key ownership is fixed at the first physical down, including across repeats.
#[derive(Default)]
pub struct Engine {
    down: [bool; 3],
    captured: [bool; 3],
    held: [bool; 3],
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Button {
    Left,
    Right,
}
#[derive(Debug, PartialEq, Eq)]
pub struct Action {
    pub button: Button,
    pub down: bool,
}
impl Engine {
    fn button(i: usize) -> Button {
        if i == 2 { Button::Right } else { Button::Left }
    }
    fn count(&self, b: Button) -> usize {
        self.held
            .iter()
            .enumerate()
            .filter(|(i, held)| **held && Self::button(*i) == b)
            .count()
    }
    pub fn key(&mut self, i: usize, down: bool, active: bool) -> (bool, Option<Action>) {
        let button = Self::button(i);
        if down {
            if self.down[i] {
                return (self.captured[i], None);
            }
            self.down[i] = true;
            self.captured[i] = active;
            if !active {
                return (false, None);
            }
            let first = self.count(button) == 0;
            self.held[i] = true;
            (true, first.then_some(Action { button, down: true }))
        } else {
            let consumed = self.captured[i];
            let held = self.held[i];
            self.down[i] = false;
            self.captured[i] = false;
            self.held[i] = false;
            (
                consumed,
                (held && self.count(button) == 0).then_some(Action {
                    button,
                    down: false,
                }),
            )
        }
    }
    pub fn release(&mut self) -> Vec<Action> {
        let actions = [Button::Left, Button::Right]
            .into_iter()
            .filter(|b| self.count(*b) > 0)
            .map(|button| Action {
                button,
                down: false,
            })
            .collect();
        self.held = [false; 3];
        actions
    }
    pub fn reset(&mut self) -> Vec<Action> {
        let actions = self.release();
        *self = Self::default();
        actions
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drag_survives_contact_loss_and_repeat() {
        let mut e = Engine::default();
        assert_eq!(
            e.key(0, true, true),
            (
                true,
                Some(Action {
                    button: Button::Left,
                    down: true
                })
            )
        );
        assert_eq!(e.key(0, true, false), (true, None));
        assert_eq!(
            e.key(0, false, false),
            (
                true,
                Some(Action {
                    button: Button::Left,
                    down: false
                })
            )
        );
    }
    #[test]
    fn typing_cannot_turn_into_click_mid_key() {
        let mut e = Engine::default();
        assert_eq!(e.key(0, true, false), (false, None));
        assert_eq!(e.key(0, true, true), (false, None));
        assert_eq!(e.key(0, false, true), (false, None));
    }
    #[test]
    fn overlapping_left_keys_release_after_last_key() {
        let mut e = Engine::default();
        e.key(0, true, true);
        assert_eq!(e.key(1, true, true), (true, None));
        assert_eq!(e.key(0, false, true), (true, None));
        assert!(!e.key(1, false, true).1.unwrap().down);
    }
    #[test]
    fn pause_swallows_remaining_repeat_and_up() {
        let mut e = Engine::default();
        e.key(2, true, true);
        assert_eq!(
            e.release(),
            vec![Action {
                button: Button::Right,
                down: false
            }]
        );
        assert_eq!(e.key(2, true, false), (true, None));
        assert_eq!(e.key(2, false, false), (true, None));
        assert_eq!(e.key(2, true, false), (false, None));
    }
}
