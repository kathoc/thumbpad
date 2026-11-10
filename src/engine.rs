//! Key ownership is fixed at the first physical down, including across repeats.
#[derive(Default)]
pub struct Engine {
    down: [bool; 5],
    captured: [bool; 5],
    held: [bool; 5],
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
    fn button(i: usize) -> Option<Button> {
        match i {
            0 | 1 => Some(Button::Left),
            2 | 3 => Some(Button::Right),
            _ => None,
        }
    }
    fn count(&self, b: Button) -> usize {
        self.held
            .iter()
            .enumerate()
            .filter(|(i, held)| **held && Self::button(*i) == Some(b))
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
            let first = button.is_some_and(|b| self.count(b) == 0);
            self.held[i] = true;
            (
                true,
                button
                    .filter(|_| first)
                    .map(|button| Action { button, down: true }),
            )
        } else {
            let consumed = self.captured[i];
            let held = self.held[i];
            self.down[i] = false;
            self.captured[i] = false;
            self.held[i] = false;
            (
                consumed,
                button
                    .filter(|b| held && self.count(*b) == 0)
                    .map(|button| Action {
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
        self.held = [false; 5];
        actions
    }
    pub fn reset(&mut self) -> Vec<Action> {
        let actions = self.release();
        *self = Self::default();
        actions
    }
    pub fn dragging(&self) -> bool {
        self.held[..4].iter().any(|held| *held)
    }
    pub fn scrolling(&self) -> bool {
        self.held[4]
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn d_and_k_share_right_button_ownership() {
        let mut e = Engine::default();
        assert_eq!(
            e.key(3, true, true).1,
            Some(Action {
                button: Button::Right,
                down: true
            })
        );
        assert_eq!(e.key(2, true, true), (true, None));
        assert_eq!(e.key(3, false, false), (true, None));
        assert_eq!(
            e.key(2, false, false).1,
            Some(Action {
                button: Button::Right,
                down: false
            })
        );
        assert!(!e.dragging());
    }
    #[test]
    fn g_owns_scroll_without_mouse_buttons_and_pause_clears_it() {
        let mut e = Engine::default();
        assert_eq!(e.key(4, true, true), (true, None));
        assert!(e.scrolling());
        assert!(!e.dragging());
        assert_eq!(e.key(4, true, false), (true, None));
        assert!(e.release().is_empty());
        assert!(!e.scrolling());
        assert_eq!(e.key(4, false, false), (true, None));
        assert_eq!(e.key(4, true, false), (false, None));
        assert!(!e.scrolling());
    }
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
