# Validation record — 2026-11-10

This is a prototype validation record, not a cross-device compatibility claim.

## Automated checks

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: 35 passed, 0 failed (v0.1.4).
- Optimized Windows x64 build: passed.
- Inno Setup 6.7.4 installer compilation: passed.

Covered cases: typing/click ownership across repeats, drag after contact loss,
overlapping left-click keys, release on pause, excluded starts, interior-to-edge
motion, 499/501ms reconnection boundaries, no grace from rejected motion,
serial/hybrid frame completion, empty frames, configuration bounds, update version
selection, foreign download URL rejection, missing digest, and corrupted/truncated downloads.
The mouse-hook regression test also invokes the callback while the UI state is borrowed.
v0.1.1 adds repeated multi-second F-held clutches at excluded edges, D/K shared
button ownership, G scroll ownership and pause, vertical wheel signs/inversion,
fractional motion accumulation, and rebasing on lifts, contact changes and mode epochs.

The user reported the initial implementation working, but drag resumption failing
after a lift, and possibly added external-mouse latency. v0.1.1 routes held-key
motion from HID, keeps the gesture accepted through clutches, and uses a dedicated
mouse-hook thread with the hook fully detached while untouched or paused.
Physical feel and latency of this revised path still need user verification.
The v0.1.1 installer was applied to the default per-user installation: exit 0,
installed executable matched the release build, and all previous settings were
preserved while the new motion/scroll defaults were added. The running UI reported
v0.1.1 and `Mouse hook: DETACHED (no mouse interception)` while idle.
v0.1.2 adds C middle-button ownership/release, reversed vertical and horizontal
defaults with saved overrides, rightward-to-left horizontal wheel signs, and live
Shift axis switching with a fresh motion epoch. These are covered by four new tests.
The installed copy was upgraded to v0.1.2 (exit 0), its executable hash matched
the release build, and both inverted-direction settings were confirmed enabled
on this PC at the user's request. Other existing settings were preserved.
v0.1.3 changes the binding to S/Shift+S and adds velocity-based decaying momentum.
New tests cover decay/termination, retouch cancellation, stale/stationary release,
horizontal direction, long-timer-stall safety, axis reset, rejected physical-contact
detection, and the S binding with G passing through. Inertia feel needs user testing.
The v0.1.3 installer completed with exit 0 at the default installation path;
the installed executable matched the release build, prior preferences were
preserved, and inertia defaults were added (enabled, decay 5.0).
v0.1.4 adds pointer trackball mode with mouse as the default, L/M aliases,
and an Excel-specific Ctrl+Shift wheel-message path without global modifier
injection. New tests cover 2D pointer momentum/cancellation, direct pointer mode,
mode serialization, shared alias ownership, and Excel message signs/modifiers.
The user confirmed horizontal scrolling in Chrome but reported failure in Excel
before this compatibility path. Excel's resulting behavior still needs user verification.
The v0.1.4 installer completed with exit 0, installed bytes matched the release
build, and all previous preferences were preserved. Pointer mode remained `mouse`
as the default; the trackball option and pointer decay default were added.

## Native UI checks

- Prototype window opened with diagnostics and controls.
- Pause / Resume changed the displayed state.
- Ctrl+Alt+F12 changed the displayed state; Exit terminated the application.
- Manual release lookup returned the expected no-stable-release message before publication.
- No physical touchpad reports were observed during the automated session.

## Installer smoke tests

- Per-user installation to a dedicated test directory: exit 0.
- Same-version overwrite: exit 0; installed executable SHA-256 matched the release build.
- Installed executable launch: passed.
- `/UPGRADE` installation: exit 0, settings file preserved, background process restarted.
- Reopening the background instance: passed after fixing STARTUPINFO/SW_HIDE handling.
- Uninstall while running: exit 0, executable removed, process stopped, settings preserved.
- A real newer-version download-to-install update has not been tested because no newer stable release exists yet.

## Fixes during local validation

An early installed build aborted during normal use (Windows reported c0000409).
The reentrant input hooks have been separated from the UI RefCell: mouse decisions
and counters use atomics, and keyboard ownership has its own state with no Windows
API calls inside its borrow. A regression test covers a mouse callback interrupting
borrowed UI state. Future Rust panic details are saved locally to crash.txt.

## Limitations still requiring physical testing

The host lists an ASUS Precision Touchpad in PnP, but the application has not
observed a physical contact in this session. F/J/K click conversion, physical cursor
suppression, palm rejection, simultaneous external-mouse behavior, and real input
latency have NOT been certified on this host or other devices.

Use TESTING.md for physical verification. The pointer edge guard is contact-correlated
and can also block simultaneous external-mouse movement. It is not a device-isolated
filter driver. No independent Authenticode or update-signing key is configured.
