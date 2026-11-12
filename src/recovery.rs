//! Fail-open deadlines, not input delays. Fresh input is processed immediately.
pub const POINTER_QUEUE_CAPACITY: usize = 64;
pub fn pointer_fresh(age: std::time::Duration) -> bool {
    age <= std::time::Duration::from_millis(40)
}

pub fn contact_fresh(now: u64, last: u64) -> bool {
    last != 0 && now >= last && now - last <= 750
}

pub fn ui_gap(now: u64, last: u64) -> bool {
    last != 0 && now.saturating_sub(last) > 2000
}

pub fn raw_fresh(now: u32, message_time: u32) -> bool {
    now.wrapping_sub(message_time) <= 100
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_contact_cannot_hold_external_mouse_after_resume() {
        assert!(!contact_fresh(1000, 0));
        assert!(contact_fresh(1000, 1000));
        assert!(contact_fresh(1750, 1000));
        assert!(!contact_fresh(1751, 1000));
        assert!(!contact_fresh(3_600_000, 1000));
        assert!(!contact_fresh(900, 1000));
    }
    #[test]
    fn raw_deadline_handles_tick_wrap_and_rejects_old_sleep_input() {
        assert!(raw_fresh(1000, 1000));
        assert!(raw_fresh(1000, 900));
        assert!(!raw_fresh(1000, 899));
        assert!(raw_fresh(10, u32::MAX - 9));
        assert!(!raw_fresh(3_600_000, 1000));
    }
    #[test]
    fn expired_pointer_packets_are_discarded_without_a_wait() {
        use std::time::Duration;
        assert!(pointer_fresh(Duration::ZERO));
        assert!(pointer_fresh(Duration::from_millis(40)));
        assert!(!pointer_fresh(Duration::from_micros(40_001)));
        assert!(!pointer_fresh(Duration::from_secs(3600)));
    }
    #[test]
    fn heartbeat_only_recovers_after_a_real_gap() {
        assert!(!ui_gap(10_000, 0));
        assert!(!ui_gap(1500, 1000));
        assert!(!ui_gap(3000, 1000));
        assert!(ui_gap(3001, 1000));
    }
}
