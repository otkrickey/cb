# CLAUDE.md

cb — macOS向けクリップボードマネージャー（Liquid Glass デザイン）

## プロジェクト概要

macOS標準のクリップボード機能を置き換える高機能クリップボードマネージャー。UIをSwift/SwiftUI + AppKit、ロジック・データ層をRustで構築するハイブリッドアーキテクチャ。

- macOS 26+（Tahoe）対応、Liquid Glass デザイン全面採用
- クリップボード履歴の自動保存・検索・選択ペースト
- テキスト・画像・ファイルパス等の各種データタイプに対応
- メニューバー常駐型 + グローバルショートカット（⌥⌘V）

## プロジェクト構造

```
├── CB/                          # Swiftアプリ（XcodeGenで管理）
│   ├── Sources/                 # Swiftソース
│   │   ├── CBApp.swift          # @main エントリポイント（MenuBarExtra）
│   │   ├── AppDelegate.swift    # ライフサイクル管理
│   │   ├── ClipboardMonitor.swift  # クリップボード変更検知
│   │   ├── HistoryWindowController.swift  # パネルウィンドウ管理
│   │   ├── ShortcutManager.swift   # グローバルホットキー（Carbon Event Manager）
│   │   ├── PasteService.swift      # ペースト操作
│   │   ├── KeychainManager.swift   # DB暗号化キー管理（Keychain）
│   │   ├── Views/
│   │   │   ├── HistoryPanel.swift      # 二ペインレイアウト + 無限スクロール
│   │   │   ├── ClipboardItemRow.swift  # リストアイテム + D&D
│   │   │   ├── SettingsView.swift      # 設定画面（@AppStorage）
│   │   │   └── ShortcutRecorderView.swift  # ショートカットキー設定UI
│   │   └── ViewModels/
│   │       ├── HistoryViewModel.swift  # @Observable VM
│   │       └── SelectionState.swift    # キーボードナビゲーション
│   └── Generated/               # swift-bridge自動生成コード
├── crates/                      # Rustワークスペース
│   └── cb-core/                 # コアロジック（データモデル・ストレージ）
│       ├── src/
│       │   ├── lib.rs           # FFIブリッジ + Storageシングルトン
│       │   ├── models.rs        # ClipboardEntry, ContentType
│       │   └── storage.rs       # SQLite CRUD + FTS5 + 暗号化 + ページネーション + touch_entry（27テスト）
│       └── build.rs             # swift-bridgeコード生成
├── CB/CB.entitlements           # アプリエンタイトルメント
├── project.yml                  # XcodeGen設定（MARKETING_VERSION管理）
├── Cargo.toml                   # ワークスペースルート
├── .github/workflows/           # CI/CD
│   ├── ci.yml                   # PR/pushテスト・ビルド
│   └── release.yml              # タグリリース自動化
├── docs/                        # ドキュメント
└── .claude/skills/              # Claude Codeスキル
```

## ビルド・テスト

```bash
# Rust
cargo build --workspace
cargo test --workspace

# XcodeGenでプロジェクト生成
xcodegen generate

# Xcodeアプリビルド（project.ymlのpre-buildスクリプトがcargo buildを自動実行）
xcodebuild -project CB.xcodeproj -scheme CB build

# Rust静的ライブラリ単体ビルド
cargo build --release -p cb-core
```

## 技術スタック

| 層 | 技術 |
|---|---|
| UI | SwiftUI + AppKit（Liquid Glass / `.glassEffect` / `GlassEffectContainer`） |
| ロジック | Rust |
| データベース | SQLite（rusqlite bundled-sqlcipher、AES-256暗号化） |
| FFI | swift-bridge（複雑な型はJSON文字列、画像は`&[u8]`） |
| ビルド | XcodeGen（`project.yml`）+ Cargo |
| ホットキー | Carbon Event Manager（デフォルト⌥⌘V、カスタマイズ対応） |
| 暗号鍵管理 | macOS Keychain（CryptoKit 256bit対称鍵） |
| 検索 | FTS5全文検索（外部コンテンツテーブル + トリガー同期） |
| CI/CD | GitHub Actions（ci.yml: テスト・ビルド、release.yml: タグリリース） |
| 配布 | GitHub Release（zip） / ソースビルド |

技術選定の詳細: [docs/design/decisions/001-technology-stack.md](docs/design/decisions/001-technology-stack.md)

## アーキテクチャ

### Swift層（UI + 監視）

- **ClipboardMonitor** (`@ObservableObject`): NSPasteboard.changeCountを0.5秒間隔でポーリング。変更検知時にコンテンツ種別判定（PlainText/FilePath/Image）→ Rust FFI経由でデータ保存。コンテンツハッシュで重複スキップ
- **HistoryWindowController**: `KeyablePanel`（NSPanelサブクラス）の生成・表示・非表示を管理。`sendEvent()`でナビゲーションキー（↑↓, Return, Esc）をインターセプト。`previousApp`でペースト先アプリを追跡。`toggle()`はパネル表示中は`ContentTypeFilter`を切り替え（All → Text → Images → Files）
- **HistoryPanel**: GlassEffectContainer + `.glassEffect(.regular, in: .rect(cornerRadius: 16))` による二ペインレイアウト（720x480）。左ペイン：エントリリスト（280pt）、右ペイン：詳細プレビュー（`SelectableTextView`でテキスト選択対応）
- **PasteService** (`@MainActor enum`): NSPasteboardにコンテンツ設定（通常/プレーンテキスト） → パネル非表示 → 前面アプリ復帰 → 0.2秒ディレイ → CGEventで⌘+Vシミュレート
- **ShortcutManager**: Carbon Event Managerでグローバルホットキー登録。UserDefaults監視で動的再登録。デフォルト⌥⌘V
- **SettingsView**: Settings Sceneで表示。保持期間（@AppStorage）、ショートカットキー設定、バージョン情報
- **ShortcutRecorderView**: Record方式のキー入力キャプチャUI。修飾キー必須バリデーション
- **MenuBarExtra**: メニューバー常駐（SF Symbol: `clipboard`）、`LSUIElement = true`、⌘,で設定画面

### Rust層（ロジック + データ）

- **models**: ClipboardEntry, ContentType（PlainText / RichText / Image / FilePath）。`image_data`は`#[serde(skip)]`でJSON除外。`copy_count`（再コピー回数）と`first_copied_at`（初回コピー日時）を保持
- **storage**: SQLite CRUD + FTS5全文検索 + カーソルベースページネーション + 自動クリーンアップ + touch_entry（rusqlite bundled-sqlcipher）。`Mutex<Option<Storage>>`シングルトン。SQLCipherによるAES-256ページレベル暗号化。27テスト
- **ffi**: swift-bridge `#[swift_bridge::bridge]` で12関数を公開（init_storage, migrate_database, save_clipboard_entry, save_clipboard_image, get_recent_entries, delete_entry, get_entry_text, get_entry_image, search_entries, get_entries_before, touch_entry, cleanup_old_entries）
- DBパス: `~/Library/Application Support/CB/clipboard.db`
- 詳細: [docs/design/modules/cb-core.md](docs/design/modules/cb-core.md)

### ロギング

`os.Logger`（subsystem: `com.otkrickey.cb`）でmacOS統合ログに出力。category はファイル単位（`AppDelegate`, `HistoryWindowController`, `ShortcutManager`, `PasteService`, `KeychainManager`）。確認: `log stream --predicate 'subsystem == "com.otkrickey.cb"'`

### レイヤー間の責務分離

| 責務 | 担当 |
|------|------|
| クリップボード変更検知（NSPasteboard） | Swift（ClipboardMonitor） |
| UI表示・ユーザー操作 | Swift（SwiftUI + AppKit） |
| グローバルショートカット | Swift（Carbon Event Manager） |
| データ保存・検索・削除 | Rust（rusqlite） |
| データモデル・ビジネスロジック | Rust |

## 用語

| 用語 | 定義 |
|------|------|
| ClipboardEntry | 1件のクリップボード履歴エントリ（テキスト/画像/ファイルパス） |
| ContentType | エントリの種別（PlainText, RichText, Image, FilePath） |
| Liquid Glass | macOS Tahoeのデザインシステム。半透明ガラス風エフェクト |
| `.glassEffect()` | SwiftUIでLiquid Glassを適用するmodifier |
| `GlassEffectContainer` | Liquid Glassの背景コンテナ |
| swift-bridge | Swift ↔ Rust間のFFI自動生成ツール |
| KeyablePanel | NSPanelサブクラス。キーイベントをインターセプトしてナビゲーション処理 |
| XcodeGen | `project.yml`からXcodeプロジェクトを生成するツール |

## 実装ルール

### Swift-Rust間の型受け渡し

- swift-bridgeの `#[swift_bridge::bridge]` モジュールで定義
- 複雑な構造体はJSON文字列で受け渡し（`serde_json`）
- 画像データは `&[u8]` / `RustVec<UInt8>` で受け渡し
- 生成コードは `CB/Generated/` に出力

### クリップボード監視

- Swift側でNSPasteboard.changeCountをポーリング（0.5秒間隔）
- `skipNextChange`フラグで自前ペーストのセルフループを防止
- コンテンツハッシュで直前と同一内容をスキップ
- コンテンツ取得 → 種別判定 → Rust FFI呼び出し → SQLite保存

### UIデザイン

- `GlassEffectContainer`でSwiftUIコンテンツ全体をラップ
- `.glassEffect(.regular, in: .rect(cornerRadius: 16))` を外枠に適用
- リストアイテムの選択状態は `.fill(.selection)` のシステム選択カラー
- SF Symbolアイコン（`doc.text.fill` / `photo` / `folder.fill` / `textformat`）
- テキストはソリッドレイヤー上に配置（アクセシビリティ）

### コミットメッセージ

形式: `type(scope): 簡潔な説明`

```
feat(monitor): クリップボード変更検知を実装
fix(storage): INSERT時のタイムスタンプ精度を修正
```

## タスク管理ルール

### 基本設定

タスクリストID `cb-tasks` で統一（`.claude/settings.json`で設定済み）。

### サブエージェント起動

必ず `model: "sonnet"` を指定する。

```
Task {
  subagent_type: "general-purpose",
  model: "sonnet",
  prompt: "..."
}
```

## ドキュメントインデックス

| ドキュメント | 内容 |
|-------------|------|
| [docs/archive/initial_plan.md](docs/archive/initial_plan.md) | 初期仕様書 |
| [docs/design/decisions/](docs/design/decisions/) | 技術選定ADR（001: 技術スタック、002: UIデザインシステム、003: DB暗号化） |
| [docs/design/modules/cb-core.md](docs/design/modules/cb-core.md) | cb-core（Rust）モジュール設計 |
| [docs/design/modules/ui.md](docs/design/modules/ui.md) | UI モジュール設計 |
| [docs/design/flows/clipboard_flow.md](docs/design/flows/clipboard_flow.md) | クリップボード監視フロー |
| [docs/design/flows/paste_flow.md](docs/design/flows/paste_flow.md) | ペースト操作フロー |
| [docs/resolved/](docs/resolved/) | 完了済み実装計画（001: MVP、002: 生産性向上、003: 高度な機能） |
| [docs/status/](docs/status/) | 実装ステータス・ロードマップ |
| [docs/usecases/](docs/usecases/) | ユーザー向け操作ガイド |
| [docs/review/](docs/review/) | コードレビューガイドライン |
