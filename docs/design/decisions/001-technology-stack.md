<!--
種別: decisions
対象: 技術スタック選定
作成日: 2026-02-15
更新日: 2026-02-16
担当: AIエージェント
-->

# 技術スタック選定

## 概要

cbプロジェクト（macOSクリップボードマネージャー）の技術スタックを選定する。UIレイヤーをSwift/SwiftUI、ロジック・データ層をRustで構築するハイブリッドアーキテクチャを採用する。

---

## 設計判断

### 判断1: UIフレームワーク — SwiftUI + AppKit

**問題**: macOSネイティブアプリのUIフレームワークをどうするか

**選択肢**:
1. SwiftUI
2. AppKit
3. Electron / Tauri

**決定**: SwiftUI（ビュー層） + AppKit（ウィンドウ管理・システム連携）

**理由**:
- macOS Tahoe の Liquid Glass デザインをネイティブにサポート（`.glassEffect` modifier）
- 宣言的UIにより開発効率が高い
- ウィンドウ管理（フローティングパネル）・メニューバー・グローバルショートカット等のシステム連携にはAppKitが必要

**トレードオフ**:
- **利点**: Liquid Glass対応、宣言的UI、AppKitの全機能にアクセス可能
- **欠点**: macOS 26+ 限定、SwiftUIとAppKitの2層を管理する必要がある

---

### 判断2: バックエンドロジック — Rust

**問題**: 履歴管理・データ処理のロジック層をどの言語で実装するか

**選択肢**:
1. Swift（UIと同一言語）
2. Rust（FFI経由でSwiftから呼び出し）
3. C++（FFI経由）

**決定**: Rust

**理由**:
- メモリ安全性が保証される
- SQLiteとの統合が容易（`rusqlite`クレート、bundled feature）
- クロスプラットフォーム展開の可能性を残せる
- `serde` / `serde_json` による構造化データのシリアライズが容易

**トレードオフ**:
- **利点**: 高性能、メモリ安全、豊富なエコシステム
- **欠点**: Swift-Rust間のFFIレイヤーが必要、ビルドパイプラインの複雑化

---

### 判断3: データ永続化 — SQLite（rusqlite bundled-sqlcipher）

**問題**: クリップボード履歴の永続化にどの技術を使うか

**選択肢**:
1. SQLite（rusqlite）
2. SwiftData
3. Core Data
4. ファイルベース（JSON / MessagePack）

**決定**: SQLite（rusqlite、`bundled-sqlcipher` feature）

**理由**:
- Rustバックエンドから直接操作できる
- `bundled-sqlcipher` featureによりSQLCipherを同梱し、AES-256ページレベル暗号化を透過的に実現
- 軽量・高速・組み込み型データベース
- フレームワーク非依存のためテスタビリティが高い（`new_in_memory()`でテスト可能）

**トレードオフ**:
- **利点**: 高速、軽量、Rustとの相性が良い、テスト容易、透過的暗号化
- **欠点**: SwiftUI側からの直接バインディングなし（JSON経由）、マイグレーション管理が手動、バイナリサイズ約2MB増加

---

### 判断4: Swift-Rust連携 — swift-bridge

**問題**: SwiftとRustの間のFFI（Foreign Function Interface）をどう実現するか

**選択肢**:
1. `swift-bridge`（Rust crate + コード生成）
2. 手動C FFI（`cbindgen` + Cヘッダ）
3. UniFFI（Mozilla製）

**決定**: swift-bridge

**理由**:
- `#[swift_bridge::bridge]`マクロでブリッジ定義、`build.rs`でSwiftコード自動生成
- `String`, `Vec<u8>`, `Option<T>` 等の型が自動変換される
- 生成コードは`CB/Generated/`に出力され、BridgingHeader.hで取り込み

**実装の実態**:
- Rustの`build.rs`が`CB/Generated/cb-core/`にSwiftラッパーとCヘッダを生成
- `CB/Generated/SwiftBridgeCore.swift`にFFI基盤型（`RustString`, `RustVec<T>`等）を生成
- 複雑な構造体（`ClipboardEntry`リスト）はJSON文字列で受け渡し
- 画像データは`&[u8]`スライス / `RustVec<UInt8>`で受け渡し

**トレードオフ**:
- **利点**: 型安全、自動コード生成、ボイラープレート最小限
- **欠点**: 複雑な構造体の直接ブリッジが困難（JSON経由が必要）

---

### 判断5: 配布方法 — GitHub Release + ソースビルド

**問題**: アプリケーションの配布方法をどうするか

**決定**: GitHub Release + ソースビルド

**理由**:
- サンドボックス制約なし（アクセシビリティ権限でCGEventが必要）
- Apple Developer Program不要

---

### 判断6: ビルドシステム — XcodeGen + Cargo

**問題**: Swift + Rustのハイブリッドプロジェクトのビルドをどう管理するか

**選択肢**:
1. 手動Xcodeプロジェクト + Cargo
2. XcodeGen（`project.yml`） + Cargo
3. Swift Package Manager + Cargo

**決定**: XcodeGen（`project.yml`） + Cargo

**理由**:
- `project.yml`でXcodeプロジェクト設定をバージョン管理可能
- `.xcodeproj`の手動編集・コンフリクトを回避
- Pre-build ScriptでConfigurationに応じた`cargo build`（debug/release）を自動実行
- リンカ設定（`-lcb_core`, Security, SystemConfiguration）を宣言的に管理

**トレードオフ**:
- **利点**: 宣言的な設定管理、Git管理が容易、再生成可能
- **欠点**: XcodeGen依存、`xcodegen generate`の実行ステップが必要

---

### 判断7: グローバルショートカット — Carbon Event Manager

**問題**: システム全体で有効なグローバルホットキーをどう実装するか

**選択肢**:
1. Carbon Event Manager（`RegisterEventHotKey`）
2. `CGEvent.tapCreate`（Accessibility API）
3. `NSEvent.addGlobalMonitorForEvents`
4. `MASShortcut`等のサードパーティライブラリ

**決定**: Carbon Event Manager

**理由**:
- `RegisterEventHotKey`はmacOS上で最も安定したグローバルホットキーAPI
- アクセシビリティ権限なしで動作する（ペースト時のみ権限が必要）
- `NSEvent.addGlobalMonitorForEvents`はキーイベントの抑制ができない

**トレードオフ**:
- **利点**: 安定、権限不要、キーコンビネーション登録が簡潔
- **欠点**: Carbonはレガシー API（ただしmacOS 26でも動作）、Swiftからの呼び出しがやや冗長

---

## 技術スタック一覧

| レイヤー | 技術 | 用途 |
|---------|------|------|
| UI | SwiftUI + AppKit | Liquid Glass UI + ウィンドウ管理 |
| ロジック | Rust | データモデル、履歴管理 |
| データベース | SQLite（rusqlite bundled-sqlcipher） | 履歴永続化（AES-256暗号化） |
| FFI | swift-bridge | Swift ↔ Rust 連携（コード生成） |
| シリアライズ | serde + serde_json | FFIデータ受け渡し |
| ビルド | XcodeGen + Cargo | ハイブリッドビルド |
| ホットキー | Carbon Event Manager | グローバルショートカット（⌥⌘V） |
| 配布 | GitHub Release | インストーラー / ソースビルド |

---

## 関連ドキュメント

- [UIデザインシステム ADR](./002-ui-design-system.md)
- [データベース暗号化 ADR](./003-database-encryption.md)
- [初期仕様書](../../archive/initial_plan.md)
