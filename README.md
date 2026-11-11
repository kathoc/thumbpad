[日本語](README.ja.md)

# Thumbpad

Click, drag, and scroll with keyboard keys while touching your laptop's touchpad. Thumbpad runs in the Windows system tray and includes an optional trackball pointer mode.

Thumbpad is an independent application inspired by ThumbSense. It is an experimental Windows 11 utility for built-in Precision Touchpads. Compatibility across laptop models is still being tested.

## Install

1. Download **thumbpad-setup-x64.exe** from the [latest release](https://github.com/kathoc/thumbpad/releases/latest).
2. Run the installer and launch Thumbpad.
3. Touch the touchpad, then press a click or scroll key.

The installer targets Windows 11 x64 and installs for your Windows account. You can optionally enable **Start with Windows** from the tray menu. Releases are currently unsigned.

To uninstall, use Windows Settings → Apps → Installed apps. Your settings file is retained.

## Keyboard controls

**CVB mode is the default.** Right-click the tray icon and choose **CVB mode** or **SDF mode**. The selection is saved and takes effect immediately.

| Action | CVB mode | SDF mode |
| --- | --- | --- |
| Left click | V or N | F or J |
| Right click | C or M | D or K |
| Middle click | G or H | C or M |
| Vertical scroll | Hold B and move a finger vertically | Hold S or L and move a finger vertically |
| Horizontal scroll | Hold Shift+B and move horizontally | Hold Shift+S or Shift+L and move horizontally |
| Drag | Hold a click key and move a finger | Hold a click key and move a finger |
| Pause / resume Thumbpad | Ctrl+Alt+F12 | Ctrl+Alt+F12 |

Touch the pad **before** pressing the action key. Keep a click key held to lift and reposition your finger during a long drag. Hold both left-click keys together if you want to transfer the hold from one hand to the other.

Keys pressed before touching the pad keep their original keyboard behavior until released. While touchpad controls are active, Ctrl, Alt, and Shift combine with the mapped mouse buttons for modified clicks and drags. Lift your finger before starting a keyboard shortcut that uses a mapped key. Windows-key shortcuts pass through. CVB mode leaves S, D, and F available for keyboard input, which can help with game controls. Games that require raw mouse input or block injected input may need separate compatibility testing.

Scroll inversion is enabled by default. Change **Invert vertical scroll** and **Invert horizontal scroll** from the tray menu. Holding Shift switches the scroll axis during a gesture. Excel horizontal scrolling includes a compatibility path, and behavior can vary by Excel version and view.

## Pointer and scrolling

The default **mouse mode** uses normal Windows pointer movement. Turn on **Trackball pointer mode (off = mouse)** in the tray menu to keep the pointer moving after a flick.

- Touch the pad again to stop inertia.
- Small movements have reduced inertia. Stationary jitter and a single lift sample are filtered.
- **Trackball pointer faster / slower** adjusts pointer sensitivity.
- **Hold pointer after inertia stop** enables a brief hold after touching to stop a glide. Its default duration is 100 ms.
- **Inertial scrolling** toggles momentum for keyboard-assisted vertical and horizontal scrolling.

Two usable fingers pause the trackball pointer for native touchpad gestures. Pointer movement resumes after the usable fingers have lifted and a new single-finger gesture begins.

### Mouse input compatibility

Thumbpad sends keyboard-assisted pointer movement and trackball inertia through standard Windows relative mouse input. The same input format is used for desktop applications, CAD tools, and game camera controls.

Fractional movement is retained between updates, including slow and diagonal inertia. Relative motion is independent of desktop bounds, so camera rotation can continue beyond the edges of the screen. Switching the active window stops an ongoing glide.

Desktop movement follows Windows mouse sensitivity and acceleration settings. Applications using raw relative movement apply their own sensitivity. You can adjust Thumbpad's sensitivity with **Trackball pointer faster / slower**.

## Edge protection

By default, the left and right 12% and the top 15% of the pad are excluded from starting pointer control.

- A finger that starts in the usable area can move all the way to an edge.
- A finger that starts in an excluded area is ignored until it lifts.
- You can rest a finger in an excluded area and move a separate finger that starts in the usable area.
- A 500 ms reconnect allowance helps with brief contact interruptions.
- While a click or scroll key is already held, repositioning can resume from an edge.

In trackball mode, resting closer to the physical top or side edges increases inertia braking. At the boundary with the usable area, inertia has its normal strength. Deeper inside an excluded strip, a glide decelerates faster and travels a shorter distance. A contact at the outermost edge stops the glide. This uses contact position, including palm-classified contacts, and applies to movement after lifting the operating finger.

## Settings

Right-click the tray icon and select **Edit settings** to open the built-in JSON editor. **Save and close** validates, saves, and applies your changes. **Cancel** discards the edit.

| Setting | What it controls |
| --- | --- |
| `key_mode` | `"cvb"` or `"sdf"`. Default `"cvb"`. |
| `pointer_mode` | `"mouse"` or `"trackball"`. Default `"mouse"`. |
| `trackball_pointer_speed` | Pointer sensitivity, 0.25–4.0. Default 1.25. |
| `pointer_inertia_min_travel_px` | Minimum confirmed relative movement before pointer inertia can start, 0–1000 units. Default 12. |
| `pointer_inertia_strength` | Pointer inertia strength, 0–2. Default 1. Set 0 to disable it. |
| `pointer_inertia_decay` | Pointer deceleration, 1–20. Larger values stop sooner. Default 5. |
| `pointer_stop_hold_enabled` / `pointer_stop_hold_ms` | Enable the stop-touch hold and set its duration, 0–2000 ms. |
| `inertia_enabled` / `inertia_decay` | Scroll inertia and its deceleration, 1–20. |
| `edge_side_percent` / `edge_top_percent` | Excluded side and top widths. Ranges 0–45% and 0–80%. |
| `edge_grace_ms` | Reconnect allowance, 0–5000 ms. Default 500. |
| `edge_pointer_guard` | Enable pointer movement suppression for excluded contacts. |
| `drag_pixels_per_pad_width` / `drag_pixels_per_pad_height` | Keyboard-assisted drag sensitivity over the pad's full width and height. |
| `scroll_units_per_pad_width` / `scroll_units_per_pad_height` | Horizontal and vertical scroll sensitivity. 120 wheel units equal one notch. |
| `scroll_invert` / `scroll_horizontal_invert` | Vertical and horizontal scroll direction. |

Settings are stored in `%LOCALAPPDATA%\Thumbpad\settings.json`.

## Updates and tray controls

Use **Check for updates** to check manually. **Automatic updates** enables checks after startup and once a day. Thumbpad asks before downloading and installing a newer release.

Double-click the tray icon to open diagnostics. Closing the diagnostics window keeps Thumbpad running. Choose **Exit** from the tray menu to quit.

## Compatibility and troubleshooting

- A built-in Windows Precision Touchpad that exposes contact reports is required. Older vendor-specific or PS/2-only touchpads are unsupported.
- In diagnostics, **HID reports** should increase when you move a finger. If they do not, that device or session is not currently supported.
- Edge protection and keyboard-assisted pointer control can also suppress an external mouse moved at the same time.
- Elevated applications, secure desktops, and some games can restrict generated mouse input.
- Touchpad tap-to-click can produce additional clicks alongside keyboard clicks.
- If input feels stuck, pause with Ctrl+Alt+F12 or exit and restart Thumbpad.

End-to-end input latency and compatibility across laptop models have not been comprehensively measured.

## Privacy and support

Thumbpad checks GitHub for releases. It does not record or transmit typed text or send usage analytics. Local diagnostics contain counters and device information. Update errors and crash reports are saved locally in `%LOCALAPPDATA%\Thumbpad`.

Report problems through [GitHub Issues](https://github.com/kathoc/thumbpad/issues), including your laptop model, Windows version, Thumbpad version, and the steps to reproduce.

## Development

Build requirements are Rust 1.96.0, Visual Studio C++ Build Tools, Windows SDK, and Inno Setup 6.3 or newer. Run `cargo test --locked`, then `./installer/build.ps1` to produce the installer and portable executable in `dist`. The project uses the MIT license.
