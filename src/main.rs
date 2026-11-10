#![windows_subsystem = "windows"]
#![allow(unsafe_op_in_unsafe_fn)]
mod engine;
mod glide;
mod inertia;
mod motion;
mod touchpad;
mod update;
use engine::{Action, Button, Engine};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::{cell::RefCell, mem::size_of, time::Instant};
use windows::{
    Win32::{
        Foundation::*,
        System::{LibraryLoader::*, Threading::*},
        UI::{
            Input::{KeyboardAndMouse::*, *},
            Shell::*,
            WindowsAndMessaging::*,
        },
    },
    core::*,
};
const TRAY: u32 = WM_APP + 1;
const SHOW: u32 = WM_APP + 2;
const PAUSE: usize = 101;
const STATUS: usize = 102;
const UPDATE: usize = 103;
const EXIT: usize = 104;
const AUTO_UPDATE: usize = 105;
const STARTUP: usize = 106;
const SETTINGS: usize = 107;
const INVERT_SCROLL: usize = 108;
const INVERT_HSCROLL: usize = 109;
const INERTIA_ENABLED: usize = 110;
const POINTER_MODE: usize = 111;
const INERTIA_TIMER: usize = 2;
struct App {
    devices: touchpad::Devices,
    enabled: bool,
    label: HWND,
    config: update::Config,
    hwnd: HWND,
    inertia: inertia::Inertia,
    pointer_inertia: inertia::PointerInertia,
}
thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }
thread_local! { static ENGINE: RefCell<Engine> = RefCell::new(Engine::default()); }
// Hooks may interrupt User32 calls while APP is borrowed. They must never borrow
// APP, and must drop the engine borrow before making any Windows API call.
static CLICK_ACTIVE: AtomicBool = AtomicBool::new(false);
static POINTER_BLOCK: AtomicBool = AtomicBool::new(false);
static CLICKS: AtomicU64 = AtomicU64::new(0);
static ERRORS: AtomicU64 = AtomicU64::new(0);
static MAX_HOOK_US: AtomicU64 = AtomicU64::new(0);
static BLOCKED_MOVES: AtomicU64 = AtomicU64::new(0);
static INTERACTION: AtomicU64 = AtomicU64::new(0);
static HAS_CONTACTS: AtomicBool = AtomicBool::new(false);
static SCROLL_EVENTS: AtomicU64 = AtomicU64::new(0);
static MOUSE_THREAD: AtomicU32 = AtomicU32::new(0);
static MOUSE_READY: AtomicBool = AtomicBool::new(false);
static TRACKBALL_MODE: AtomicBool = AtomicBool::new(false);
static EXCEL_SCROLL_EVENTS: AtomicU64 = AtomicU64::new(0);
static GLIDE: std::sync::OnceLock<glide::Client> = std::sync::OnceLock::new();
fn pointer_running() -> bool {
    GLIDE.get().is_some_and(glide::Client::active)
}
const RECONFIGURE_MOUSE: u32 = WM_APP + 10;
fn publish_interaction(dragging: bool, scrolling: bool, shift: bool) {
    let flags =
        u64::from(dragging) | (u64::from(scrolling) << 1) | (u64::from(scrolling && shift) << 2);
    let old = INTERACTION.load(Ordering::Relaxed);
    if old & 7 != flags {
        INTERACTION.store(((old.wrapping_add(8)) & !7) | flags, Ordering::Relaxed);
    }
}
fn interaction() -> (motion::Mode, u64) {
    let value = INTERACTION.load(Ordering::Relaxed);
    (
        if value & 2 != 0 {
            if value & 4 != 0 {
                motion::Mode::HorizontalScroll
            } else {
                motion::Mode::Scroll
            }
        } else if value & 1 != 0 {
            motion::Mode::Drag
        } else {
            motion::Mode::Off
        },
        value >> 3,
    )
}
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}
fn flip_setting(config: &mut update::Config, command: usize) {
    match command {
        AUTO_UPDATE => config.auto_update = !config.auto_update,
        INVERT_SCROLL => config.scroll_invert = !config.scroll_invert,
        INVERT_HSCROLL => config.scroll_horizontal_invert = !config.scroll_horizontal_invert,
        INERTIA_ENABLED => config.inertia_enabled = !config.inertia_enabled,
        POINTER_MODE => {
            config.pointer_mode = if config.pointer_mode == update::PointerMode::Mouse {
                update::PointerMode::Trackball
            } else {
                update::PointerMode::Mouse
            }
        }
        _ => {}
    }
}
pub fn message(text: &str) {
    unsafe {
        MessageBoxW(None, PCWSTR(wide(text).as_ptr()), w!("Thumbpad"), MB_OK);
    }
}
unsafe fn inject(action: &Action) -> bool {
    let flags = match (action.button, action.down) {
        (Button::Left, true) => MOUSEEVENTF_LEFTDOWN,
        (Button::Left, false) => MOUSEEVENTF_LEFTUP,
        (Button::Right, true) => MOUSEEVENTF_RIGHTDOWN,
        (Button::Right, false) => MOUSEEVENTF_RIGHTUP,
        (Button::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
        (Button::Middle, false) => MOUSEEVENTF_MIDDLEUP,
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwFlags: flags,
                dwExtraInfo: 0x54485044,
                ..Default::default()
            },
        },
    };
    SendInput(&[input], size_of::<INPUT>() as i32) == 1
}
thread_local! { static EXCEL_WHEEL: std::cell::Cell<(isize, i32)> = const { std::cell::Cell::new((0, 0)) }; }
fn excel_wheel_parameters(horizontal: i32, point: POINT) -> (WPARAM, LPARAM) {
    // Excel uses Ctrl+Shift+vertical wheel: positive wheel scrolls left.
    let vertical = -horizontal.clamp(-32760, 32760);
    let wp = ((vertical as i16 as u16 as u32) << 16) | 0x000c;
    let lp = point.x as i16 as u16 as u32 | ((point.y as i16 as u16 as u32) << 16);
    (WPARAM(wp as usize), LPARAM(lp as isize))
}
unsafe fn excel_horizontal_scroll(delta: i32) -> bool {
    let mut point = POINT::default();
    if GetCursorPos(&mut point).is_err() {
        return false;
    }
    let target = WindowFromPoint(point);
    if target.0.is_null() {
        return false;
    }
    let root = GetAncestor(target, GA_ROOT);
    let mut name = [0u16; 64];
    let len = GetClassNameW(root, &mut name).max(0) as usize;
    if !String::from_utf16_lossy(&name[..len]).eq_ignore_ascii_case("XLMAIN") {
        return false;
    }
    let whole = EXCEL_WHEEL.with(|state| {
        let (previous, remainder) = state.get();
        let total = if previous == target.0 as isize {
            remainder
        } else {
            0
        } + delta;
        state.set((target.0 as isize, total % 120));
        (total / 120) * 120
    });
    if whole == 0 {
        return true;
    }
    let (wp, lp) = excel_wheel_parameters(whole, point);
    PostMessageW(Some(target), WM_MOUSEWHEEL, wp, lp).is_ok()
}
unsafe fn inject_motion(effect: motion::Effect) {
    if let motion::Effect::Move { dx, dy } = effect
        && TRACKBALL_MODE.load(Ordering::Relaxed)
        && let Some(worker) = GLIDE.get()
    {
        worker.move_by(dx, dy);
        return;
    }
    let input = match effect {
        motion::Effect::Move { dx, dy } => MOUSEINPUT {
            dx,
            dy,
            dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_MOVE_NOCOALESCE,
            ..Default::default()
        },
        motion::Effect::Wheel(delta) => {
            SCROLL_EVENTS.fetch_add(1, Ordering::Relaxed);
            MOUSEINPUT {
                mouseData: delta as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                ..Default::default()
            }
        }
        motion::Effect::HorizontalWheel(delta) => {
            SCROLL_EVENTS.fetch_add(1, Ordering::Relaxed);
            if excel_horizontal_scroll(delta) {
                EXCEL_SCROLL_EVENTS.fetch_add(1, Ordering::Relaxed);
                return;
            }
            MOUSEINPUT {
                mouseData: delta as u32,
                dwFlags: MOUSEEVENTF_HWHEEL,
                ..Default::default()
            }
        }
    };
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dwExtraInfo: 0x54485044,
                ..input
            },
        },
    };
    if SendInput(&[input], size_of::<INPUT>() as i32) != 1 {
        ERRORS.fetch_add(1, Ordering::Relaxed);
    }
}
impl App {
    unsafe fn stop_inertia(&mut self) {
        if let Some(worker) = GLIDE.get() {
            worker.stop();
        }
        self.inertia.stop();
        self.pointer_inertia.stop();
        let _ = KillTimer(Some(self.hwnd), INERTIA_TIMER);
    }
    unsafe fn release(&mut self, reset: bool) {
        self.stop_inertia();
        CLICK_ACTIVE.store(false, Ordering::Relaxed);
        POINTER_BLOCK.store(false, Ordering::Relaxed);
        TRACKBALL_MODE.store(false, Ordering::Relaxed);
        HAS_CONTACTS.store(false, Ordering::Relaxed);
        let thread = MOUSE_THREAD.load(Ordering::Relaxed);
        if thread != 0 {
            let _ = PostThreadMessageW(thread, RECONFIGURE_MOUSE, WPARAM(0), LPARAM(0));
        }
        publish_interaction(false, false, false);
        let actions = ENGINE.with_borrow_mut(|e| if reset { e.reset() } else { e.release() });
        for a in actions {
            if !inject(&a) {
                ERRORS.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
    fn publish(&self) {
        TRACKBALL_MODE.store(
            self.enabled && self.config.pointer_mode == update::PointerMode::Trackball,
            Ordering::Relaxed,
        );
        let contacts = self.enabled && self.devices.has_contacts();
        if HAS_CONTACTS.swap(contacts, Ordering::Relaxed) != contacts {
            let id = MOUSE_THREAD.load(Ordering::Relaxed);
            if id != 0 {
                unsafe {
                    let _ = PostThreadMessageW(id, RECONFIGURE_MOUSE, WPARAM(0), LPARAM(0));
                }
            }
        }
        CLICK_ACTIVE.store(self.enabled && self.devices.touching(), Ordering::Relaxed);
        POINTER_BLOCK.store(
            self.enabled && self.config.edge_pointer_guard && self.devices.block_pointer(),
            Ordering::Relaxed,
        );
    }
    unsafe fn toggle(&mut self) {
        self.enabled = !self.enabled;
        self.release(false);
        self.publish();
        self.refresh();
    }
    unsafe fn refresh(&self) {
        let text = format!(
            "Thumbpad {}  /  {}\r\n\r\nTouch: F/J = left, D/K = right, C/M = middle, S/L = scroll.\r\nHold a click key to drag; lift and reposition without releasing the key.\r\nShift+S or Shift+L = horizontal scroll (rightward motion scrolls left by default).\r\nCtrl+Alt+F12 pauses. Change scroll direction from the tray menu.\r\nCtrl / Alt / Win shortcuts pass through.\r\n\r\n{}\r\n\r\nClicks: {}   Input / hook failures: {}\r\nMax keyboard hook processing: {} us (not end-to-end latency)\r\n\r\nAutomatic update checks: {} (startup and every 24 hours)\r\nUpdates require confirmation before installation.\r\n\r\nRight-click the tray icon for settings and controls.\r\nClosing this window keeps Thumbpad running.",
            env!("CARGO_PKG_VERSION"),
            if self.enabled { "ENABLED" } else { "PAUSED" },
            self.devices.summary(),
            CLICKS.load(Ordering::Relaxed),
            ERRORS.load(Ordering::Relaxed),
            MAX_HOOK_US.load(Ordering::Relaxed),
            if self.config.auto_update { "ON" } else { "OFF" }
        );
        let text = format!(
            "{text}\r\nMouse hook: {} / scroll events: {}",
            if MOUSE_READY.load(Ordering::Relaxed) {
                "ACTIVE (dedicated thread)"
            } else {
                "DETACHED (no mouse interception)"
            },
            SCROLL_EVENTS.load(Ordering::Relaxed)
        );
        let text = format!(
            "{text}\r\nPointer edge guard: {} / blocked moves: {}\r\nMay also block an external mouse used during a rejected contact.",
            if self.config.edge_pointer_guard {
                "ON"
            } else {
                "OFF"
            },
            BLOCKED_MOVES.load(Ordering::Relaxed)
        );
        let text = format!(
            "{text}\r\nInertial scrolling: {} (touch the pad to stop)",
            if self.inertia.active() {
                "RUNNING"
            } else if self.config.inertia_enabled {
                "ON"
            } else {
                "OFF"
            }
        );
        let text = format!(
            "{text}\r\nPointer mode: {} / Excel horizontal samples: {}",
            if self.config.pointer_mode == update::PointerMode::Trackball {
                if pointer_running() {
                    "TRACKBALL (rolling)"
                } else {
                    "TRACKBALL"
                }
            } else {
                "MOUSE"
            },
            EXCEL_SCROLL_EVENTS.load(Ordering::Relaxed)
        );
        let (ticks, max_gap) = GLIDE.get().map(glide::Client::stats).unwrap_or_default();
        let text = format!(
            "{text}\r\nPointer glide: high-resolution 2ms / ticks {ticks} / max gap {max_gap} us"
        );
        let _ = SetWindowTextW(self.label, PCWSTR(wide(&text).as_ptr()));
    }
}
unsafe extern "system" fn keyboard(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wp, lp);
    }
    let k = &*(lp.0 as *const KBDLLHOOKSTRUCT);
    if k.flags.contains(LLKHF_INJECTED) {
        return CallNextHookEx(None, code, wp, lp);
    }
    let down = wp.0 == WM_KEYDOWN as usize || wp.0 == WM_SYSKEYDOWN as usize;
    let up = wp.0 == WM_KEYUP as usize || wp.0 == WM_SYSKEYUP as usize;
    if !down && !up {
        return CallNextHookEx(None, code, wp, lp);
    }
    let modifiers = |vk| GetAsyncKeyState(vk) < 0;
    if matches!(k.vkCode, 0x10 | 0xa0 | 0xa1) {
        // Low-level callbacks precede the async-state update for this event.
        let shift = down
            || match k.vkCode {
                0xa0 => modifiers(VK_RSHIFT.0 as i32),
                0xa1 => modifiers(VK_LSHIFT.0 as i32),
                _ => false,
            };
        let (dragging, scrolling) = ENGINE.with_borrow(|e| (e.dragging(), e.scrolling()));
        publish_interaction(dragging, scrolling, shift);
        return CallNextHookEx(None, code, wp, lp);
    }
    let Some(index) = binding(k.vkCode) else {
        return CallNextHookEx(None, code, wp, lp);
    };
    let start = Instant::now();
    let active = CLICK_ACTIVE.load(Ordering::Relaxed)
        && ![VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
            .iter()
            .any(|vk| modifiers(vk.0 as i32));
    let (consumed, action, dragging, scrolling) = ENGINE.with_borrow_mut(|e| {
        let (consumed, action) = e.key(index, down, active);
        (consumed, action, e.dragging(), e.scrolling())
    });
    publish_interaction(dragging, scrolling, modifiers(VK_SHIFT.0 as i32));
    if let Some(action) = action {
        if inject(&action) {
            if action.down {
                CLICKS.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            ERRORS.fetch_add(1, Ordering::Relaxed);
        }
    }
    MAX_HOOK_US.fetch_max(start.elapsed().as_micros() as u64, Ordering::Relaxed);
    if consumed {
        LRESULT(1)
    } else {
        CallNextHookEx(None, code, wp, lp)
    }
}
fn binding(vk: u32) -> Option<usize> {
    match vk {
        0x46 => Some(0),
        0x4a => Some(1),
        0x4b => Some(2),
        0x44 => Some(3),
        0x53 => Some(4),
        0x43 => Some(5),
        0x4c => Some(6),
        0x4d => Some(7),
        _ => None,
    }
}
// A low-level mouse hook has no device handle. This prototype gates physical
// movement only while rejected touchpad contacts are present; simultaneous
// external-mouse movement can also be blocked. Injected input always passes.
unsafe extern "system" fn mouse(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && wp.0 == WM_MOUSEMOVE as usize {
        let event = &*(lp.0 as *const MSLLHOOKSTRUCT);
        if event.flags & LLMHF_INJECTED == 0 {
            let block = POINTER_BLOCK.load(Ordering::Relaxed)
                || ((INTERACTION.load(Ordering::Relaxed) & 3 != 0
                    || TRACKBALL_MODE.load(Ordering::Relaxed))
                    && HAS_CONTACTS.load(Ordering::Relaxed));
            if block {
                BLOCKED_MOVES.fetch_add(1, Ordering::Relaxed);
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wp, lp)
}
fn start_mouse_thread() -> Result<(std::thread::JoinHandle<()>, u32)> {
    let (tx, rx) = std::sync::mpsc::sync_channel(1);
    let worker = std::thread::spawn(move || unsafe {
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
        let id = GetCurrentThreadId();
        MOUSE_THREAD.store(id, Ordering::Relaxed);
        if tx.send(id).is_err() {
            return;
        }
        let mut hook = None;
        while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
            if msg.message == RECONFIGURE_MOUSE {
                if HAS_CONTACTS.load(Ordering::Relaxed) && hook.is_none() {
                    match SetWindowsHookExW(
                        WH_MOUSE_LL,
                        Some(mouse),
                        GetModuleHandleW(None).ok().map(Into::into),
                        0,
                    ) {
                        Ok(installed) => {
                            hook = Some(installed);
                            MOUSE_READY.store(true, Ordering::Relaxed);
                        }
                        Err(_) => {
                            ERRORS.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                } else if !HAS_CONTACTS.load(Ordering::Relaxed)
                    && let Some(installed) = hook.take()
                {
                    MOUSE_READY.store(false, Ordering::Relaxed);
                    let _ = UnhookWindowsHookEx(installed);
                }
            }
        }
        MOUSE_READY.store(false, Ordering::Relaxed);
        MOUSE_THREAD.store(0, Ordering::Relaxed);
        if let Some(installed) = hook {
            let _ = UnhookWindowsHookEx(installed);
        }
    });
    let id = rx.recv().map_err(|e| Error::new(E_FAIL, e.to_string()))?;
    Ok((worker, id))
}
unsafe fn tray(hwnd: HWND, add: bool) {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: TRAY,
        hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
        ..Default::default()
    };
    let tip = wide("Thumbpad - F/J: left, D/K: right, C/M: middle, S/L: scroll");
    data.szTip[..tip.len()].copy_from_slice(&tip);
    let _ = Shell_NotifyIconW(if add { NIM_ADD } else { NIM_DELETE }, &data);
}
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_HOTKEY => {
            APP.with_borrow_mut(|app| {
                if let Some(a) = app {
                    a.toggle();
                }
            });
            return LRESULT(0);
        }
        WM_INPUT => {
            let effects = APP.with_borrow_mut(|app| {
                if let Some(a) = app {
                    let (key_mode, epoch) = interaction();
                    let trackball = a.config.pointer_mode == update::PointerMode::Trackball;
                    let mode = if key_mode == motion::Mode::Off && trackball {
                        motion::Mode::Pointer
                    } else {
                        key_mode
                    };
                    let was_physical = a.devices.physical_contacts();
                    let (effects, complete) = a.devices.input(
                        HRAWINPUT(lp.0 as _),
                        mode,
                        epoch,
                        a.config.motion_tuning(),
                    );
                    let now = Instant::now();
                    if a.devices.physical_contacts() {
                        if !was_physical || a.inertia.active() || pointer_running() {
                            a.stop_inertia();
                        }
                        if complete && a.enabled {
                            let axis = match mode {
                                motion::Mode::Scroll | motion::Mode::HorizontalScroll => Some(mode),
                                _ => None,
                            };
                            let delta: f64 = effects
                                .iter()
                                .filter_map(|e| match e {
                                    motion::Effect::Wheel(d)
                                    | motion::Effect::HorizontalWheel(d) => Some(*d as f64),
                                    _ => None,
                                })
                                .sum();
                            a.inertia.record(axis, delta, now);
                            if trackball
                                && matches!(mode, motion::Mode::Pointer | motion::Mode::Drag)
                            {
                                let (dx, dy) =
                                    effects.iter().fold((0.0, 0.0), |(x, y), e| match e {
                                        motion::Effect::Move { dx, dy } => {
                                            (x + *dx as f64, y + *dy as f64)
                                        }
                                        _ => (x, y),
                                    });
                                a.pointer_inertia.record(dx, dy, now);
                            } else {
                                a.pointer_inertia.stop();
                            }
                        }
                    } else if was_physical && a.enabled {
                        let scroll = a.config.inertia_enabled && a.inertia.release(now);
                        let pointer = trackball && a.pointer_inertia.release(now);
                        if pointer && let Some(worker) = GLIDE.get() {
                            worker.start(
                                std::mem::take(&mut a.pointer_inertia),
                                a.config.pointer_inertia_decay,
                            );
                        }
                        if scroll && SetTimer(Some(hwnd), INERTIA_TIMER, 16, None) == 0 {
                            a.stop_inertia();
                            ERRORS.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                    a.publish();
                    effects
                } else {
                    Vec::new()
                }
            });
            if MOUSE_READY.load(Ordering::Relaxed) {
                for effect in effects {
                    inject_motion(effect);
                }
            }
        }
        WM_INPUT_DEVICE_CHANGE => {
            APP.with_borrow_mut(|app| {
                if let Some(a) = app {
                    a.devices.clear();
                    a.release(false);
                }
            });
        }
        WM_POWERBROADCAST | 0x02B1 => {
            APP.with_borrow_mut(|app| {
                if let Some(a) = app {
                    a.release(true);
                    a.devices.clear();
                }
            });
        }
        WM_TIMER if wp.0 == INERTIA_TIMER => {
            let effect = APP.with_borrow_mut(|app| {
                let a = app.as_mut()?;
                if !a.enabled || a.devices.physical_contacts() {
                    a.stop_inertia();
                    return Some(Vec::new());
                }
                if !a.config.inertia_enabled {
                    a.inertia.stop();
                }
                let now = Instant::now();
                let effects: Vec<_> = a
                    .inertia
                    .tick(now, a.config.inertia_decay)
                    .into_iter()
                    .collect();
                if !a.inertia.active() {
                    let _ = KillTimer(Some(hwnd), INERTIA_TIMER);
                }
                Some(effects)
            });
            if let Some(effects) = effect {
                for effect in effects {
                    inject_motion(effect);
                }
            }
            return LRESULT(0);
        }
        WM_TIMER => {
            APP.with_borrow(|app| {
                if let Some(a) = app {
                    a.refresh();
                }
            });
            return LRESULT(0);
        }
        WM_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            return LRESULT(0);
        }
        SHOW => {
            let _ = ShowWindow(hwnd, SW_SHOW);
            let _ = SetForegroundWindow(hwnd);
            return LRESULT(0);
        }
        WM_COMMAND => {
            match wp.0 & 0xffff {
                PAUSE => APP.with_borrow_mut(|app| {
                    if let Some(a) = app {
                        a.toggle();
                    }
                }),
                STATUS => {
                    let _ = PostMessageW(Some(hwnd), SHOW, WPARAM(0), LPARAM(0));
                }
                UPDATE => update::check_async(true),
                command @ (AUTO_UPDATE | INVERT_SCROLL | INVERT_HSCROLL | INERTIA_ENABLED
                | POINTER_MODE) => {
                    let error = APP.with_borrow_mut(|app| {
                        let a = app.as_mut()?;
                        flip_setting(&mut a.config, command);
                        let error = a.config.save().err().map(|e| e.to_string());
                        if error.is_some() {
                            flip_setting(&mut a.config, command);
                        }
                        if command != AUTO_UPDATE {
                            a.stop_inertia();
                        }
                        a.publish();
                        a.refresh();
                        error
                    });
                    if let Some(error) = error {
                        message(&error);
                    }
                }
                STARTUP => {
                    if let Err(e) = update::toggle_startup() {
                        message(&e.to_string());
                    }
                }
                SETTINGS => {
                    if let Err(e) = update::open_settings() {
                        message(&e.to_string());
                    }
                }
                EXIT => {
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
            return LRESULT(0);
        }
        TRAY => {
            if lp.0 as u32 == WM_LBUTTONDBLCLK {
                let _ = PostMessageW(Some(hwnd), SHOW, WPARAM(0), LPARAM(0));
            }
            if lp.0 as u32 == WM_RBUTTONUP
                && let Ok(menu) = CreatePopupMenu()
            {
                APP.with_borrow(|app| {
                    if let Some(a) = app {
                        let _ = AppendMenuW(
                            menu,
                            MF_STRING,
                            PAUSE,
                            if a.enabled { w!("Pause") } else { w!("Resume") },
                        );
                        let _ = AppendMenuW(menu, MF_STRING, STATUS, w!("Status / diagnostics"));
                        let _ = AppendMenuW(
                            menu,
                            MF_STRING,
                            SETTINGS,
                            w!("Edit settings (restart to apply)"),
                        );
                        let _ = AppendMenuW(menu, MF_STRING, UPDATE, w!("Check for updates"));
                        let _ = AppendMenuW(
                            menu,
                            MF_STRING
                                | if a.config.pointer_mode == update::PointerMode::Trackball {
                                    MF_CHECKED
                                } else {
                                    MF_UNCHECKED
                                },
                            POINTER_MODE,
                            w!("Trackball pointer mode (off = mouse)"),
                        );
                        let _ = AppendMenuW(
                            menu,
                            MF_STRING
                                | if a.config.inertia_enabled {
                                    MF_CHECKED
                                } else {
                                    MF_UNCHECKED
                                },
                            INERTIA_ENABLED,
                            w!("Inertial scrolling"),
                        );
                        for (id, label, checked) in [
                            (
                                INVERT_SCROLL,
                                w!("Invert vertical scroll"),
                                a.config.scroll_invert,
                            ),
                            (
                                INVERT_HSCROLL,
                                w!("Invert horizontal scroll"),
                                a.config.scroll_horizontal_invert,
                            ),
                        ] {
                            let _ = AppendMenuW(
                                menu,
                                MF_STRING | if checked { MF_CHECKED } else { MF_UNCHECKED },
                                id,
                                label,
                            );
                        }
                        let _ = AppendMenuW(
                            menu,
                            MF_STRING
                                | if a.config.auto_update {
                                    MF_CHECKED
                                } else {
                                    MF_UNCHECKED
                                },
                            AUTO_UPDATE,
                            w!("Automatic update checks"),
                        );
                        let _ = AppendMenuW(
                            menu,
                            MF_STRING
                                | if update::startup_enabled() {
                                    MF_CHECKED
                                } else {
                                    MF_UNCHECKED
                                },
                            STARTUP,
                            w!("Start with Windows"),
                        );
                        let _ = AppendMenuW(menu, MF_STRING, EXIT, w!("Exit"));
                    }
                });
                let mut point = POINT::default();
                let _ = GetCursorPos(&mut point);
                let _ = SetForegroundWindow(hwnd);
                let _ = TrackPopupMenu(menu, TPM_RIGHTBUTTON, point.x, point.y, None, hwnd, None);
                let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
                let _ = DestroyMenu(menu);
            }
            return LRESULT(0);
        }
        WM_DESTROY => {
            APP.with_borrow_mut(|app| {
                if let Some(a) = app {
                    a.release(false);
                }
            });
            tray(hwnd, false);
            PostQuitMessage(0);
            return LRESULT(0);
        }
        _ => {}
    }
    DefWindowProcW(hwnd, msg, wp, lp)
}
fn run() -> Result<()> {
    unsafe {
        if std::env::args().any(|s| s == "--quit") {
            if let Ok(hwnd) = FindWindowW(w!("ThumbpadWindow"), None) {
                let _ = PostMessageW(Some(hwnd), WM_COMMAND, WPARAM(EXIT), LPARAM(0));
            }
            return Ok(());
        }
        let mutex = CreateMutexW(None, false, w!("Local\\Thumbpad.Singleton"))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Ok(hwnd) = FindWindowW(w!("ThumbpadWindow"), None) {
                let _ = PostMessageW(Some(hwnd), SHOW, WPARAM(0), LPARAM(0));
            }
            let _ = CloseHandle(mutex);
            return Ok(());
        }
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(wndproc),
            hInstance: instance.into(),
            lpszClassName: w!("ThumbpadWindow"),
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(
                (windows::Win32::Graphics::Gdi::COLOR_WINDOW.0 + 1) as _,
            ),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(Error::from_thread());
        }
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("ThumbpadWindow"),
            w!("Thumbpad - Status / diagnostics"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            800,
            600,
            None,
            None,
            Some(instance.into()),
            None,
        )?;
        let label = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!(""),
            WS_CHILD | WS_VISIBLE,
            24,
            20,
            740,
            460,
            Some(hwnd),
            None,
            Some(instance.into()),
            None,
        )?;
        let font = windows::Win32::Graphics::Gdi::GetStockObject(
            windows::Win32::Graphics::Gdi::DEFAULT_GUI_FONT,
        );
        SendMessageW(
            label,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        for (index, (id, title)) in [
            (PAUSE, "Pause / Resume"),
            (UPDATE, "Check updates"),
            (SETTINGS, "Settings"),
            (EXIT, "Exit"),
        ]
        .into_iter()
        .enumerate()
        {
            let button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                PCWSTR(wide(title).as_ptr()),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                24 + index as i32 * 180,
                500,
                165,
                32,
                Some(hwnd),
                Some(HMENU(id as _)),
                Some(instance.into()),
                None,
            )?;
            SendMessageW(
                button,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        windows::Win32::System::RemoteDesktop::WTSRegisterSessionNotification(
            hwnd,
            windows::Win32::System::RemoteDesktop::NOTIFY_FOR_THIS_SESSION,
        )?;
        RegisterHotKey(
            Some(hwnd),
            1,
            MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
            VK_F12.0 as u32,
        )?;
        RegisterRawInputDevices(
            &[RAWINPUTDEVICE {
                usUsagePage: 0x0D,
                usUsage: 0x05,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: hwnd,
            }],
            size_of::<RAWINPUTDEVICE>() as u32,
        )?;
        let config = update::Config::load();
        APP.with_borrow_mut(|a| {
            *a = Some(App {
                devices: touchpad::Devices::new(
                    config.edge_side_percent,
                    config.edge_top_percent,
                    config.edge_grace_ms,
                ),
                enabled: true,
                label,
                config,
                hwnd,
                inertia: inertia::Inertia::default(),
                pointer_inertia: inertia::PointerInertia::default(),
            })
        });
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), Some(instance.into()), 0)?;
        // No global mouse hook while the touchpad is untouched. When needed it
        // runs on its own message thread, independent of rendering and settings.
        let (mouse_worker, mouse_thread_id) = start_mouse_thread()?;
        let (pointer_client, pointer_worker) = glide::spawn()?;
        let _ = GLIDE.set(pointer_client);
        tray(hwnd, true);
        SetTimer(Some(hwnd), 1, 500, None);
        // Consume STARTUPINFO's show command even for a hidden launch. Otherwise
        // an installer's SW_HIDE can override the first later "show" request.
        let show = if std::env::args().any(|s| s == "--background") {
            SW_HIDE
        } else {
            SW_SHOW
        };
        let _ = ShowWindow(hwnd, show);
        let _ = ShowWindow(hwnd, show);
        APP.with_borrow(|app| {
            if let Some(a) = app {
                a.refresh();
            }
        });
        update::start_scheduler();
        let mut msg = MSG::default();
        loop {
            let result = GetMessageW(&mut msg, None, 0, 0);
            if result.0 <= 0 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        let _ = UnhookWindowsHookEx(hook);
        let _ = PostThreadMessageW(mouse_thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        let _ = mouse_worker.join();
        if let Some(worker) = GLIDE.get() {
            worker.shutdown();
        }
        let _ = pointer_worker.join();
        let _ = CloseHandle(mutex);
        Ok(())
    }
}
fn main() {
    std::panic::set_hook(Box::new(|info| {
        let folder = update::data_dir();
        let _ = std::fs::create_dir_all(&folder);
        let _ = std::fs::write(
            folder.join("crash.txt"),
            format!("{info}\n{}", std::backtrace::Backtrace::force_capture()),
        );
    }));
    if let Err(e) = run() {
        message(&format!("Thumbpad could not start:\n{e}"));
    }
}

#[cfg(test)]
mod callback_tests {
    use super::*;
    #[test]
    fn excel_wheel_uses_ctrl_shift_and_correct_leftward_sign_without_key_injection() {
        let (wp, lp) = excel_wheel_parameters(-120, POINT { x: -15, y: 500 });
        assert_eq!(wp.0 & 0xffff, 0x000c);
        assert_eq!(((wp.0 >> 16) & 0xffff) as u16 as i16, 120);
        assert_eq!((lp.0 & 0xffff) as u16 as i16, -15);
        assert_eq!(((lp.0 >> 16) & 0xffff) as u16 as i16, 500);
        let (wp, _) = excel_wheel_parameters(120, POINT::default());
        assert_eq!(((wp.0 >> 16) & 0xffff) as u16 as i16, -120);
        assert_eq!(binding(0x4c), Some(6));
        assert_eq!(binding(0x4d), Some(7));
    }
    #[test]
    fn s_replaces_g_as_the_scroll_binding() {
        assert_eq!(binding(0x53), Some(4));
        assert_eq!(binding(0x47), None);
        assert_eq!(binding(0x43), Some(5));
    }
    #[test]
    fn shift_switches_s_axis_and_rebases_the_gesture() {
        publish_interaction(false, true, false);
        let (mode, epoch) = interaction();
        assert_eq!(mode, motion::Mode::Scroll);
        publish_interaction(false, true, true);
        let (mode, shifted) = interaction();
        assert_eq!(mode, motion::Mode::HorizontalScroll);
        assert_ne!(epoch, shifted);
        publish_interaction(false, true, false);
        let (mode, released) = interaction();
        assert_eq!(mode, motion::Mode::Scroll);
        assert_ne!(shifted, released);
        publish_interaction(false, false, false);
        let mut config = update::Config::default();
        flip_setting(&mut config, INVERT_SCROLL);
        assert!(!config.scroll_invert);
        flip_setting(&mut config, INVERT_HSCROLL);
        assert!(!config.scroll_horizontal_invert);
    }
    #[test]
    fn mouse_hook_can_interrupt_a_borrowed_ui_state() {
        let event = MSLLHOOKSTRUCT::default();
        POINTER_BLOCK.store(true, Ordering::Relaxed);
        let result = APP.with_borrow_mut(|_| unsafe {
            mouse(
                0,
                WPARAM(WM_MOUSEMOVE as usize),
                LPARAM(&event as *const _ as isize),
            )
        });
        POINTER_BLOCK.store(false, Ordering::Relaxed);
        assert_eq!(result.0, 1);
    }
}
