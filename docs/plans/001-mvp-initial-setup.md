# Phase 1: MVP 初回セットアップ & コア機能実装

<!--
種別: enhancement
優先度: 高
作成日: 2026-02-15
更新日: 2026-02-16
担当: メインセッション
状態: ✅ 完了
-->

## 概要

cbプロジェクトのMVP（最小実行可能プロダクト）を構築する。XcodeGen + Rustワークスペースのハイブリッド基盤を整備し、クリップボード監視・履歴保存・Liquid Glass UIによる履歴パネル・選択ペーストの一連の機能を実装する。

**背景**:
- macOS標準のクリップボードは履歴1件のみで使いにくい
- macOS 26.2 / Swift 6.2.3 / Rust 1.92.0 / Xcode 26.2 が利用可能

**目的**:
- クリップボード履歴の自動保存と選択ペーストが動作するMVPを完成させる

---

## 実装スコープ

### 対応範囲 ✅

- [x] XcodeGen + Cargo ハイブリッドプロジェクト構成
- [x] swift-bridge によるSwift ↔ Rust FFI基盤
- [x] Rustデータモデル（ClipboardEntry, ContentType）
- [x] SQLiteストレージ（スキーマ作成、CRUD操作）
- [x] クリップボード変更検知（NSPasteboard polling）
- [x] メニューバー常駐アプリ
- [x] Liquid Glass クリップボード履歴パネル
- [x] グローバルショートカットキー（⌥⌘V）
- [x] 履歴からの選択ペースト
- [x] テキスト・画像の基本対応

### 対応外 ❌

- 全文検索・FTS5（Phase 2）
- ピン留め機能（Phase 2）
- ドラッグ&ドロップ（Phase 2）
- iCloud同期（Phase 3）
- CI/CD・GitHub Release自動化（Phase 3）

---

## 設計判断

### 判断1: クリップボード監視の実装場所

**問題**: NSPasteboardの変更検知をSwift側・Rust側どちらで行うか

**選択肢**:
1. Swift側でNSPasteboard.changeCountをポーリングし、変更時にRustへデータを渡す
2. Rustから`objc`クレート経由でNSPasteboardを直接操作する

**決定**: 選択肢1（Swift側で監視）

**理由**:
- NSPasteboardはAppKit APIであり、Swift/AppKitから自然に扱える
- 監視ロジック自体は軽量（タイマー + changeCount比較のみ）
- Rustは受け取ったデータの処理・保存に専念できる
- `objc`クレートを使ったmacOS API呼び出しは保守コストが高い

**トレードオフ**:
- メリット: 自然なAPI利用、保守性が高い
- デメリット: Swift側にごく一部のロジック（監視タイマー）が残る

### 判断2: アプリ形式

**問題**: メインウィンドウ型かメニューバー常駐型か

**選択肢**:
1. メニューバー常駐型（NSStatusItem）
2. 通常のメインウィンドウ型
3. 両方（メニューバー + 設定ウィンドウ）

**決定**: 選択肢3（メニューバー常駐 + パネルウィンドウ）

**理由**:
- クリップボードマネージャーは常時バックグラウンド動作が必要
- メニューバーアイコンから素早くアクセス
- 履歴パネルはショートカットキーまたはメニューバークリックで表示
- 設定は別ウィンドウで開く（Phase 2）

### 判断3: グローバルホットキーの実装方式

**問題**: システム全体で有効なショートカットキーの実装方法

**選択肢**:
1. CGEvent.tapCreate（イベントタップ）
2. Carbon Event Manager（RegisterEventHotKey）
3. MASShortcut等のサードパーティライブラリ

**決定**: 選択肢2（Carbon Event Manager）

**理由**:
- legacy APIだが安定して動作する
- ⌥⌘V（Opt+Cmd+V）の組み合わせを正確に検知可能
- 外部依存ライブラリが不要

---

## 実装タスク

| タスクID | タスク名 | 説明 | 依存 | 状態 |
|---------|---------|------|------|------|
| 001-01 | プロジェクト基盤構築 | XcodeGen (`project.yml`) + Cargoワークスペース作成 | - | ✅ 完了 |
| 001-02 | swift-bridge FFI基盤 | swift-bridge導入、ブリッジモジュール作成、ビルド連携 | 001-01 | ✅ 完了 |
| 001-03 | Rustデータモデル | ClipboardEntry, ContentType等の型定義 | 001-01 | ✅ 完了 |
| 001-04 | SQLiteストレージ | スキーマ作成、CRUD操作 | 001-03 | ✅ 完了 |
| 001-05 | Rust FFI API定義 | swift-bridgeブリッジ関数の定義・実装（7関数） | 001-02, 001-04 | ✅ 完了 |
| 001-06 | クリップボード監視 | NSPasteboard polling、変更検知、Rustへのデータ送信 | 001-05 | ✅ 完了 |
| 001-07 | メニューバーアプリ | MenuBarExtra、AppDelegate、アプリライフサイクル | 001-01 | ✅ 完了 |
| 001-08 | 履歴パネルUI | Liquid Glass二ペインレイアウト、テキスト/画像プレビュー | 001-05, 001-07 | ✅ 完了 |
| 001-09 | グローバルショートカット | ⌥⌘V でパネル表示/非表示（Carbon Event Manager） | 001-08 | ✅ 完了 |
| 001-10 | 選択ペースト | 履歴アイテム選択 → アクティブアプリへペースト | 001-08 | ✅ 完了 |

---

## 詳細実装内容

### タスク001-01: プロジェクト基盤構築 ✅

**目的**: Swift + Rust ハイブリッドプロジェクトの骨格を作る

**成果物のディレクトリ構成**:
```
cb/
├── CB/                          # Swiftアプリ
│   ├── Sources/                 # Swiftソース
│   │   ├── CBApp.swift          # @main エントリポイント
│   │   └── ...
│   └── Generated/               # swift-bridge自動生成コード
├── crates/                      # Rustワークスペース
│   └── cb-core/                 # コアロジッククレート
│       ├── Cargo.toml
│       ├── build.rs             # swift-bridgeコード生成
│       └── src/
│           ├── lib.rs           # FFIブリッジ + シングルトン
│           ├── models.rs        # データモデル
│           └── storage.rs       # SQLite操作
├── project.yml                  # XcodeGen設定
├── Cargo.toml                   # ワークスペースルート
└── docs/
```

**実装内容**:
1. `project.yml` でmacOS Appプロジェクト「CB」を定義（macOS 26.0, Swift 6.0）
2. `Cargo.toml`（ワークスペースルート）を作成
3. `crates/cb-core/` クレートを作成（`staticlib` + `lib` crate-type）
4. `project.yml` のpre-buildスクリプトでcargo buildを自動実行
5. `LSUIElement = true` でDockアイコン非表示

---

### タスク001-02: swift-bridge FFI基盤 ✅

**目的**: SwiftとRust間の型安全なFFIレイヤーを構築する

**対象ファイル**:
- `crates/cb-core/Cargo.toml`: swift-bridge 0.1 依存
- `crates/cb-core/src/lib.rs`: `#[swift_bridge::bridge]` モジュール
- `crates/cb-core/build.rs`: `CB/Generated/` へコード生成

**実装内容**:
1. `swift-bridge-build`, `swift-bridge` クレートを依存に追加
2. `build.rs` で `CB/Generated/` にSwiftコードを生成
3. 複雑な型はJSON文字列で受け渡し（`serde_json`）
4. 画像データは `&[u8]` / `RustVec<UInt8>` で受け渡し

---

### タスク001-03: Rustデータモデル ✅

**目的**: クリップボードエントリのデータ構造を定義する

**対象ファイル**:
- `crates/cb-core/src/models.rs`

**主要な型**:
```rust
pub enum ContentType {
    PlainText,
    RichText,
    Image,
    FilePath,
}

pub struct ClipboardEntry {
    pub id: i64,
    pub content_type: ContentType,
    pub text_content: Option<String>,
    #[serde(skip)]
    pub image_data: Option<Vec<u8>>,
    pub source_app: Option<String>,
    pub created_at: i64,
}
```

---

### タスク001-04: SQLiteストレージ ✅

**目的**: クリップボード履歴の永続化レイヤーを構築する

**対象ファイル**:
- `crates/cb-core/src/storage.rs`
- `crates/cb-core/Cargo.toml`: rusqlite 0.35（bundled feature）

**スキーマ**:
```sql
CREATE TABLE IF NOT EXISTS clipboard_entries (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type  TEXT NOT NULL,
    text_content  TEXT,
    image_data    BLOB,
    source_app    TEXT,
    created_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_created_at ON clipboard_entries(created_at DESC);
```

**CRUD操作**: insert_text_entry, insert_image_entry, get_recent_entries, delete_entry, get_entry_text, get_entry_image

**テスト**: 13個のユニットテスト（正常系・境界値・異常系）

**DBファイル配置**: `~/Library/Application Support/CB/clipboard.db`

---

### タスク001-05: Rust FFI API定義 ✅

**目的**: Swift側から呼び出すブリッジ関数を定義する

**対象ファイル**:
- `crates/cb-core/src/lib.rs`: ブリッジモジュール

**ブリッジAPI（7関数）**:

| 関数 | シグネチャ | 説明 |
|------|-----------|------|
| `init_storage` | `fn(db_path: String) -> bool` | Storageシングルトン初期化 |
| `save_clipboard_entry` | `fn(content_type: String, text: String, source_app: String) -> bool` | テキスト系エントリ保存 |
| `save_clipboard_image` | `fn(image_data: &[u8], source_app: String) -> bool` | 画像エントリ保存 |
| `get_recent_entries` | `fn(limit: i32) -> String` | 最新N件をJSON配列で返却 |
| `delete_entry` | `fn(id: i64) -> bool` | ID指定で削除 |
| `get_entry_text` | `fn(id: i64) -> Option<String>` | テキスト内容取得 |
| `get_entry_image` | `fn(id: i64) -> Option<Vec<u8>>` | 画像バイト列取得 |

**Storageシングルトン**: `static STORAGE: Mutex<Option<Storage>> = Mutex::new(None);`

---

### タスク001-06: クリップボード監視 ✅

**目的**: システムクリップボードの変更をリアルタイムで検知し、Rustへ保存する

**対象ファイル**:
- `CB/Sources/ClipboardMonitor.swift`

**実装内容**:
1. `@MainActor @ObservableObject` で実装
2. `Timer`で`NSPasteboard.general.changeCount`を定期チェック（0.5秒間隔）
3. `skipNextChange`フラグで自前ペーストのセルフループを防止
4. コンテンツハッシュで直前と同一内容をスキップ
5. コンテンツタイプ判定: `.string` → PlainText/FilePath、`.tiff`/`.png` → Image
6. Rust FFI経由で`save_clipboard_entry()` / `save_clipboard_image()`を呼び出し
7. `latestEntryTimestamp`（`@Published`）でUI更新をトリガー

---

### タスク001-07: メニューバーアプリ ✅

**目的**: メニューバー常駐型アプリの基本構造を作る

**対象ファイル**:
- `CB/Sources/CBApp.swift`: `@main` + `MenuBarExtra`
- `CB/Sources/AppDelegate.swift`: ライフサイクル管理

**実装内容**:
1. `@main`で`MenuBarExtra`を使用したアプリ構成
2. メニューバーアイコン（SF Symbol: `clipboard`）
3. `LSUIElement = true` でDockアイコン非表示
4. `AppDelegate.applicationDidFinishLaunching()`: initStorage → ClipboardMonitor → HistoryWindowController → ShortcutManager

---

### タスク001-08: 履歴パネルUI ✅

**目的**: Liquid Glassデザインのクリップボード履歴パネルを構築する

**対象ファイル**:
- `CB/Sources/Views/HistoryPanel.swift`: 二ペインレイアウト
- `CB/Sources/Views/ClipboardItemRow.swift`: リストアイテム
- `CB/Sources/ViewModels/HistoryViewModel.swift`: `@Observable` VM
- `CB/Sources/ViewModels/SelectionState.swift`: キーボードナビゲーション
- `CB/Sources/HistoryWindowController.swift`: KeyablePanel管理

**実装内容**:
1. `KeyablePanel`（NSPanelサブクラス）: フローティング、ボーダーレス、透明背景
2. `GlassEffectContainer` + `.glassEffect(.regular, in: .rect(cornerRadius: 16))`
3. 二ペインレイアウト（720x480）: 左リスト（280pt）+ 右詳細プレビュー
4. SF Symbolアイコン（`doc.text.fill` / `photo` / `folder.fill` / `textformat`）
5. 選択状態: `.tint.opacity(0.15)` の背景色
6. 日付グループヘッダ（Today / Yesterday / フォーマット済み日付）
7. ボトムバー: アプリ名 + Returnキーインジケータ（`.glassEffect`アクセント）
8. 画像キャッシュ（`[Int64: NSImage]`）

---

### タスク001-09: グローバルショートカット ✅

**目的**: システム全体で有効なショートカットキーを登録する

**対象ファイル**:
- `CB/Sources/ShortcutManager.swift`

**実装内容**:
1. Carbon Event Manager の `RegisterEventHotKey` でグローバルキー登録
2. ⌥⌘V（Opt+Cmd+V）で履歴パネルのトグル表示
3. `_shortcutManagerInstance` グローバル変数経由でコールバック
4. `onTogglePanel`クロージャで`HistoryWindowController.toggle()`を呼び出し

---

### タスク001-10: 選択ペースト ✅

**目的**: 履歴アイテムを選択し、アクティブアプリにペーストする

**対象ファイル**:
- `CB/Sources/PasteService.swift`

**実装内容**:
1. `@MainActor enum`（インスタンス化不可）
2. `copyToClipboard()`: NSPasteboardにコンテンツ設定 + `monitor.skipNextChange = true`
3. テキスト: `NSPasteboard.setString()`、画像: `NSPasteboard.writeObjects([NSImage])`
4. パネル非表示 → `previousApp.activate()` → 0.15秒ディレイ
5. `simulatePaste()`: `AXIsProcessTrusted()`確認 → CGEventで⌘+Vキーストローク送信

---

## テスト計画

### Rustユニットテスト（✅ 13テスト実装済み）

| テスト対象 | テスト内容 |
|-----------|-----------|
| `storage` | テキストエントリINSERT+取得、画像BLOB INSERT+取得、空DB、大量エントリ、削除、存在しないID削除 |

### 手動テスト

| テスト項目 | 確認内容 | 状態 |
|-----------|---------|------|
| クリップボード監視 | テキスト・画像のコピーが自動保存されること | ✅ |
| 履歴パネル | ⌥⌘Vで表示され、履歴が一覧されること | ✅ |
| 選択ペースト | 履歴アイテム選択後、アクティブアプリにペーストされること | ✅ |
| メニューバー | アイコンが表示され、クリックでメニューが開くこと | ✅ |

---

## 成功基準

**受け入れ条件**:
- [x] `cargo build` でRustクレートがビルド成功
- [x] `cargo test` で全テストパス
- [x] Xcodeからのアプリビルド・起動が成功
- [x] テキストをコピーすると自動的に履歴に保存される
- [x] ⌥⌘V で履歴パネルが表示される
- [x] 履歴パネルのUIにLiquid Glassエフェクトが適用されている
- [x] 履歴アイテムをクリックすると、アクティブアプリにペーストされる
- [x] アプリを再起動しても履歴が保持される

---

## リスクと緩和策

| リスク | 影響度 | 結果 |
|--------|--------|------|
| swift-bridgeの互換性問題 | 高 | ✅ 問題なし — swift-bridge 0.1 で安定動作 |
| アクセシビリティ権限の要求 | 中 | ⚠️ `AXIsProcessTrusted() == false` 時はサイレントスキップ（Phase 2で誘導UIを追加予定） |
| グローバルショートカットの競合 | 低 | ✅ ⌥⌘V は一般的に未使用の組み合わせ |

---

## 関連ドキュメント

- [技術スタック ADR](../design/decisions/001-technology-stack.md)
- [UIデザインシステム ADR](../design/decisions/002-ui-design-system.md)
- [cb-core モジュール設計](../design/modules/cb-core.md)
- [UI モジュール設計](../design/modules/ui.md)
- [クリップボード監視フロー](../design/flows/clipboard_flow.md)
- [ペースト操作フロー](../design/flows/paste_flow.md)
- [初期仕様書](../archive/initial_plan.md)
- [Phase 2: 生産性向上計画](./002-phase2-productivity.md)
- [ロードマップ](../status/roadmap.md)
