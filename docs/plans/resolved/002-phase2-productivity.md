# Phase 2: 生産性向上機能

<!--
種別: enhancement
優先度: 高
作成日: 2026-02-16
担当: メインセッション
状態: ✅ 完了
-->

## 概要

Phase 1（MVP）で構築したクリップボードマネージャーに、生産性向上のための6機能を追加する。FTS5全文検索・プレーンテキストペースト・自動クリーンアップ・設定UI・ドラッグ&ドロップ・アクセシビリティ誘導UIを実装し、日常ユースでの実用性を大幅に向上させる。

**背景**:
- 現在の検索はSwift側のクライアントサイドフィルタ（`localizedCaseInsensitiveContains`）で、大量エントリ時にパフォーマンスが劣化する
- 履歴が無制限に蓄積され、ストレージを圧迫する（保存期間は最大7日間に制限したい）
- リッチテキストをプレーンテキストとして貼り付ける手段がない
- 設定変更にはコード修正が必要な状態
- AX権限が未許可時にサイレントスキップしており、ユーザーが気づけない

**目的**:
- FTS5による高速全文検索で大量履歴でも快適に検索
- 7日間の自動クリーンアップでストレージを適正に保つ
- プレーンテキストペースト・D&D等の操作性向上
- 設定UIとAX誘導UIでユーザビリティを完成させる

---

## 現状分析

### 不足している要素

- Rust側にFTS5テーブルが未定義（現在は全件取得→Swift側フィルタ）
- 古いエントリの自動削除ロジックが未実装
- 設定の永続化機構がない（UserDefaults未使用）
- NSPasteboard.setString()でプレーンテキストペーストするパスがない
- NSItemProvider/ドラッグ&ドロップ対応なし
- AXIsProcessTrusted() == false 時のUI誘導なし

### 影響範囲

| 影響対象 | 現状の問題 | 優先度 |
|---------|----------|--------|
| `storage.rs` | FTS5テーブル未定義、クリーンアップ未実装 | 高 |
| `lib.rs` (FFI) | FTS5検索・クリーンアップ用のブリッジ関数なし | 高 |
| `HistoryViewModel` | クライアントサイド検索、大量データに弱い | 高 |
| `PasteService` | プレーンテキストペーストパスなし | 中 |
| `HistoryPanel` | D&D非対応、設定画面なし | 中 |
| `AppDelegate` | AX権限チェック後の誘導UIなし | 中 |

---

## 実装スコープ

### 対応範囲 ✅

- [x] FTS5全文検索（Rust側テーブル作成・検索API・Swift側統合）
- [x] プレーンテキストペースト（Shift+Return で書式なしペースト）
- [x] 自動クリーンアップ（最大7日間保持、アプリ起動時に削除）
- [x] 設定UI（保持期間・ショートカット等の設定画面）
- [x] ドラッグ&ドロップ（履歴リストからのドラッグ対応）
- [x] アクセシビリティ誘導UI（AX権限未許可時のダイアログ表示）

### 対応外 ❌

- ピン留め機能（理由: 今回のスコープ外、Phase 2.5以降で検討）
- iCloud同期（理由: Phase 3）
- CI/CD・GitHub Release自動化（理由: Phase 3）
- スニペット管理（理由: Phase 3）

---

## 設計判断

### 判断1: FTS5テーブルの構成

**問題**: FTS5テーブルをどのように構成するか

**選択肢**:
1. 外部コンテンツテーブル（`content=clipboard_entries`）でメインテーブルと連動
2. 独立したFTS5テーブルにテキストをコピー

**決定**: 選択肢1（外部コンテンツテーブル）

**理由**:
- データの二重管理を防ぐ
- INSERT/DELETE時にトリガーで自動同期可能
- SQLCipher環境でもFTS5は動作する

**トレードオフ**:
- メリット: ストレージ効率が良い、データの一貫性が保たれる
- デメリット: トリガーの実装が必要

### 判断2: 自動クリーンアップのタイミング

**問題**: 古いエントリの削除をいつ実行するか

**選択肢**:
1. アプリ起動時のみ
2. 定期タイマー（1時間ごと等）
3. 新規エントリ保存時に毎回チェック

**決定**: 選択肢1（アプリ起動時のみ）

**理由**:
- ユーザー要件「再起動後にはクリアされている」と合致
- 実装がシンプルで、ランタイムのパフォーマンスに影響しない
- 7日間の保持期間であれば、起動時チェックで十分

**トレードオフ**:
- メリット: シンプル、パフォーマンス影響なし
- デメリット: 長期間起動し続けた場合、7日を超えるエントリが残る可能性（次回起動時に削除される）

### 判断3: プレーンテキストペーストのトリガー

**問題**: プレーンテキストペーストをどのUI操作で発動するか

**選択肢**:
1. Shift+Return で発動
2. 右クリックコンテキストメニュー
3. ボトムバーにトグルボタン

**決定**: 選択肢1（Shift+Return）

**理由**:
- 通常ペースト（Return）との一貫性がある
- キーボードだけで完結する操作フロー
- 他のクリップボードマネージャーでも一般的なパターン

### 判断4: 設定の永続化

**問題**: ユーザー設定の保存先

**選択肢**:
1. UserDefaults
2. Rust側のSQLiteテーブル
3. JSONファイル

**決定**: 選択肢1（UserDefaults）

**理由**:
- macOSネイティブの設定管理機構で最もシンプル
- SwiftUI `@AppStorage` と自然に統合できる
- 設定項目が少数（保持期間・ショートカット程度）なのでDBは過剰

---

## 実装タスク

| タスクID | タスク名 | 説明 | 依存 | 状態 |
|---------|---------|------|------|------|
| 002-01 | FTS5テーブル・検索API（Rust） | FTS5テーブル作成、トリガー、search_entries関数、FFIブリッジ | - | ✅ 完了 |
| 002-02 | FTS5検索のUI統合（Swift） | HistoryViewModelの検索をFTS5に切り替え、デバウンス0.3秒 | 002-01 | ✅ 完了 |
| 002-03 | 自動クリーンアップ（Rust） | cleanup_old_entries関数、FFIブリッジ | - | ✅ 完了 |
| 002-04 | 自動クリーンアップ起動時呼び出し（Swift） | AppDelegate起動時にcleanup_old_entries呼び出し、UserDefaultsから保持期間取得 | 002-03 | ✅ 完了 |
| 002-05 | プレーンテキストペースト | PasteServiceにasPlainTextパラメータ追加、Shift+Returnハンドリング | - | ✅ 完了 |
| 002-06 | 設定UI | Settings Scene、@AppStorage連携、保持期間設定、⌘,対応 | - | ✅ 完了 |
| 002-07 | ドラッグ&ドロップ | ClipboardItemRowに.onDrag()対応（テキスト/画像） | - | ✅ 完了 |
| 002-08 | アクセシビリティ誘導UI | AX権限チェック＋NSAlert誘導ダイアログ＋システム設定遷移 | - | ✅ 完了 |

**状態記号**:
- ⏳ 未着手
- 🏃 進行中
- ✅ 完了
- ⛔ ブロック中

---

## 詳細実装内容

### タスク002-01: FTS5テーブル・検索API（Rust）

**目的**: SQLite FTS5による高速全文検索基盤を構築する

**対象ファイル**:
- `crates/cb-core/src/storage.rs`: 修正（FTS5スキーマ・トリガー・検索関数追加）
- `crates/cb-core/src/lib.rs`: 修正（FFIブリッジ関数追加）

**実装内容**:

1. `init_schema()`にFTS5テーブルとトリガーを追加:
```sql
-- FTS5仮想テーブル（外部コンテンツ）
CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts
USING fts5(text_content, content=clipboard_entries, content_rowid=id);

-- INSERT時の自動同期トリガー
CREATE TRIGGER IF NOT EXISTS clipboard_fts_insert
AFTER INSERT ON clipboard_entries BEGIN
    INSERT INTO clipboard_fts(rowid, text_content) VALUES (new.id, new.text_content);
END;

-- DELETE時の自動同期トリガー
CREATE TRIGGER IF NOT EXISTS clipboard_fts_delete
AFTER DELETE ON clipboard_entries BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, text_content)
    VALUES ('delete', old.id, old.text_content);
END;
```

2. `search_entries(query: &str, limit: i32)` 関数を追加:
```rust
pub fn search_entries(&self, query: &str, limit: i32) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
    // FTS5 MATCH クエリ（前方一致: query*）
    // clipboard_entries JOIN clipboard_fts で結果取得
    // created_at DESC でソート
}
```

3. FFIブリッジ関数を追加:
```rust
fn search_entries(query: String, limit: i32) -> String;
```

4. 既存DBに対するFTS5リビルド処理（マイグレーション）:
```sql
INSERT INTO clipboard_fts(clipboard_fts) VALUES ('rebuild');
```

**テスト**:
- FTS5検索の正常系（部分一致・前方一致）
- 空クエリ時のフォールバック（全件返却）
- INSERT/DELETE後のFTS5同期確認

---

### タスク002-02: FTS5検索のUI統合（Swift）

**目的**: HistoryViewModelの検索をFTS5ベースに切り替える

**対象ファイル**:
- `CB/Sources/ViewModels/HistoryViewModel.swift`: 修正

**実装内容**:

1. `filteredEntries`のsearchText処理を変更:
   - searchTextが空の場合: 従来通り `get_recent_entries()` を使用
   - searchTextが入力されている場合: `search_entries()` FFIを呼び出し
2. 検索はデバウンス（0.3秒）を適用してFFI呼び出し回数を抑制
3. typeFilterとの組み合わせはSwift側で二次フィルタ（FTS5はテキスト検索のみ）

---

### タスク002-03: 自動クリーンアップ（Rust）

**目的**: 7日間を超えた古いエントリを自動削除する

**対象ファイル**:
- `crates/cb-core/src/storage.rs`: 修正
- `crates/cb-core/src/lib.rs`: 修正

**実装内容**:

1. `cleanup_old_entries(max_age_days: i32)` 関数を追加:
```rust
pub fn cleanup_old_entries(&self, max_age_days: i32) -> Result<u64, rusqlite::Error> {
    let cutoff = SystemTime::now()
        .duration_since(UNIX_EPOCH).unwrap()
        .as_secs() as i64 - (max_age_days as i64 * 86400);
    let deleted = self.conn.execute(
        "DELETE FROM clipboard_entries WHERE created_at < ?1",
        params![cutoff],
    )?;
    Ok(deleted as u64)
}
```

2. FFIブリッジ関数を追加:
```rust
fn cleanup_old_entries(max_age_days: i32) -> i64;
```

**テスト**:
- 7日超過エントリが削除されること
- 7日以内エントリが保持されること
- 空DBでエラーにならないこと

---

### タスク002-04: 自動クリーンアップ起動時呼び出し（Swift）

**目的**: アプリ起動時に古いエントリを自動削除する

**対象ファイル**:
- `CB/Sources/AppDelegate.swift`: 修正

**実装内容**:

1. `applicationDidFinishLaunching()`内、`init_storage()`成功後に`cleanup_old_entries(7)`を呼び出し
2. 削除件数をログ出力（`logger.notice`）
3. 将来的に設定UIから保持期間を変更可能にする（UserDefaultsから読み取り）

---

### タスク002-05: プレーンテキストペースト

**目的**: リッチテキストの書式を除去してプレーンテキストとして貼り付ける

**対象ファイル**:
- `CB/Sources/PasteService.swift`: 修正（プレーンテキストモード追加）
- `CB/Sources/HistoryWindowController.swift`: 修正（Shift+Returnハンドリング）
- `CB/Sources/Views/HistoryPanel.swift`: 修正（ボトムバーにShift+Returnインジケータ追加）

**実装内容**:

1. `PasteService.copyToClipboard()`に`asPlainText: Bool`パラメータを追加:
   - `asPlainText == true`の場合、`NSPasteboard.setString()`でプレーンテキストのみ設定
   - 画像エントリの場合はプレーンテキストモード無効（通常ペースト）
2. `KeyablePanel.sendEvent()`でShift+Return（keyCode 36 + shift flag）を検知
3. `HistoryWindowController.handleKey()`にShift+Returnハンドラを追加（`selectAndPaste(asPlainText: true)`）
4. ボトムバーに「⇧⏎ Plain Text」インジケータを追加

---

### タスク002-06: 設定UI

**目的**: ユーザーが保持期間等の設定を変更できるウィンドウを提供する

**対象ファイル**:
- `CB/Sources/Views/SettingsView.swift`: 新規作成
- `CB/Sources/CBApp.swift`: 修正（Settings Sceneまたはメニューから設定を開く）

**実装内容**:

1. `SettingsView`（SwiftUI）:
   - 保持期間スライダー（1〜7日、デフォルト7日）
   - グローバルショートカット表示（⌥⌘V、将来的にカスタマイズ可能に）
   - 「起動時に開く」オプション
   - バージョン情報
2. `@AppStorage`で設定値をUserDefaultsに永続化:
   - `retentionDays`: Int（デフォルト7）
3. メニューバーの「Settings...」メニュー項目から開く
4. macOS標準のSettings Scene（`Settings { SettingsView() }`）を使用

---

### タスク002-07: ドラッグ&ドロップ

**目的**: 履歴リストのアイテムを他のアプリにドラッグ&ドロップできるようにする

**対象ファイル**:
- `CB/Sources/Views/ClipboardItemRow.swift`: 修正

**実装内容**:

1. `ClipboardItemRow`に`.draggable()`モディファイアを追加:
   - テキストエントリ: `String`をTransferable対象として提供
   - 画像エントリ: `NSImage`のData表現を提供
   - ファイルパスエントリ: `URL`として提供
2. ドラッグプレビューはアイテムのプレビューテキスト/サムネイルを表示
3. パネルはドラッグ開始時に閉じない（ユーザーがドラッグ先を選択できるようにする）

---

### タスク002-08: アクセシビリティ誘導UI

**目的**: AX権限が未許可の場合にユーザーをシステム設定へ誘導する

**対象ファイル**:
- `CB/Sources/AppDelegate.swift`: 修正
- `CB/Sources/Views/AccessibilityGuideView.swift`: 新規作成（必要に応じて）

**実装内容**:

1. `applicationDidFinishLaunching()`で`AXIsProcessTrusted()`をチェック
2. 未許可の場合、`NSAlert`ダイアログを表示:
   - タイトル: "アクセシビリティ権限が必要です"
   - メッセージ: "CBがクリップボード内容をペーストするには、アクセシビリティ権限が必要です。"
   - ボタン: "システム設定を開く" / "後で"
3. 「システム設定を開く」押下時:
   - `AXIsProcessTrustedWithOptions`に`kAXTrustedCheckOptionPrompt: true`を渡してシステムダイアログを表示
4. 起動ごとに1回のみチェック（許可済みなら何もしない）

---

## テスト計画

### 新規テスト（Rust）

| テスト種別 | テスト対象 | テスト数 | 内容 |
|-----------|-----------|---------|------|
| ユニット | `storage::search_entries` | 5個 | FTS5検索（部分一致・前方一致・空クエリ・特殊文字・日本語） |
| ユニット | `storage::cleanup_old_entries` | 3個 | 期限超過削除・期限内保持・空DB |

### 既存テストへの影響

- [x] 既存テストの修正が必要（FTS5スキーマ追加によりinit_schema変更）
- [x] 回帰テストの実行が必要（`cargo test --workspace`）

### 手動テスト

| テスト項目 | 確認内容 |
|-----------|---------|
| FTS5検索 | 検索バーにテキスト入力→Rust側FTS5で検索結果が返ること |
| プレーンテキストペースト | Shift+Return→書式なしテキストがペーストされること |
| 自動クリーンアップ | 7日超過エントリがアプリ再起動後に削除されていること |
| 設定UI | メニューから設定画面が開き、保持期間が変更・保存されること |
| ドラッグ&ドロップ | 履歴アイテムをテキストエディタにドラッグ→内容が挿入されること |
| AX誘導 | AX権限なしで起動→ダイアログ表示→システム設定が開くこと |

---

## 成功基準

**受け入れ条件**:
- [x] `cargo test --workspace` で全テストパス（FTS5テスト含む）
- [x] Xcodeビルド・起動が成功
- [x] 検索バーでテキスト入力すると、FTS5による高速検索結果が表示される
- [x] Shift+Returnでプレーンテキストとしてペーストできる
- [x] アプリを再起動すると7日超過のエントリが自動削除されている
- [x] メニューバーから設定画面を開き、保持期間を変更できる
- [x] 履歴アイテムを他のアプリにドラッグ&ドロップできる
- [x] AX権限未許可時にダイアログが表示され、システム設定へ遷移できる

---

## 依存関係

### ブロックするタスク

- Phase 3の全タスク（Phase 2完了が前提）

### ブロックされるタスク

- Phase 1: MVP（✅ 完了済み）

### タスク間の依存

```
002-01 (FTS5 Rust) → 002-02 (FTS5 Swift統合)
002-03 (クリーンアップ Rust) → 002-04 (クリーンアップ Swift)
002-05, 002-06, 002-07, 002-08 は独立して並行実装可能
```

---

## リスクと緩和策

| リスク | 影響度 | 結果 |
|--------|--------|------|
| SQLCipher + FTS5の互換性問題 | 高 | ✅ 問題なし — bundled-sqlcipherでFTS5が正常動作 |
| FTS5リビルドの既存データへの影響 | 中 | ✅ トリガーベースの自動同期で安全に動作 |
| `.draggable()`のNSPanel上での動作 | 中 | ✅ .onDrag()でNSPanel上でも正常動作 |
| Settings Sceneの表示制約 | 低 | ✅ MenuBarExtraアプリでSettings Sceneが正常動作 |

---

## 関連ドキュメント

- [Phase 1: MVP計画](../resolved/001-mvp-initial-setup.md)
- [技術スタック ADR](../design/decisions/001-technology-stack.md)
- [UIデザインシステム ADR](../design/decisions/002-ui-design-system.md)
- [DB暗号化 ADR](../design/decisions/003-database-encryption.md)
- [cb-core モジュール設計](../design/modules/cb-core.md)
- [UI モジュール設計](../design/modules/ui.md)
- [ロードマップ](../status/roadmap.md)
- [実装ステータス](../status/implementation.md)
