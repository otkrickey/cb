# レビュー指摘事項の修正

<!--
種別: bugfix
優先度: 高
作成日: 2026-02-16
担当: メインセッション
状態: ✅ 完了
-->

## 概要

コードレビューで検出された High（潜在的バグ・信頼性の問題）指摘事項を修正する。Rust Core のタイムスタンプ精度・FFI エラーハンドリング・マイグレーション冪等性、Swift UI の画像非同期ロード・deleteEntry 楽観的削除、ビルド設定の Info.plist 不整合・CI/Release 改善を対応する。

**背景**:
- PR 前レビューで Rust Core 4件、Swift UI 28件（うち実害ありと判断されたもの）、設定・ビルド 5件の指摘があった
- 調査の結果、7件は誤検出（問題なし）、残りに対して修正方針をユーザーと合意済み

**目的**:
- レビュー指摘のバグ・信頼性問題をすべて解消する
- タイムスタンプをミリ秒化し、ソート安定性を保証する
- FFI エラー情報を Swift 側に伝搬できるようにする
- 画像ロードの非同期化で UI ブロックを排除する
- CI/Release パイプラインの堅牢性とリリース品質を向上させる

---

## 現状分析

### 不足している要素

| 影響対象 | 現状の問題 | 優先度 |
|---------|----------|--------|
| `storage.rs` タイムスタンプ | 秒単位。同一秒内の複数エントリでソート不安定 | 中 |
| `lib.rs` FFI エラー | 空配列 `[]` とエラーが区別不能。Swift 側でエラー検知不可 | 高 |
| `storage.rs` マイグレーション | `copy_count` の存在のみチェック。部分失敗時に `first_copied_at` 欠損 | 低 |
| `HistoryViewModel` deleteEntry | FFI 失敗時もローカル配列から削除。次回ロードでエントリ復活 | 中 |
| `HistoryViewModel` loadImage | メインスレッド同期ロード。初回表示時に UI ブロック | 中 |
| `Info.plist` バージョン | `0.1.0` ハードコード vs `project.yml` の `1.0.0` | 高 |
| `Info.plist` UsageDescription | `NSAppleEventsUsageDescription` 未設定 | 中 |
| `ci.yml` コード署名 | 環境変数未定義時にビルド失敗の可能性 | 中 |
| `release.yml` | notarization 未実装、アーキテクチャ指定なし | 中 |

---

## 設計判断

### 判断1: タイムスタンプ精度の変更方法

**問題**: 秒単位からミリ秒単位への移行時、既存データのマイグレーションが必要

**決定**: `as_secs()` → `as_millis()` に変更し、マイグレーションで既存値を ×1000

**理由**:
- ミリ秒精度で同一タイムスタンプの可能性がほぼゼロになる
- マイグレーションは単純な算術演算で安全に実行可能
- Swift 側は `TimeInterval(created_at) / 1000.0` に変更するだけ

### 判断2: FFI エラー伝搬方式

**問題**: 現在 `String`（JSON）を返す FFI 関数でエラーと空結果が区別不能

**決定**: JSON ラッパー方式 `{"ok": [...]}` / `{"error": "message"}` を採用

**理由**:
- swift-bridge の型制約内で実装可能（String 戻り値を維持）
- Swift 側の変更が最小限（JSON パースの分岐追加のみ）
- 既存の `bool` 戻り値関数（save, delete, touch）は変更不要

### 判断3: 画像非同期ロード方式

**問題**: `loadImage(for:)` がメインスレッドで同期的に FFI を呼び出す

**決定**: `Task.detached` で非同期ロードし、キャッシュヒット時のみ同期返却

**理由**:
- キャッシュヒット時はオーバーヘッドなし（現状と同じ）
- キャッシュミス時のみ非同期化し、ローディング表示を挟む
- `@Observable` の変更通知で UI が自動更新される

---

## 実装スコープ

### 対応範囲 ✅

- [ ] タイムスタンプをミリ秒に変更（スキーマ + マイグレーション + Rust/Swift 両方）
- [ ] FFI エラー情報の JSON ラッパー化（Rust lib.rs + Swift パース）
- [ ] マイグレーション冪等性の修正（各カラム独立チェック）
- [ ] deleteEntry のエラーハンドリング追加
- [ ] 画像ロードの非同期化（Task.detached + プレースホルダー）
- [ ] Info.plist バージョン不整合修正（`$(MARKETING_VERSION)` 参照化）
- [ ] NSAppleEventsUsageDescription 追加
- [ ] CI コード署名環境変数のデフォルト設定
- [ ] Release ワークフローの notarization 対応
- [ ] Release ワークフローのユニバーサルバイナリ対応

### 対応外 ❌

- FTS5 の NULL ハンドリング最適化（理由: 実害なし、search_entries で Image 除外済み）
- previousApp 追跡のエッジケース（理由: hidesOnDeactivate で実質発生しない）
- RichText コンテンツタイプの実装（理由: 別スコープ）

---

## 実装タスク

| タスクID | タスク名 | 説明 | 担当 | 依存 | 見積もり | 状態 |
|---------|---------|------|------|------|---------|------|
| 001-01 | タイムスタンプミリ秒化 | storage.rs のタイムスタンプを秒→ミリ秒に変更、マイグレーション追加 | - | - | 2時間 | ⏳ 未着手 |
| 001-02 | FFI エラーハンドリング改善 | lib.rs の JSON 戻り値にエラー情報を含める | - | - | 1.5時間 | ⏳ 未着手 |
| 001-03 | マイグレーション冪等性修正 | migrate_add_columns の各カラム独立チェック化 | - | - | 0.5時間 | ⏳ 未着手 |
| 001-04 | Swift FFI パース適応 | JSON ラッパーのパース対応 + deleteEntry エラーハンドリング | - | 001-02 | 1.5時間 | ⏳ 未着手 |
| 001-05 | Swift タイムスタンプ適応 | ClipboardEntryModel のミリ秒対応 | - | 001-01 | 0.5時間 | ⏳ 未着手 |
| 001-06 | 画像非同期ロード | HistoryViewModel.loadImage を非同期化 | - | - | 2時間 | ⏳ 未着手 |
| 001-07 | Info.plist 修正 | バージョン参照化 + NSAppleEventsUsageDescription 追加 | - | - | 0.5時間 | ⏳ 未着手 |
| 001-08 | CI/Release 改善 | コード署名デフォルト + notarization + ユニバーサルバイナリ | - | - | 2時間 | ⏳ 未着手 |

### 並列実行可能なグループ

- **グループA**: 001-01, 001-02, 001-03（Rust Core、独立）
- **グループB**: 001-06, 001-07, 001-08（Swift UI/ビルド、独立）
- **グループC**: 001-04（001-02 完了後）, 001-05（001-01 完了後）

---

## 詳細実装内容

### タスク 001-01: タイムスタンプミリ秒化

**対象ファイル**:
- `crates/cb-core/src/storage.rs`: 修正
- `CB/Sources/ViewModels/HistoryViewModel.swift`: 修正（001-05 で対応）

**実装内容**:
1. `init_schema()` にマイグレーション追加: `UPDATE clipboard_entries SET created_at = created_at * 1000, first_copied_at = first_copied_at * 1000 WHERE created_at < 10000000000`（秒単位の値を判定）
2. `insert_text_entry`, `insert_image_entry`, `touch_entry`, `cleanup_old_entries` の `.as_secs()` を `.as_millis()` に変更
3. `cleanup_old_entries` の cutoff 計算を `max_age_days as i64 * 86_400_000` に変更
4. テストの `sleep` 時間を調整（1100ms → 10ms で十分に）
5. `ORDER BY created_at DESC, id DESC` に変更してソート安定性を保証

### タスク 001-02: FFI エラーハンドリング改善

**対象ファイル**:
- `crates/cb-core/src/lib.rs`: 修正

**実装内容**:
1. ヘルパー関数 `json_ok(entries)` → `{"ok": [...]}` と `json_error(msg)` → `{"error": "..."}` を追加
2. `get_recent_entries`, `search_entries`, `get_entries_before` の戻り値を JSON ラッパーに変更
3. Storage 未初期化時は `{"error": "Storage not initialized"}` を返す
4. クエリエラー時は `{"error": "Failed to get entries: {e}"}` を返す

### タスク 001-03: マイグレーション冪等性修正

**対象ファイル**:
- `crates/cb-core/src/storage.rs`: 修正

**実装内容**:
1. `copy_count` と `first_copied_at` を独立してチェック
2. 各 ALTER TABLE を個別に実行（execute_batch ではなく個別 execute）
3. UPDATE は `first_copied_at` カラムが存在するか確認してから実行

### タスク 001-04: Swift FFI パース適応

**対象ファイル**:
- `CB/Sources/ViewModels/HistoryViewModel.swift`: 修正

**実装内容**:
1. FFI JSON レスポンスのパース用 `FFIResponse<T: Decodable>` 構造体を追加（`ok: T?`, `error: String?`）
2. `loadEntries()`, `performSearch()`, `loadMoreEntries()` のパースを更新
3. エラー時は `os.Logger` でログ出力
4. `deleteEntry()` で `delete_entry()` の返り値をチェックし、失敗時はローカル配列を変更しない

### タスク 001-05: Swift タイムスタンプ適応

**対象ファイル**:
- `CB/Sources/ViewModels/HistoryViewModel.swift`: 修正

**実装内容**:
1. `ClipboardEntryModel.createdAt` を `Date(timeIntervalSince1970: TimeInterval(created_at) / 1000.0)` に変更
2. `firstCopiedAt` も同様に変更
3. `loadMoreEntries()` の `lastTimestamp` は生の `created_at`（ミリ秒）をそのまま使用（Rust 側の比較もミリ秒）

### タスク 001-06: 画像非同期ロード

**対象ファイル**:
- `CB/Sources/ViewModels/HistoryViewModel.swift`: 修正
- `CB/Sources/Views/HistoryPanel.swift`: 修正

**実装内容**:
1. `loadImage(for:)` をキャッシュヒット時は同期返却、ミス時は `nil` 返却 + バックグラウンドロード起動に変更
2. バックグラウンドロード完了時に `@Observable` の変更通知で UI 更新
3. `loadingImageIds: Set<Int64>` でロード中 ID を管理し、重複ロード防止
4. HistoryPanel の画像表示部分にローディングプレースホルダー（`ProgressView`）を追加

### タスク 001-07: Info.plist 修正

**対象ファイル**:
- `CB/Resources/Info.plist`: 修正

**実装内容**:
1. `CFBundleShortVersionString` を `$(MARKETING_VERSION)` に変更
2. `CFBundleVersion` を `$(CURRENT_PROJECT_VERSION)` に変更
3. `NSAppleEventsUsageDescription` を追加: 「CB はクリップボード内容を他のアプリにペーストするために Apple Events を使用します。」

### タスク 001-08: CI/Release 改善

**対象ファイル**:
- `.github/workflows/ci.yml`: 修正
- `.github/workflows/release.yml`: 修正
- `project.yml`: 修正

**実装内容**:
1. `ci.yml`: `xcodebuild` に `CB_CODE_SIGNING_ALLOWED=NO` 環境変数を追加
2. `project.yml`: コード署名設定にデフォルト値を追加（`CB_CODE_SIGNING_ALLOWED` 未定義時は `NO`）
3. `release.yml`: Rust ビルドに `--target aarch64-apple-darwin --target x86_64-apple-darwin` を追加
4. `release.yml`: `lipo` で Universal Binary 作成ステップを追加
5. `release.yml`: notarization ステップを追加（`xcrun notarytool`、シークレット参照）
6. `release.yml`: アーキテクチャ情報をリリースノートに記載

---

## テスト計画

### 新規テスト

| テスト種別 | テストファイル | テスト数 | 対象 |
|-----------|--------------|---------|------|
| ユニット | `crates/cb-core/src/storage.rs` | 3個 | ミリ秒タイムスタンプのソート・マイグレーション・境界値 |
| ユニット | `crates/cb-core/src/lib.rs` | 2個 | FFI JSON ラッパーのエラー・正常系 |

### 既存テストへの影響

- [x] `storage.rs` の既存 27 テストをミリ秒対応に修正が必要（sleep 時間・アサーション値）
- [x] `test_ordering` の sleep を 1100ms → 10ms に短縮可能
- [x] `test_get_entries_before_boundary` のタイムスタンプ値をミリ秒に変更

---

## 成功基準

**受け入れ条件**:
- [ ] `cargo test --workspace` が全テストパス
- [ ] `xcodebuild -project CB.xcodeproj -scheme CB build` が成功
- [ ] タイムスタンプがミリ秒精度で保存される
- [ ] 既存 DB（秒単位）からのマイグレーションが正常動作する
- [ ] FFI エラー時に Swift 側でエラーメッセージが取得可能
- [ ] 画像表示時に UI がブロックされない
- [ ] Info.plist のバージョンが project.yml の MARKETING_VERSION と一致する
- [ ] CI ワークフローがコード署名なしで正常ビルド
- [ ] Release ワークフローが Universal Binary を生成

---

## リスクと緩和策

| リスク | 影響度 | 緩和策 |
|--------|--------|--------|
| ミリ秒マイグレーションで既存データ破損 | 高 | `WHERE created_at < 10000000000` で秒単位の値のみ変換。冪等性あり |
| FFI JSON ラッパー変更で Swift パースが壊れる | 高 | Rust と Swift を同一タスクグループで変更、テストで検証 |
| notarization に Apple Developer 資格情報が必要 | 中 | GitHub Secrets に設定手順をドキュメント化。Secrets 未設定時はスキップ |

---

## 関連ドキュメント

- [モジュール設計: cb-core](../design/modules/cb-core.md)
- [モジュール設計: UI](../design/modules/ui.md)
- [ADR: 技術スタック](../design/decisions/001-technology-stack.md)
- [ADR: DB暗号化](../design/decisions/003-database-encryption.md)
