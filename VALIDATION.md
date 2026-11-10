# Validation record — 2026-11-10

This is a prototype validation record, not a cross-device compatibility claim.

## Automated checks

- `cargo fmt --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: 12 passed, 0 failed.
- Optimized Windows x64 build: passed.
- Inno Setup 6.7.4 installer compilation: passed.

Covered cases: typing/click ownership across repeats, drag after contact loss,
overlapping left-click keys, release on pause, excluded starts, interior-to-edge
motion, 499/501ms reconnection boundaries, no grace from rejected motion,
serial/hybrid frame completion, empty frames, configuration bounds, update version
selection, foreign download URL rejection, missing digest, and corrupted/truncated downloads.

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

## Limitations still requiring physical testing

The host lists an ASUS Precision Touchpad in PnP, but the application has not
observed a physical contact in this session. F/J/K click conversion, physical cursor
suppression, palm rejection, simultaneous external-mouse behavior, and real input
latency have NOT been certified on this host or other devices.

Use TESTING.md for physical verification. The pointer edge guard is contact-correlated
and can also block simultaneous external-mouse movement. It is not a device-isolated
filter driver. No independent Authenticode or update-signing key is configured.
