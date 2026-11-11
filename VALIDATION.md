# Validation record — 2026-11-10

This is a prototype validation record, not a cross-device compatibility claim.

## Automated checks

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: 41 passed, 0 failed, 1 ignored (v0.1.6).
- Separate native timer probe: passed; 300 intervals, median 2108 us, p95 2554 us, max 2930 us on this PC. This probe generates no pointer input and does not measure rendering latency.
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
v0.1.5 moves pointer momentum off WM_TIMER/UI onto a dedicated high-resolution
2ms waitable timer. Fractional 2D distances are retained until final physical-pixel
output; a low-speed minor axis is no longer discarded independently. Tests cover
fractional shallow diagonals, tick-rate-independent total distance, and absolute
coordinate round trips across a negative-origin virtual desktop.
Trackball direct movement and momentum use the same absolute-pixel output, avoiding
relative mouse acceleration on each tick. Queued movement uses cancellation generations.
Smoothness under actual flicks still requires user comparison with v0.1.4.
The v0.1.5 installer completed with exit 0 at the default location. Installed
bytes matched the release build and settings.json was byte-for-byte unchanged,
including the user's enabled trackball mode. The updated application started.
The user confirmed that v0.1.5 movement became smooth.
v0.1.6 adds a 300ms pointer-output hold only when contact stops inertia in trackball
mode. The HID baseline continues to update while movement is discarded; the first
post-deadline sample rebases so retained finger displacement is not replayed.
Tests cover deadline boundaries, resuming without lifting, non-extended deadlines,
long sample gaps, normal contacts/mouse mode, and disabling the hold.
The v0.1.6 installer completed with exit 0, installed bytes matched the release
build, existing preferences were preserved, and pointer_stop_hold_ms migrated to 300.

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

## v0.1.7

44 tests passed, one native timing probe ignored. Added coverage for tiny-gesture suppression, monotonic bounded travel gain, unchanged maximum release speed, preserved pointer direction, and persistent hold toggle duration. Pointer travel ramps from 12 to 120 pixels; wheel travel ramps from 18 to 180 units using smoothstep. Existing release-velocity and stale-release checks remain active.

## v0.1.8

46 tests passed, one native timing probe ignored; formatting and clippy passed. Added a frame-level regression for single-finger movement, a second finger at an excluded edge, a remaining finger after scrolling, full lift and a jump-free new gesture. Trackball pointer velocity estimation is reset during the multi-finger gesture. Configuration tests verify independent pointer gain, unchanged wheel/mouse tuning, preserved hold duration, and invalid gain rejection. Default trackball sensitivity is now 125%; this is an adjustable initial tuning, not a measured match to Windows Precision Touchpad acceleration. Physical two-finger scrolling and perceived sensitivity still require user verification.

## v0.1.9

47 tests passed, one timing probe ignored; fmt and clippy passed. Reproduced the Notepad path error with the installed app and also through the Notepad Open dialog. Replaced external editor launching with a native JSON editor. On the installed final build, verified current settings display, rejection of trackball_pointer_speed=0 without changing the saved file, successful save and editor close, and preservation of trackball mode, gain 1.25 and stop hold 100ms. Tests cover invalid JSON, unknown fields, out-of-range values and valid custom settings. The precise cause of Notepad path resolution was not established; the built-in editor removes that dependency.

## v0.1.12

54 automated tests passed, one native timer probe ignored. Regression coverage includes CVB/SDF mappings, CVB default and saved SDF loading, physical key ownership across layout changes, exclusion-depth braking at all three edges, physical contacts classified as palms, lift of an operating finger while an excluded contact remains, and reduced glide distance and duration. Formatting and clippy passed. The development PC does not expose contact width/height; braking uses position as requested. Physical game compatibility and the new edge-braking feel require user testing. All 12 prior release bodies and titles were verified to contain English text after translation.

## v0.1.13

55 automated tests passed, one native timer probe ignored. Relative INPUT construction is tested for signed unbounded deltas and fractional accumulation. Packetized coast distance and shallow diagonals match across 2ms and 16ms update schedules. All production pointer movement uses MOUSEEVENTF_MOVE without MOUSEEVENTF_ABSOLUTE. There is no process-name detection or application-specific output mode. Queued movements carry their foreground target, and a focus change cancels ongoing inertia. Windows mouse sensitivity/acceleration may affect desktop movement compared with the previous absolute-output release. After installing this build, the user confirmed that physical touchpad camera movement works in Minecraft Bedrock. CAD interaction remains unverified.

## v0.1.15

57 automated tests passed, one native timer probe ignored; formatting and clippy passed. Removed the Ctrl/Alt bypass from click activation while retaining the Windows-key bypass. Regression coverage checks modifier-first and click-first ordering for left/right drag ownership, modifier pass-through, consumed click-key repeat and release after contact loss. Actual InDesign modified drag behavior still requires physical verification.
