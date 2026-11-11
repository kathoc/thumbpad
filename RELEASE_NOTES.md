# v0.1.11

- Allow pointer motion from a usable finger while other fingers rest in excluded edge-start regions.
- Count usable contacts for native two-finger gesture suppression and select an interior-start finger before edge-start contacts.
- Route mixed edge/interior contact movement through the accepted finger in mouse mode as well.

# v0.1.10

- Filter small trackball pointer jitter without buffering cursor motion.
- Use confirmed motion to launch pointer inertia so a single lift sample cannot trigger it or increase its launch speed.
- Add trackball inertia travel threshold and strength to Edit Settings.

# v0.1.9

- Replace external Notepad launching with a built-in JSON settings editor.
- Validate settings before saving and apply them immediately. Invalid edits keep the editor open and preserve current settings. Cancel discards edits.
- Set a stable working directory for installer-launched instances.

# v0.1.8

- Add independent trackball pointer sensitivity, defaulting to 125%, with persistent faster/slower tray controls.
- Suppress trackball pointer movement and momentum during native two-finger gestures until all fingers lift. Rebase the next single-finger gesture to avoid jumps.
- Preserve existing mouse-mode and scrolling sensitivity, click-key dragging, and stop-hold settings.

# v0.1.7

- Scale pointer and scroll inertia by gesture travel, suppressing tiny gestures and capping at the previous strength.

- Add a tray toggle for the pointer hold after stopping inertia.
- Preserve the hold duration when toggled off; default to 100ms for new settings.
- Apply changes immediately and persist across restarts.

# Thumbpad v0.1.6

- トラックボールモードで指を置いて慣性を止めたとき、300msポインターを固定するよう変更。
- 0.3秒後は指を置いたまま移動を再開できます。固定中の移動は再開時にまとめて反映しません。
- 慣性のない通常接触とマウスモードには、この固定を適用しません。
- settings.jsonのpointer_stop_hold_msで固定時間を変更できます（既定300、0で無効）。

41件の自動テスト成功。v0.1.5の高精度・小数座標による滑らかな慣性移動と、既存のキー割り当て・設定は保持します。
