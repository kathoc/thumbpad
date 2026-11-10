#![windows_subsystem = "windows"]
#![allow(unsafe_op_in_unsafe_fn)]
mod engine;
mod touchpad;
mod update;
use engine::{Action, Button, Engine};
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
struct App {
    engine: Engine,
    devices: touchpad::Devices,
    enabled: bool,
    label: HWND,
    clicks: u64,
    max_hook_us: u128,
    errors: u64,
    config: update::Config,
    blocked_moves: u64,
}
thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
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
impl App {
    unsafe fn release(&mut self, reset: bool) {
        let actions = if reset {
            self.engine.reset()
        } else {
            self.engine.release()
        };
        for a in actions {
            if !inject(&a) {
                self.errors += 1;
            }
        }
    }
    unsafe fn toggle(&mut self) {
        self.enabled = !self.enabled;
        self.release(false);
        self.refresh();
    }
    unsafe fn refresh(&self) {
        let text = format!(
            "Thumbpad {}  /  {}\r\n\r\nTouch the touchpad, then hold F / J for left click or K for right click.\r\nHold the key and move your finger to drag.\r\nCtrl / Alt / Win shortcuts pass through. Ctrl+Alt+F12 pauses.\r\n\r\n{}\r\n\r\nClicks: {}   Input injection failures: {}\r\nMax hook processing: {} us (not end-to-end latency)\r\n\r\nAutomatic update checks: {} (startup and every 24 hours)\r\nUpdates require confirmation before installation.\r\n\r\nRight-click the tray icon for settings and controls.\r\nClosing this window keeps Thumbpad running.",
            env!("CARGO_PKG_VERSION"),
            if self.enabled { "ENABLED" } else { "PAUSED" },
            self.devices.summary(),
            self.clicks,
            self.errors,
            self.max_hook_us,
            if self.config.auto_update { "ON" } else { "OFF" }
        );
        let text = format!(
            "{text}\r\nPointer edge guard: {} / blocked moves: {}\r\nMay also block an external mouse used during a rejected contact.",
            if self.config.edge_pointer_guard {
                "ON"
            } else {
                "OFF"
            },
            self.blocked_moves
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
    let index = match k.vkCode {
        0x46 => 0,
        0x4A => 1,
        0x4B => 2,
        _ => return CallNextHookEx(None, code, wp, lp),
    };
    let start = Instant::now();
    let consumed = APP.with_borrow_mut(|app| {
        let Some(a) = app else {
            return false;
        };
        let active = a.enabled
            && a.devices.touching()
            && ![VK_CONTROL, VK_MENU, VK_LWIN, VK_RWIN]
                .iter()
                .any(|vk| modifiers(vk.0 as i32));
        let (consumed, action) = a.engine.key(index, down, active);
        if let Some(action) = action {
            if inject(&action) {
                if action.down {
                    a.clicks += 1;
                }
            } else {
                a.errors += 1;
            }
        }
        a.max_hook_us = a.max_hook_us.max(start.elapsed().as_micros());
        consumed
    });
    if consumed {
        LRESULT(1)
    } else {
        CallNextHookEx(None, code, wp, lp)
    }
}
// A low-level mouse hook has no device handle. This prototype gates physical
// movement only while rejected touchpad contacts are present; simultaneous
// external-mouse movement can also be blocked. Injected input always passes.
unsafe extern "system" fn mouse(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code >= 0 && wp.0 == WM_MOUSEMOVE as usize {
        let event = &*(lp.0 as *const MSLLHOOKSTRUCT);
        if event.flags & LLMHF_INJECTED == 0 {
            let block = APP.with_borrow_mut(|app| {
                let Some(a) = app else {
                    return false;
                };
                let block = a.enabled && a.config.edge_pointer_guard && a.devices.block_pointer();
                if block {
                    a.blocked_moves += 1;
                }
                block
            });
            if block {
                return LRESULT(1);
            }
        }
    }
    CallNextHookEx(None, code, wp, lp)
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
    let tip = wide("Thumbpad - F/J: left click, K: right click");
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
            APP.with_borrow_mut(|app| {
                if let Some(a) = app {
                    a.devices.input(HRAWINPUT(lp.0 as _));
                }
            });
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
                AUTO_UPDATE => {
                    let error = APP.with_borrow_mut(|app| {
                        let a = app.as_mut()?;
                        a.config.auto_update = !a.config.auto_update;
                        let error = a.config.save().err().map(|e| e.to_string());
                        if error.is_some() {
                            a.config.auto_update = !a.config.auto_update;
                        }
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
                engine: Engine::default(),
                devices: touchpad::Devices::new(
                    config.edge_side_percent,
                    config.edge_top_percent,
                    config.edge_grace_ms,
                ),
                enabled: true,
                label,
                clicks: 0,
                max_hook_us: 0,
                errors: 0,
                blocked_moves: 0,
                config,
            })
        });
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard), Some(instance.into()), 0)?;
        let mouse_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse), Some(instance.into()), 0)?;
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
        let _ = UnhookWindowsHookEx(mouse_hook);
        let _ = CloseHandle(mutex);
        Ok(())
    }
}
fn main() {
    if let Err(e) = run() {
        message(&format!("Thumbpad could not start:\n{e}"));
    }
}
