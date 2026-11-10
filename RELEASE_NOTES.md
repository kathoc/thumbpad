# Thumbpad v0.1.0 — experimental prototype

Rust 製の Windows 11 / Precision Touchpad 向け ThumbSense 風アプリです。

- 接触中の F/J で左クリック、K で右クリック、長押しでドラッグ
- 左右端・上端からの開始を除外し、中央からの操作は端でも継続
- 最後の有効な移動から 500ms の再接触猶予
- 接触状態に基づくカーソル移動の端抑制（実験機能）
- 常駐トレイ、診断画面、一時停止、任意の自動起動
- GitHub 正式版の自動確認と、承認後の SHA-256 照合・インストール・再起動

`thumbpad-setup-x64.exe` がインストーラー、`thumbpad-x64.exe` が単体版です。
Windows 11 x64 用。追加ドライバー・管理者権限は不要です。配布物は未コード署名です。

**実指による接触操作、機種間互換性、エンドツーエンド遅延は未検証です。**
自動テストとインストーラーの確認結果は [VALIDATION.md](https://github.com/kathoc/thumbpad/blob/main/VALIDATION.md) を参照してください。

端のカーソル抑制中は、同時に使う外付けマウスの移動も止まる場合があります。
`edge_pointer_guard: false` で移動抑制だけを無効化できます。
このプレリリース自体は自動更新の対象外です。インストール済み v0.1.0 は、今後の新しい正式版を検出します。
