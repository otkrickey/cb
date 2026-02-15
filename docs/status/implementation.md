# 実装ステータス

## モジュール別ステータス

| モジュール | 機能 | ステータス | 備考 |
|-----------|------|-----------|------|
| プロジェクト基盤 | XcodeGen (`project.yml`) + Cargo ワークスペース | `done` | `LSUIElement = true`, pre-buildでcargo自動実行 |
| swift-bridge FFI | Swift ↔ Rust FFIブリッジ | `done` | 12関数公開、JSON文字列でデータ受け渡し |
| データモデル | ClipboardEntry, ContentType | `done` | `#[serde(skip)]`でimage_dataをJSON除外、`copy_count`/`first_copied_at`保持 |
| SQLiteストレージ | rusqlite bundled-sqlcipher、CRUD + FTS5 + ページネーション + touch_entry | `done` | `Mutex<Option<Storage>>`シングルトン、27テスト、スキーママイグレーション対応 |
| クリップボード監視 | NSPasteboard.changeCountポーリング | `done` | 0.5秒間隔、コンテンツハッシュで重複スキップ |
| メニューバーアプリ | MenuBarExtra + AppDelegate + Settings Scene | `done` | SF Symbol `clipboard`、⌘,で設定画面 |
| 履歴パネルUI | Liquid Glass 二ペインレイアウト + 無限スクロール | `done` | GlassEffectContainer、720x480、ページネーション対応 |
| グローバルショートカット | カスタマイズ可能ホットキー（デフォルト⌥⌘V） | `done` | Carbon Event Manager、UserDefaults連動、動的再登録 |
| 選択ペースト | 通常ペースト + プレーンテキストペースト | `done` | Return: 通常、Shift+Return: プレーンテキスト、ペースト時にtouch_entry |
| コピー回数管理 | touch_entryによるcopy_count追跡・リスト先頭移動 | `done` | copy_count, first_copied_at、スキーママイグレーション |
| キーボードナビゲーション | ↑↓選択、Return/Shift+Return確定、Escクローズ | `done` | KeyablePanel.sendEvent()でインターセプト |
| FTS5全文検索 | SQLite FTS5によるサーバーサイド検索 | `done` | 外部コンテンツテーブル、トリガー同期、前方一致、デバウンス0.3秒 |
| 画像対応 | 画像コピー検知・保存・プレビュー・ペースト | `done` | BLOB保存、NSCache（100枚/50MB制限） |
| 日付グループ | Today / Yesterday / 日付ヘッダ | `done` | shouldShowDateHeader / dateHeader |
| DB暗号化 | SQLCipher AES-256ページレベル暗号化 | `done` | bundled-sqlcipher、PRAGMA key |
| Keychain鍵管理 | CryptoKit 256bit対称鍵、macOS Keychain保存 | `done` | iCloud同期無効、端末限定 |
| DBマイグレーション | 平文→暗号化DB自動変換 + スキーマ進化 | `done` | sqlcipher_export、SQLiteヘッダ判定、migrate_add_columns |
| 自動クリーンアップ | アプリ起動時に保持期間超過エントリを削除 | `done` | UserDefaults `retentionDays`（デフォルト7日） |
| 設定UI | 保持期間・ショートカット・バージョン情報 | `done` | Settings Scene、@AppStorage、⌘, |
| ドラッグ&ドロップ | リストからのD&D（テキスト/画像） | `done` | .onDrag() + NSItemProvider |
| AX誘導UI | アクセシビリティ権限未許可時のダイアログ | `done` | NSAlert + AXIsProcessTrustedWithOptions |
| ショートカットカスタマイズ | Record方式のキー入力キャプチャ | `done` | ShortcutRecorderView、UserDefaults永続化 |
| ページネーション | カーソルベース無限スクロール | `done` | get_entries_before、.onAppear検知 |
| 画像キャッシュ最適化 | NSCache（100枚/50MB制限） | `done` | Dictionary → NSCache移行 |
| バージョン管理 | MARKETING_VERSION + エンタイトルメント | `done` | project.yml、CB.entitlements |
| CI/CD | GitHub Actions（テスト・ビルド・リリース） | `done` | ci.yml + release.yml |

## Phase進捗

| Phase | 説明 | ステータス |
|-------|------|-----------|
| Phase 1: MVP | コア機能（監視・保存・UI・ペースト） | `done` |
| Phase 2: 生産性向上 | FTS5検索・設定・D&D・プレーンテキスト・クリーンアップ・AX誘導 | `done` |
| Phase 3: 高度な機能 | ショートカットカスタマイズ・パフォーマンス最適化・CI/CD | `done` |
