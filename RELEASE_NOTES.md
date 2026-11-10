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
