//! Key ownership is fixed at the first physical down, including across repeats.
pub struct Engine {
    down: [bool; 256],
    captured: [bool; 256],
    held: [bool; 256],
    bindings: [Option<usize>; 256],
}
impl Default for Engine {
    fn default() -> Self {
        Self {
            down: [false; 256],
            captured: [false; 256],
            held: [false; 256],
            bindings: [None; 256],
        }
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Button {
    Left,
    Right,
    Middle,
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
            5 | 7 => Some(Button::Middle),
            _ => None,
        }
    }
    fn count(&self, b: Button) -> usize {
        self.held
            .iter()
            .enumerate()
            .filter(|(i, held)| **held && self.bindings[*i].and_then(Self::button) == Some(b))
            .count()
    }
    #[cfg(test)]
    pub fn key(&mut self, i: usize, down: bool, active: bool) -> (bool, Option<Action>) {
        self.key_physical(i, Some(i), down, active)
    }
    pub fn key_physical(
        &mut self,
        i: usize,
        binding: Option<usize>,
        down: bool,
        active: bool,
    ) -> (bool, Option<Action>) {
        if down && !self.down[i] {
            self.bindings[i] = binding;
        }
        let button = self.bindings[i].and_then(Self::button);
        let active = active && self.bindings[i].is_some();
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
        let actions = [Button::Left, Button::Right, Button::Middle]
            .into_iter()
            .filter(|b| self.count(*b) > 0)
            .map(|button| Action {
                button,
                down: false,
            })
            .collect();
        self.held = [false; 256];
        actions
    }
    pub fn reset(&mut self) -> Vec<Action> {
        let actions = self.release();
        *self = Self::default();
        actions
    }
    pub fn dragging(&self) -> bool {
        self.held
            .iter()
            .enumerate()
            .any(|(i, held)| *held && self.bindings[i].and_then(Self::button).is_some())
    }
    pub fn scrolling(&self) -> bool {
        self.held
            .iter()
            .enumerate()
            .any(|(i, held)| *held && matches!(self.bindings[i], Some(4 | 6)))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn layout_change_releases_old_buttons_and_keeps_physical_key_ownership() {
        let mut e = Engine::default();
        assert_eq!(
            e.key_physical(0x46, Some(0), true, true).1,
            Some(Action {
                button: Button::Left,
                down: true
            })
        );
        assert_eq!(
            e.release(),
            vec![Action {
                button: Button::Left,
                down: false
            }]
        );
        assert_eq!(
            e.key_physical(0x56, Some(0), true, true).1,
            Some(Action {
                button: Button::Left,
                down: true
            })
        );
        assert_eq!(e.key_physical(0x46, None, false, true), (true, None));
        assert!(e.dragging());
        assert_eq!(
            e.key_physical(0x56, Some(0), false, true).1,
            Some(Action {
                button: Button::Left,
                down: false
            })
        );
        assert_eq!(e.key_physical(0x53, None, true, true), (false, None));
        e.release();
        assert_eq!(e.key_physical(0x53, Some(4), true, true), (false, None));
        assert!(!e.scrolling());
        assert_eq!(e.key_physical(0x53, Some(4), false, true), (false, None));
        e.key_physical(0x53, Some(4), true, true);
        assert!(e.scrolling());
    }
    #[test]
    fn l_and_m_aliases_share_scroll_and_middle_ownership() {
        let mut e = Engine::default();
        e.key(4, true, true);
        assert_eq!(e.key(6, true, true), (true, None));
        e.key(4, false, true);
        assert!(e.scrolling());
        e.key(6, false, true);
        assert!(!e.scrolling());
        assert_eq!(
            e.key(7, true, true).1,
            Some(Action {
                button: Button::Middle,
                down: true
            })
        );
        assert_eq!(e.key(5, true, true), (true, None));
        assert_eq!(e.key(7, false, false), (true, None));
        assert_eq!(
            e.key(5, false, false).1,
            Some(Action {
                button: Button::Middle,
                down: false
            })
        );
    }
    #[test]
    fn c_holds_and_releases_middle_button_including_pause() {
        let mut e = Engine::default();
        assert_eq!(
            e.key(5, true, true).1,
            Some(Action {
                button: Button::Middle,
                down: true
            })
        );
        assert!(e.dragging());
        assert_eq!(e.key(5, true, false), (true, None));
        assert_eq!(
            e.release(),
            vec![Action {
                button: Button::Middle,
                down: false
            }]
        );
        assert_eq!(e.key(5, false, false), (true, None));
        assert!(!e.dragging());
    }
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
    fn s_owns_scroll_without_mouse_buttons_and_pause_clears_it() {
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
