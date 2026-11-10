//! Native settings editor; no file association or external editor is required.
use crate::{APP, touchpad, update};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::*,
        UI::{Controls::EM_SETLIMITTEXT, WindowsAndMessaging::*},
    },
    core::*,
};
const EDIT: i32 = 201;
const SAVE: usize = 202;
const CANCEL: usize = 203;

unsafe extern "system" fn procedure(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND if wp.0 & 0xffff == SAVE => {
            let result = (|| -> std::result::Result<(), String> {
                let edit = GetDlgItem(Some(hwnd), EDIT).map_err(|e| e.to_string())?;
                let mut text = vec![0u16; GetWindowTextLengthW(edit) as usize + 1];
                let len = GetWindowTextW(edit, &mut text) as usize;
                let config = update::Config::parse(&String::from_utf16_lossy(&text[..len]))
                    .map_err(|e| e.to_string())?;
                config.save().map_err(|e| e.to_string())?;
                APP.with_borrow_mut(|app| {
                    if let Some(a) = app {
                        a.release(true);
                        a.devices = touchpad::Devices::new(
                            config.edge_side_percent,
                            config.edge_top_percent,
                            config.edge_grace_ms,
                        );
                        a.config = config;
                        a.publish();
                        a.refresh();
                    }
                });
                Ok(())
            })();
            match result {
                Ok(()) => {
                    let _ = DestroyWindow(hwnd);
                }
                Err(error) => {
                    let text = crate::wide(&format!("Settings were not saved.\r\n\r\n{error}"));
                    MessageBoxW(
                        Some(hwnd),
                        PCWSTR(text.as_ptr()),
                        w!("Thumbpad settings"),
                        MB_OK | MB_ICONERROR,
                    );
                }
            }
            LRESULT(0)
        }
        WM_COMMAND if wp.0 & 0xffff == CANCEL => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub unsafe fn open() -> Result<()> {
    if let Ok(hwnd) = FindWindowW(w!("ThumbpadSettings"), None) {
        let _ = ShowWindow(hwnd, SW_RESTORE);
        let _ = SetForegroundWindow(hwnd);
        return Ok(());
    }
    let text = APP
        .with_borrow(|app| serde_json::to_string_pretty(&app.as_ref().expect("running app").config))
        .map_err(|e| Error::new(E_FAIL, e.to_string()))?;
    let text = crate::wide(&text.replace('\n', "\r\n"));
    let instance = GetModuleHandleW(None)?;
    let class = WNDCLASSW {
        lpfnWndProc: Some(procedure),
        hInstance: instance.into(),
        lpszClassName: w!("ThumbpadSettings"),
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        hbrBackground: HBRUSH((COLOR_WINDOW.0 + 1) as _),
        ..Default::default()
    };
    RegisterClassW(&class);
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        w!("ThumbpadSettings"),
        w!("Thumbpad - Edit Settings"),
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
        CW_USEDEFAULT,
        CW_USEDEFAULT,
        800,
        740,
        None,
        None,
        Some(instance.into()),
        None,
    )?;
    let result = (|| -> Result<()> {
        let label = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("STATIC"),
            w!(
                "Edit JSON settings. Save applies changes immediately. Cancel keeps current settings."
            ),
            WS_CHILD | WS_VISIBLE,
            16,
            16,
            750,
            28,
            Some(hwnd),
            None,
            Some(instance.into()),
            None,
        )?;
        let edit = CreateWindowExW(
            WS_EX_CLIENTEDGE,
            w!("EDIT"),
            PCWSTR(text.as_ptr()),
            WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WS_VSCROLL
                | WS_HSCROLL
                | WINDOW_STYLE(
                    (ES_MULTILINE | ES_WANTRETURN | ES_AUTOVSCROLL | ES_AUTOHSCROLL) as u32,
                ),
            16,
            48,
            750,
            590,
            Some(hwnd),
            Some(HMENU(EDIT as _)),
            Some(instance.into()),
            None,
        )?;
        SendMessageW(edit, EM_SETLIMITTEXT, Some(WPARAM(65536)), None);
        let font = GetStockObject(DEFAULT_GUI_FONT);
        SendMessageW(
            label,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        let font = GetStockObject(ANSI_FIXED_FONT);
        SendMessageW(
            edit,
            WM_SETFONT,
            Some(WPARAM(font.0 as usize)),
            Some(LPARAM(1)),
        );
        for (id, title, x) in [
            (SAVE, w!("Save and close"), 430),
            (CANCEL, w!("Cancel"), 610),
        ] {
            let button = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                title,
                WS_CHILD | WS_VISIBLE | WS_TABSTOP,
                x,
                652,
                156,
                32,
                Some(hwnd),
                Some(HMENU(id as _)),
                Some(instance.into()),
                None,
            )?;
            let font = GetStockObject(DEFAULT_GUI_FONT);
            SendMessageW(
                button,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(1)),
            );
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = DestroyWindow(hwnd);
    }
    result?;
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetForegroundWindow(hwnd);
    Ok(())
}
