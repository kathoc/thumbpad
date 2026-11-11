# Thumbpad v0.1.13

- Use standard Windows relative mouse movement for all generated pointer motion and trackball inertia.
- Preserve fractional movement and high-resolution inertia updates for smooth low-speed and diagonal motion.
- Allow relative camera movement beyond desktop bounds using the same input format across applications.
- Stop ongoing inertia when the foreground window changes and discard queued movement for another window.
- Show relative output in diagnostics.

## Download

Use `thumbpad-setup-x64.exe` for the Windows 11 x64 installer or `thumbpad-x64.exe` for the portable executable. A compatible built-in Precision Touchpad is required. The binaries are unsigned.

Desktop movement follows Windows mouse sensitivity and acceleration settings. Applications receiving raw relative movement use their own sensitivity. Physical camera controls in Minecraft Bedrock and CAD applications still require in-app verification.
