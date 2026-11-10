# Thumbpad

Windows 11 内蔵 Precision Touchpad 向けの、Rust 製 ThumbSense 風プロトタイプです。
ドライバーの追加なしで、タッチパッドへの接触中にホームポジションのキーでクリックできます。
元の ThumbSense とは独立した実装です。

**v0.1.x は実験段階です。複数メーカーの実機検証・エンドツーエンドの遅延測定は未完了です。**

## インストール

[Releases](https://github.com/kathoc/thumbpad/releases) の `thumbpad-setup-x64.exe` を実行します。
Windows 11 x64 が対象です。ARM64 では x64 エミュレーション経由となり、未検証です。
管理者権限は不要で、`%LOCALAPPDATA%\Programs\Thumbpad` にインストールします。
アンインストールは Windows の「インストールされているアプリ」から行えます。
起動時常駐は任意です。設定ファイルはアンインストール後も保持します。

現在の配布ファイルには Authenticode コード署名がありません。

## 操作

| 操作 | 動作 |
| --- | --- |
| タッチパッドに触れながら F / J | 左クリック |
| タッチパッドに触れながら D / K | 右クリック |
| タッチパッドに触れながら C | センタークリック（中ボタン） |
| タッチパッドに触れながら G を保持して上下に動かす | 上下スクロール |
| タッチパッドに触れながら Shift + G を保持して左右に動かす | 左右スクロール（既定は右へ動かすと左スクロール） |
| キーを押し続けて指を動かす | ドラッグ |
| Ctrl / Alt / Win を伴うキー | 通常のショートカットとして通す |
| Ctrl + Alt + F12 | 一時停止 / 再開 |
| トレイアイコンを右クリック | 設定・自動起動・更新確認・終了 |
| トレイアイコンをダブルクリック | 診断画面 |

ウィンドウを閉じても常駐します。完全終了は Exit を選択します。
キーを押した時点で文字かクリックかを決め、指の離脱やキーリピートで途中から切り替えません。
F と J を同時に押しても、両方を離すまで左ボタンを保持します。
F/J/D/K/C/G を保持したまま指を離して置き直せます。保持中は500msの期限を設けず、端からも再開できます。
置き直した最初の位置は基準点として扱い、離す前の位置との差でカーソルやスクロールを飛ばしません。
G を離すとスクロールを終了します。キーを先に押したまま接触しても、文字から操作へ途中では切り替えません。
上下・左右とも既定で反転方向です。G保持中にShiftを押すと左右、離すと上下に切り替わります。
トレイの `Invert vertical scroll` / `Invert horizontal scroll` で方向を個別に変更できます。即時反映して保存され、再起動は不要です。
タッチパッドの「タップでクリック」を併用すると意図しない二重操作になる場合があります。

キーを保持している間のドラッグ・スクロールは、HIDの移動量から入力を生成します。
これによりWindows側のキー入力中のタッチパッド移動抑制に依存しません。OSの感度設定は変更しません。
ドラッグの速度は通常のPrecision Touchpad移動と異なる場合があり、下記の設定で調整できます。

## 端からの誤操作防止

初期値では左右各 12%、上 15% を操作開始の除外領域にします。下端は除外しません。

- 中央側から開始した接触は、端へ移動しても有効なままです。
- 除外領域から開始した接触は、指を離すまで無効です。中央へ滑らせても有効化しません。
- 有効な接触の移動から 500ms 以内は、端での再接触も有効です。瞬断や持ち替えへの猶予です。
- 無効な接触の移動やセンサーの小さな揺れだけでは、猶予を延長しません。
- 有効な接触がなく、無効な端の接触だけがある間は、物理的なカーソル移動も抑制します。
- すでに有効なクリック・スクロール用キーを保持中なら、再接触時の端判定を免除します。

**カーソル抑制は接触状態との対応付けによる実験機能です。** 低レベルマウスフックは
発生元のデバイスハンドルを提供しないため、無効な接触中やキー操作中に外付けマウスも同時に動かすと、
その移動も止まることがあります。HID レポートとマウスイベントの順序次第で、開始時の移動が
一部通る可能性もあります。注入された入力・リモート操作は抑制対象外です。
ドライバーによるデバイス別の完全な遮断ではありません。
`edge_pointer_guard` を `false` にすると、キー変換の端判定だけを残せます。
タップや物理ボタン、スクロール・ズームを遮断する機能ではありません。

マウスフックは専用スレッドで処理し、診断画面・設定処理の待ち時間から切り離しています。
タッチパッドに接触がない間と一時停止中はフック自体を解除します。
診断の `Mouse hook: DETACHED` はマウス入力をフックしていない状態です。

## 設定

トレイまたは診断画面の Settings から `%LOCALAPPDATA%\Thumbpad\settings.json` を開けます。
保存後は終了して再起動してください。

```json
{
  "auto_update": true,
  "edge_side_percent": 12.0,
  "edge_top_percent": 15.0,
  "edge_grace_ms": 500,
  "edge_pointer_guard": true,
  "drag_pixels_per_pad_width": 1600.0,
  "drag_pixels_per_pad_height": 1000.0,
  "scroll_units_per_pad_height": 2400.0,
  "scroll_invert": true,
  "scroll_units_per_pad_width": 2400.0,
  "scroll_horizontal_invert": true
}
```

左右幅は 0–45%、上端幅は 0–80%、猶予は 0–5000ms。
不正な設定は上書きせず通知し、そのセッションでは既定値を使用します。
ドラッグの幅・高さはパッド全体を動かした際の相対移動量、スクロール量は全高に対するホイール単位です（120で1ノッチ）。
移動・スクロール量は1–20000。`scroll_invert` で上下、`scroll_horizontal_invert` で左右を反転できます。
左右スクロール量は `scroll_units_per_pad_width` で調整します。
既存の設定値は保持し、新しい項目の既定値だけをアップグレード時に追加します。
現段階の割り当ては F/J/D/K/C/G と Shift+G 固定です。

## 更新

起動 15 秒後と 24 時間ごとに GitHub Releases の最新の正式版を確認します。
確認はトレイから無効化でき、手動の Check updates も利用できます。
新しいバージョンがあると確認画面を出し、承認後にインストーラーをダウンロードして
置き換え・再起動します。プレリリースとダウングレードは自動更新の対象外です。
GitHub API から取得した SHA-256 digest とファイルサイズを照合し、欠落や不一致なら実行しません。
GitHub アカウント・リポジトリと HTTPS を信頼する方式であり、独立した署名鍵による検証ではありません。
GitHub の API 制限やネットワーク障害時は入力処理を継続します。
失敗の詳細は `%LOCALAPPDATA%\Thumbpad\update-error.txt` に保存します。
更新用ファイルは同じフォルダー内の `updates` に残ります。

外部通信は公開 GitHub API とリリースファイル取得のみです。キー入力内容の記録・送信、
利用統計の送信はありません。診断はカウンターとデバイス情報を表示します。
Rust のパニックが発生した場合は、原因とスタック情報を同フォルダーの `crash.txt` に保存します。自動送信はしません。

## 対応範囲と制約

- Digitizers / Touch Pad (usage page `0x0D`, usage `0x05`) を Raw Input で公開する機種が対象です。
- HID descriptor の Tip Switch、Contact ID、X/Y、Contact Count を解析します。Confidence があれば掌判定を尊重します。
- 固定のバイト配列形式やメーカー名では分岐しません。並列・シリアル・ハイブリッド報告をフレーム単位で処理します。
- 旧式の Synaptics / ELAN 独自 API、PS/2 マウス互換のタッチパッドは未対応です。
- 指を動かしても診断画面の Decoded devices が 0、または HID reports が増えない機種・セッションでは動作しません。
- 管理者権限のアプリ、UAC のセキュアデスクトップ、一部ゲーム等への入力注入は保証しません。
- スリープ・セッション変更・デバイス変更・一時停止・正常終了時は保持ボタンを解除します。
- 強制終了や OS によるフック解除後の完全な復旧は保証しません。一時停止または再起動してください。
- Max hook processing はアプリ内の処理時間であり、指やキーから画面表示までの遅延ではありません。

## 開発

Rust 1.96.0、Visual Studio C++ Build Tools、Windows SDK、Inno Setup 6.3 以降が必要です。
Windows x64 の C ランタイムは静的リンクし、別途 Visual C++ ランタイムの導入を不要にします。

```powershell
cargo test --locked
cargo clippy --locked --all-targets -- -D warnings
cargo run
./installer/build.ps1
# コンパイラーの場所を指定する場合:
./installer/build.ps1 -Iscc 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe'
```

`dist` にインストーラー、単体 EXE、SHA256SUMS.txt ができます。
既存プロセスを正常終了させるには `thumbpad.exe --quit` を使用します。

## リリース手順

Cargo.toml の version と RELEASE_NOTES.md を更新してテストし、`v<version>` タグを push します。
GitHub Actions がテスト・ビルド・インストーラー作成を行い、全ファイルをドラフトにアップロード後に公開します。
`v0.1.0` と `-beta` 等を含むタグはプレリリースになります。
リポジトリを fork する場合は `src/update.rs` の `REPOSITORY` とインストーラー内の URL を変更してください。

## 実機テスト

[TESTING.md](TESTING.md) の手順に従って機種ごとの確認を行ってください。
通常のマウス注入ではタッチパッドの物理接触を再現できないため、実際に指を置いての確認が必要です。

## 参考

- [ThumbSense 原論文](https://www2.sonycsl.co.jp/person/rekimoto/papers/chi03tsense.pdf)
- [Microsoft: Precision Touchpad collection](https://learn.microsoft.com/en-us/windows-hardware/design/component-guidelines/touchpad-windows-precision-touchpad-collection)
- [Microsoft: LowLevelMouseProc](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)
- [Microsoft: SendInput](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)

MIT License
