//! Pointer delivery stays off the UI thread. A high-resolution timer runs only
//! during a glide; cancellation generations reject stale queued movement.
#![allow(unsafe_op_in_unsafe_fn)]
use crate::inertia::PointerInertia;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::Instant,
};
use windows::{
    Win32::{
        Foundation::*,
        System::Threading::*,
        UI::{Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::*,
};
enum Command {
    Move(u64, usize, Instant, i32, i32),
    Start(u64, usize, Instant, Box<PointerInertia>, f64),
    Stop(u64),
    Shutdown,
}
#[derive(Clone)]
pub struct Client {
    tx: mpsc::SyncSender<Command>,
    generation: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
    ticks: Arc<AtomicU64>,
    max_gap_us: Arc<AtomicU64>,
    brake_gain: Arc<AtomicU64>,
}
impl Client {
    pub fn set_brake_gain(&self, gain: f64) {
        self.brake_gain
            .store(gain.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }
    pub fn active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    pub fn stop(&self) {
        let generation = self
            .generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        self.active.store(false, Ordering::Release);
        let _ = self.tx.try_send(Command::Stop(generation));
    }
    pub fn move_by(&self, dx: i32, dy: i32) {
        let _ = self.tx.try_send(Command::Move(
            self.generation.load(Ordering::Acquire),
            unsafe { GetForegroundWindow().0 as usize },
            Instant::now(),
            dx,
            dy,
        ));
    }
    pub fn start(&self, motion: PointerInertia, decay: f64) {
        self.active.store(true, Ordering::Release);
        self.ticks.store(0, Ordering::Relaxed);
        self.max_gap_us.store(0, Ordering::Relaxed);
        if self
            .tx
            .try_send(Command::Start(
                self.generation.load(Ordering::Acquire),
                unsafe { GetForegroundWindow().0 as usize },
                Instant::now(),
                Box::new(motion),
                decay,
            ))
            .is_err()
        {
            self.active.store(false, Ordering::Release);
        }
    }
    pub fn shutdown(&self) {
        self.stop();
        let _ = self.tx.send(Command::Shutdown);
    }
    pub fn stats(&self) -> (u64, u64) {
        (
            self.ticks.load(Ordering::Relaxed),
            self.max_gap_us.load(Ordering::Relaxed),
        )
    }
}
fn relative_input(dx: f64, dy: f64, remainder: &mut (f64, f64)) -> Option<INPUT> {
    let x = remainder.0 + dx;
    let y = remainder.1 + dy;
    let dx = x.round() as i32;
    let dy = y.round() as i32;
    *remainder = (x - dx as f64, y - dy as f64);
    (dx != 0 || dy != 0).then_some(INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                dwFlags: MOUSEEVENTF_MOVE,
                dwExtraInfo: 0x54485044,
                ..Default::default()
            },
        },
    })
}
unsafe fn deliver(remainder: &mut (f64, f64), dx: f64, dy: f64, foreground: HWND) -> bool {
    if unsafe { GetForegroundWindow() } != foreground {
        return true;
    }
    match relative_input(dx, dy, remainder) {
        Some(input) => unsafe {
            let sent = SendInput(&[input], std::mem::size_of::<INPUT>() as i32) == 1;
            if sent {
                crate::LAST_POINTER_DELIVERY_MS.store(
                    windows::Win32::System::SystemInformation::GetTickCount64(),
                    Ordering::Relaxed,
                );
            }
            sent
        },
        None => true,
    }
}
struct Run {
    generation: u64,
    motion: PointerInertia,
    decay: f64,
    last: Instant,
}
pub fn spawn() -> Result<(Client, JoinHandle<()>)> {
    let (tx, rx) = mpsc::sync_channel(crate::recovery::POINTER_QUEUE_CAPACITY);
    let (ready_tx, ready_rx) = mpsc::sync_channel::<std::result::Result<(), String>>(1);
    let client = Client {
        tx,
        generation: Arc::new(AtomicU64::new(0)),
        active: Arc::new(AtomicBool::new(false)),
        ticks: Arc::new(AtomicU64::new(0)),
        max_gap_us: Arc::new(AtomicU64::new(0)),
        brake_gain: Arc::new(AtomicU64::new(1.0f64.to_bits())),
    };
    let shared = client.clone();
    let worker = std::thread::spawn(move || unsafe {
        let timer = match CreateWaitableTimerExW(
            None,
            None,
            CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
            TIMER_ALL_ACCESS.0,
        ) {
            Ok(timer) => timer,
            Err(error) => {
                let _ = ready_tx.send(Err(error.to_string()));
                return;
            }
        };
        if ready_tx.send(Ok(())).is_err() {
            let _ = CloseHandle(timer);
            return;
        }
        let mut run: Option<Run> = None;
        let mut foreground = HWND::default();
        let mut remainder = (0.0, 0.0);
        let mut delivered_generation = 0;
        loop {
            let command = if run.is_none() {
                match rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                }
            } else {
                rx.try_recv().ok()
            };
            let latest_generation = shared.generation.load(Ordering::Acquire);
            if delivered_generation != latest_generation {
                delivered_generation = latest_generation;
                run = None;
                remainder = (0.0, 0.0);
                let _ = CancelWaitableTimer(timer);
            }
            let current_foreground = GetForegroundWindow();
            let changed = foreground != current_foreground;
            if changed {
                foreground = current_foreground;
                run = None;
                remainder = (0.0, 0.0);
                shared.active.store(false, Ordering::Release);
                let _ = CancelWaitableTimer(timer);
            }
            if let Some(command) = command {
                let generation = match &command {
                    Command::Move(g, ..) | Command::Start(g, ..) | Command::Stop(g) => *g,
                    Command::Shutdown => break,
                };
                if generation != shared.generation.load(Ordering::Acquire) {
                    continue;
                }
                if let Command::Move(_, _, queued_at, ..) | Command::Start(_, _, queued_at, ..) =
                    &command
                    && !crate::recovery::pointer_fresh(queued_at.elapsed())
                {
                    if matches!(command, Command::Start(..)) {
                        shared.active.store(false, Ordering::Release);
                    }
                    continue;
                }
                if let Command::Move(_, window, ..) | Command::Start(_, window, ..) = &command
                    && *window != foreground.0 as usize
                {
                    if matches!(command, Command::Start(..)) {
                        shared.active.store(false, Ordering::Release);
                    }
                    continue;
                }
                match command {
                    Command::Stop(_) => {
                        run = None;
                        remainder = (0.0, 0.0);
                        shared.active.store(false, Ordering::Release);
                        let _ = CancelWaitableTimer(timer);
                    }
                    Command::Move(_, _, _, dx, dy) => {
                        if generation == shared.generation.load(Ordering::Acquire)
                            && !deliver(&mut remainder, dx as f64, dy as f64, foreground)
                        {
                            crate::ERRORS.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Command::Start(_, _, _, motion, decay) => {
                        if changed {
                            continue;
                        }
                        shared.active.store(true, Ordering::Release);
                        let due = -20_000i64; // 2ms, expressed in 100ns units. No process-wide timer-resolution change.
                        if SetWaitableTimer(timer, &due, 2, None, None, false).is_err() {
                            shared.active.store(false, Ordering::Release);
                            crate::ERRORS.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                        run = Some(Run {
                            generation,
                            motion: *motion,
                            decay,
                            last: Instant::now(),
                        });
                    }
                    Command::Shutdown => unreachable!(),
                }
                continue;
            }
            if run.is_none() {
                continue;
            }
            if WaitForSingleObject(timer, 250) != WAIT_OBJECT_0 {
                run = None;
                shared.active.store(false, Ordering::Release);
                let _ = CancelWaitableTimer(timer);
                continue;
            }
            let Some(current) = run.as_mut() else {
                continue;
            };
            if current.generation != shared.generation.load(Ordering::Acquire) {
                run = None;
                let _ = CancelWaitableTimer(timer);
                continue;
            }
            let gain = f64::from_bits(shared.brake_gain.load(Ordering::Relaxed));
            let now = Instant::now();
            shared.ticks.fetch_add(1, Ordering::Relaxed);
            shared.max_gap_us.fetch_max(
                now.saturating_duration_since(current.last).as_micros() as u64,
                Ordering::Relaxed,
            );
            current.last = now;
            if let Some((dx, dy)) = current.motion.tick_braked(now, current.decay, gain)
                && current.generation == shared.generation.load(Ordering::Acquire)
                && !deliver(&mut remainder, dx, dy, foreground)
            {
                crate::ERRORS.fetch_add(1, Ordering::Relaxed);
            }
            if !current.motion.active() {
                run = None;
                shared.active.store(false, Ordering::Release);
                let _ = CancelWaitableTimer(timer);
            }
        }
        shared.active.store(false, Ordering::Release);
        let _ = CancelWaitableTimer(timer);
        let _ = CloseHandle(timer);
    });
    match ready_rx.recv() {
        Ok(Ok(())) => Ok((client, worker)),
        other => {
            let _ = worker.join();
            Err(Error::new(
                E_FAIL,
                format!("Pointer timer initialization failed: {other:?}"),
            ))
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_mailbox_never_blocks_input_and_stop_invalidates_full_queue() {
        let (tx, rx) = mpsc::sync_channel(crate::recovery::POINTER_QUEUE_CAPACITY);
        let client = Client {
            tx,
            generation: Arc::new(AtomicU64::new(0)),
            active: Arc::new(AtomicBool::new(true)),
            ticks: Arc::new(AtomicU64::new(0)),
            max_gap_us: Arc::new(AtomicU64::new(0)),
            brake_gain: Arc::new(AtomicU64::new(1.0f64.to_bits())),
        };
        for _ in 0..crate::recovery::POINTER_QUEUE_CAPACITY {
            client.move_by(1, 1);
        }
        client.move_by(1, 1); // Must return even with no worker receiving.
        client.stop(); // Stop's mailbox packet cannot fit; generation is authoritative.
        assert!(!client.active());
        let generation = client.generation.load(Ordering::Acquire);
        let packets: Vec<_> = rx.try_iter().collect();
        assert_eq!(packets.len(), crate::recovery::POINTER_QUEUE_CAPACITY);
        assert!(
            packets
                .into_iter()
                .all(|c| matches!(c, Command::Move(g, ..) if g != generation))
        );
    }
    #[test]
    fn relative_output_keeps_deltas_and_fractional_motion_without_desktop_clamping() {
        let mut remainder = (0.0, 0.0);
        let input = relative_input(2400.0, -1800.0, &mut remainder).unwrap();
        let mouse = unsafe { input.Anonymous.mi };
        assert_eq!((mouse.dx, mouse.dy), (2400, -1800));
        assert_eq!(mouse.dwFlags, MOUSEEVENTF_MOVE);
        assert!(!mouse.dwFlags.contains(MOUSEEVENTF_ABSOLUTE));
        let mut total = (0, 0);
        for _ in 0..100 {
            if let Some(input) = relative_input(0.1, -0.02, &mut remainder) {
                let mouse = unsafe { input.Anonymous.mi };
                total.0 += mouse.dx;
                total.1 += mouse.dy;
            }
        }
        assert_eq!(total, (10, -2));
    }
    #[test]
    fn packetized_relative_coast_preserves_distance_and_diagonal_at_different_tick_rates() {
        let t = Instant::now();
        let coast = |ticks: &[u64]| {
            let mut motion = PointerInertia::default();
            for i in 0..6 {
                motion.record(24.0, -0.48, t + std::time::Duration::from_millis(i * 8));
            }
            assert!(motion.release(t + std::time::Duration::from_millis(40), 12.0, 1.0));
            let mut remainder = (0.0, 0.0);
            let mut distance = (0, 0);
            for tick in ticks {
                if let Some((dx, dy)) =
                    motion.tick_braked(t + std::time::Duration::from_millis(40 + tick), 5.0, 1.0)
                    && let Some(input) = relative_input(dx, dy, &mut remainder)
                {
                    let mouse = unsafe { input.Anonymous.mi };
                    distance.0 += mouse.dx;
                    distance.1 += mouse.dy;
                }
            }
            distance
        };
        let fine: Vec<_> = (1..=50).map(|i| i * 2).collect();
        let a = coast(&fine);
        let b = coast(&[16, 32, 48, 64, 80, 96, 100]);
        assert_eq!(a, b);
        assert!(a.0 > 0 && a.1 < 0);
    }
    #[test]
    #[ignore = "native timer timing probe; no pointer input is generated"]
    fn native_timer_probe() {
        unsafe {
            let timer = CreateWaitableTimerExW(
                None,
                None,
                CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                TIMER_ALL_ACCESS.0,
            )
            .unwrap();
            SetWaitableTimer(timer, &-20_000, 2, None, None, false).unwrap();
            let mut last = Instant::now();
            let mut gaps = Vec::new();
            for _ in 0..300 {
                assert_eq!(WaitForSingleObject(timer, 250), WAIT_OBJECT_0);
                let now = Instant::now();
                gaps.push(now.duration_since(last).as_micros());
                last = now;
            }
            let _ = CancelWaitableTimer(timer);
            let _ = CloseHandle(timer);
            gaps.sort_unstable();
            println!(
                "2ms high-resolution timer: median={}us p95={}us max={}us",
                gaps[150], gaps[285], gaps[299]
            );
        }
    }
}
