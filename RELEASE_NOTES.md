# Thumbpad v0.1.16

- Reset touchpad state, pointer suppression and inertia on suspend/resume and long UI scheduling gaps.
- Re-register touchpad input and rebuild the mouse hook after resume.
- Let physical pointer movement pass when touchpad contact reports stop arriving.
- Bound the pointer output mailbox and discard old queued movements instead of replaying a backlog.
- Ignore stale raw-input messages after a stall. Fresh input is processed immediately.

If input becomes sluggish, Ctrl+Alt+F12 pauses Thumbpad without using the mouse.
Physical sleep/resume behavior still requires verification on affected hardware.

Use `thumbpad-setup-x64.exe` for Windows 11 x64 installation. A compatible built-in Precision Touchpad is required.
