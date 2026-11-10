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
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
    },
    core::*,
};
enum Command {
    Move(u64, i32, i32),
    Start(u64, PointerInertia, f64),
    Stop(u64),
    Shutdown,
}
#[derive(Clone)]
pub struct Client {
    tx: mpsc::Sender<Command>,
    generation: Arc<AtomicU64>,
    active: Arc<AtomicBool>,
    ticks: Arc<AtomicU64>,
    max_gap_us: Arc<AtomicU64>,
}
impl Client {
    pub fn active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }
    pub fn stop(&self) {
        let generation = self
            .generation
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1);
        self.active.store(false, Ordering::Release);
        let _ = self.tx.send(Command::Stop(generation));
    }
    pub fn move_by(&self, dx: i32, dy: i32) {
        let _ = self.tx.send(Command::Move(
            self.generation.load(Ordering::Acquire),
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
            .send(Command::Start(
                self.generation.load(Ordering::Acquire),
                motion,
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
#[derive(Clone, Copy)]
struct Desktop {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}
impl Desktop {
    unsafe fn current() -> Self {
        Self {
            left: GetSystemMetrics(SM_XVIRTUALSCREEN),
            top: GetSystemMetrics(SM_YVIRTUALSCREEN),
            width: GetSystemMetrics(SM_CXVIRTUALSCREEN).max(1),
            height: GetSystemMetrics(SM_CYVIRTUALSCREEN).max(1),
        }
    }
    fn clamp(&self, x: f64, y: f64) -> (f64, f64) {
        (
            x.clamp(self.left as f64, (self.left + self.width - 1) as f64),
            y.clamp(self.top as f64, (self.top + self.height - 1) as f64),
        )
    }
    fn normalized(&self, x: f64, y: f64) -> (i32, i32) {
        let (x, y) = self.clamp(x, y);
        let axis = |p: f64, origin: i32, size: i32| {
            (((p.round() - origin as f64 + 0.5) * 65536.0 / size as f64).floor() as i32)
                .clamp(0, 65535)
        };
        (
            axis(x, self.left, self.width),
            axis(y, self.top, self.height),
        )
    }
    unsafe fn send(&self, point: (f64, f64)) -> bool {
        let (dx, dy) = self.normalized(point.0, point.1);
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK,
                    dwExtraInfo: 0x54485044,
                    ..Default::default()
                },
            },
        };
        SendInput(&[input], std::mem::size_of::<INPUT>() as i32) == 1
    }
}
unsafe fn cursor() -> (f64, f64) {
    let mut point = POINT::default();
    let _ = GetPhysicalCursorPos(&mut point);
    (point.x as f64, point.y as f64)
}
struct Run {
    generation: u64,
    motion: PointerInertia,
    decay: f64,
    last: Instant,
}
pub fn spawn() -> Result<(Client, JoinHandle<()>)> {
    let (tx, rx) = mpsc::channel();
    let (ready_tx, ready_rx) = mpsc::sync_channel::<std::result::Result<(), String>>(1);
    let client = Client {
        tx,
        generation: Arc::new(AtomicU64::new(0)),
        active: Arc::new(AtomicBool::new(false)),
        ticks: Arc::new(AtomicU64::new(0)),
        max_gap_us: Arc::new(AtomicU64::new(0)),
    };
    let shared = client.clone();
    let worker = std::thread::spawn(move || unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
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
        let mut position: Option<(f64, f64)> = None;
        let mut desktop = Desktop::current();
        loop {
            let command = if run.is_none() {
                match rx.recv() {
                    Ok(command) => Some(command),
                    Err(_) => break,
                }
            } else {
                rx.try_recv().ok()
            };
            if let Some(command) = command {
                let generation = match &command {
                    Command::Move(g, ..) | Command::Start(g, ..) | Command::Stop(g) => *g,
                    Command::Shutdown => break,
                };
                if generation != shared.generation.load(Ordering::Acquire) {
                    continue;
                }
                match command {
                    Command::Stop(_) => {
                        run = None;
                        position = None;
                        desktop = Desktop::current();
                        shared.active.store(false, Ordering::Release);
                        let _ = CancelWaitableTimer(timer);
                    }
                    Command::Move(_, dx, dy) => {
                        let p = position.unwrap_or_else(|| cursor());
                        let p = desktop.clamp(p.0 + dx as f64, p.1 + dy as f64);
                        position = Some(p);
                        if generation == shared.generation.load(Ordering::Acquire)
                            && !desktop.send(p)
                        {
                            crate::ERRORS.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    Command::Start(_, motion, decay) => {
                        desktop = Desktop::current();
                        position = Some(position.unwrap_or_else(|| cursor()));
                        let due = -20_000i64; // 2ms, expressed in 100ns units. No process-wide timer-resolution change.
                        if SetWaitableTimer(timer, &due, 2, None, None, false).is_err() {
                            shared.active.store(false, Ordering::Release);
                            crate::ERRORS.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                        run = Some(Run {
                            generation,
                            motion,
                            decay,
                            last: Instant::now(),
                        });
                    }
                    Command::Shutdown => unreachable!(),
                }
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
            let now = Instant::now();
            shared.ticks.fetch_add(1, Ordering::Relaxed);
            shared.max_gap_us.fetch_max(
                now.saturating_duration_since(current.last).as_micros() as u64,
                Ordering::Relaxed,
            );
            current.last = now;
            if let Some((dx, dy)) = current.motion.tick_precise(now, current.decay) {
                let p = position.unwrap_or_else(|| cursor());
                let p = desktop.clamp(p.0 + dx, p.1 + dy);
                position = Some(p);
                if current.generation == shared.generation.load(Ordering::Acquire)
                    && !desktop.send(p)
                {
                    crate::ERRORS.fetch_add(1, Ordering::Relaxed);
                }
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
    fn absolute_coordinates_round_trip_across_negative_origin_desktops() {
        let desktop = Desktop {
            left: -3840,
            top: -200,
            width: 7680,
            height: 2360,
        };
        for x in [-3840, -2000, -1, 0, 1920, 3839] {
            let (nx, _) = desktop.normalized(x as f64, 100.0);
            let actual = desktop.left + (nx as i64 * desktop.width as i64 / 65536) as i32;
            assert_eq!(actual, x);
        }
        assert_eq!(desktop.clamp(-5000.0, 9000.0), (-3840.0, 2159.0));
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
