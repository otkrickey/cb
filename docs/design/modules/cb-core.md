<!--
種別: modules
対象: cb-core（Rustコアクレート）
作成日: 2026-02-16
更新日: 2026-02-16
担当: AIエージェント
-->

# cb-core モジュール設計

## 概要

Rustで実装されたコアライブラリ。データモデル定義、SQLiteストレージ操作、Swift向けFFIブリッジを提供する。

**スコープ**:
- クリップボードエントリのデータモデル（`models`）
- SQLiteによる永続化（`storage`）
- swift-bridgeによるFFI関数（`lib`）

**対象外**:
- クリップボード変更検知（Swift側の責務）
- UI表示ロジック（Swift側の責務）

---

## 責務と境界

**責務**:
- `ClipboardEntry` / `ContentType` の型定義と変換
- SQLiteデータベースの初期化・CRUD操作・FTS5全文検索
- 暗号化DB管理・マイグレーション
- 自動クリーンアップ（古いエントリの削除）
- Swift側へのFFI関数公開

**境界**:
- NSPasteboardの操作には関与しない
- UI表示フォーマットには関与しない（JSONシリアライズまで）

**入力**: 文字列（content_type, text, source_app）、バイトスライス（image_data）
**出力**: bool（成功/失敗）、JSON文字列（`{"ok": [...]}` / `{"error": "..."}`形式のラッパー）、Option型（テキスト/画像データ）

**被依存**:
| 呼び出し元 | 用途 |
|------------|------|
| `AppDelegate`（Swift） | `init_storage()` でDB初期化、`migrate_database()` でマイグレーション、`cleanup_old_entries()` で起動時クリーンアップ |
| `ClipboardMonitor`（Swift） | `save_clipboard_entry()` / `save_clipboard_image()` で保存 |
| `HistoryViewModel`（Swift） | `get_recent_entries()` / `search_entries()` / `get_entries_before()` / `delete_entry()` で取得・検索・削除 |
| `HistoryWindowController`（Swift） | `touch_entry()` でペースト時にコピー回数更新 |
| `PasteService`（Swift） | `get_entry_text()` / `get_entry_image()` でデータ取得 |

---

## 公開API

### FFIブリッジ関数（`lib.rs`）

| 関数 | シグネチャ | 説明 |
|------|-----------|------|
| `init_storage` | `fn(db_path: String, encryption_key: String) -> bool` | Storageシングルトン初期化（暗号化キー付き） |
| `migrate_database` | `fn(plain_path: String, encrypted_path: String, encryption_key: String) -> bool` | 平文DB→暗号化DBマイグレーション |
| `save_clipboard_entry` | `fn(content_type: String, text: String, source_app: String) -> bool` | テキスト系エントリ保存 |
| `save_clipboard_image` | `fn(image_data: &[u8], source_app: String) -> bool` | 画像エントリ保存 |
| `get_recent_entries` | `fn(limit: i32) -> String` | 最新N件をJSONラッパー `{"ok": [...]}` で返却。エラー時は `{"error": "..."}` |
| `delete_entry` | `fn(id: i64) -> bool` | ID指定で削除 |
| `get_entry_text` | `fn(id: i64) -> Option<String>` | テキスト内容取得 |
| `get_entry_image` | `fn(id: i64) -> Option<Vec<u8>>` | 画像バイト列取得 |
| `search_entries` | `fn(query: String, limit: i32) -> String` | FTS5全文検索（前方一致）。JSONラッパー形式 |
| `get_entries_before` | `fn(before_timestamp: i64, limit: i32) -> String` | カーソルベースページネーション（ミリ秒タイムスタンプ）。JSONラッパー形式 |
| `touch_entry` | `fn(id: i64) -> bool` | `created_at`を現在時刻に更新 + `copy_count`をインクリメント |
| `cleanup_old_entries` | `fn(max_age_days: i32) -> i64` | 指定日数より古いエントリを削除 |

### データモデル（`models.rs`）

```rust
// crates/cb-core/src/models.rs
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
    pub copy_count: i64,
    pub first_copied_at: i64,
}
```

`image_data`は`#[serde(skip)]`でJSONシリアライズから除外され、`get_entry_image()`で個別取得する設計。`copy_count`は再コピー回数（初回は1）、`first_copied_at`は最初のコピー日時（`touch_entry`で`created_at`が更新されても保持）。`created_at`と`first_copied_at`はミリ秒単位のUnixタイムスタンプ。

### Storage（`storage.rs`）

| メソッド | 説明 |
|---------|------|
| `Storage::new(db_path, encryption_key)` | DB初期化・暗号化キー設定（`PRAGMA key`）・スキーマ作成 |
| `Storage::new_in_memory()` | テスト用インメモリDB |
| `Storage::migrate_to_encrypted(plain_path, encrypted_path, key)` | `sqlcipher_export`による平文→暗号化DB変換。`encrypted_path`/`encryption_key`に`'`/`\0`が含まれる場合はSQLインジェクション防止のためエラー返却 |
| `insert_text_entry(content_type, text, source_app)` | テキスト系INSERT |
| `insert_image_entry(image_data, source_app)` | 画像INSERT（BLOB） |
| `get_recent_entries(limit)` | `created_at DESC, id DESC` で最新N件取得（ソート安定性保証） |
| `delete_entry(id)` | ID指定DELETE |
| `get_entry_text(id)` | text_contentのみSELECT |
| `get_entry_image(id)` | image_dataのみSELECT |
| `search_entries(query, limit)` | FTS5 MATCHクエリ（フレーズ前方一致 `"query"*`、ダブルクォートエスケープ対応）。空クエリ時は`get_recent_entries`にフォールバック。画像エントリを除外 |
| `get_entries_before(before_timestamp, limit)` | カーソルベースページネーション（ミリ秒タイムスタンプ）。`before_timestamp <= 0`の場合は`get_recent_entries`にフォールバック。`ORDER BY created_at DESC, id DESC` |
| `touch_entry(id)` | `created_at`を現在時刻に更新し`copy_count`をインクリメント。エントリがリスト先頭に移動する |
| `cleanup_old_entries(max_age_days)` | `created_at < (now - max_age_days * 86_400_000)` のエントリをDELETE（ミリ秒単位）。削除件数を返却 |

---

## 内部設計

### Storageシングルトン

```rust
// crates/cb-core/src/lib.rs
static STORAGE: Mutex<Option<Storage>> = Mutex::new(None);
```

`Mutex<Option<Storage>>`でスレッドセーフなシングルトンを実現。`init_storage(db_path, encryption_key)`で暗号化キー付きで初期化し、以後の全FFI関数が`match`式でロックを取得してアクセスする。lock poisoning時は`eprintln!`でエラー出力し`false`/`{"error": "..."}`/`None`/`-1`を返却（パニックしない）。

### 暗号化

`rusqlite`の`bundled-sqlcipher`フィーチャーにより、SQLCipherによるAES-256ページレベル暗号化を実現:
- `Storage::new()`で`PRAGMA key`を設定し、透過的に暗号化/復号
- `encryption_key`が空文字列の場合は暗号化なし（テスト互換）
- `migrate_to_encrypted()`で既存の平文DBを`sqlcipher_export`で暗号化DBへ変換（ATTACH DATABASE文はパラメータ化不可のため、入力値の`'`/`\0`チェックでSQLインジェクションを防止）
- 暗号化キーはSwift側の`KeychainManager`がmacOS Keychainから取得・管理

### DBスキーマ

```sql
CREATE TABLE IF NOT EXISTS clipboard_entries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type    TEXT NOT NULL,
    text_content    TEXT,
    image_data      BLOB,
    source_app      TEXT,
    created_at      INTEGER NOT NULL,   -- ミリ秒単位のUnixタイムスタンプ
    copy_count      INTEGER NOT NULL DEFAULT 1,
    first_copied_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_created_at ON clipboard_entries(created_at DESC);

-- FTS5仮想テーブル（外部コンテンツ）
CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts
USING fts5(text_content, content='clipboard_entries', content_rowid='id');

-- INSERT時の自動同期トリガー
CREATE TRIGGER IF NOT EXISTS clipboard_entries_ai
AFTER INSERT ON clipboard_entries BEGIN
    INSERT INTO clipboard_fts(rowid, text_content) VALUES (new.id, new.text_content);
END;

-- DELETE時の自動同期トリガー
CREATE TRIGGER IF NOT EXISTS clipboard_entries_ad
AFTER DELETE ON clipboard_entries BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, text_content)
    VALUES ('delete', old.id, old.text_content);
END;

-- スキーママイグレーション（各カラムを独立チェックし、未存在の場合のみ追加）
-- ALTER TABLE clipboard_entries ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 1;  -- 独立チェック
-- ALTER TABLE clipboard_entries ADD COLUMN first_copied_at INTEGER NOT NULL DEFAULT 0;  -- 独立チェック
-- UPDATE clipboard_entries SET first_copied_at = created_at WHERE first_copied_at = 0;

-- タイムスタンプマイグレーション（秒→ミリ秒、冪等性あり）
-- UPDATE clipboard_entries SET created_at = created_at * 1000 WHERE created_at > 0 AND created_at < 10000000000;
-- UPDATE clipboard_entries SET first_copied_at = first_copied_at * 1000 WHERE first_copied_at > 0 AND first_copied_at < 10000000000;

-- FTSインデックスのリビルド（メインテーブルとの行数不一致時のみ実行）
-- 毎起動の無条件リビルドを廃止し、大規模DBでの起動遅延を回避
INSERT INTO clipboard_fts(clipboard_fts) VALUES ('rebuild');
```

DBファイル: `~/Library/Application Support/CB/clipboard.db`

---

## エラーハンドリング

| エラー種別 | 発生条件 | 対処 |
|-----------|---------|------|
| DB初期化失敗 | ディレクトリ不在、権限エラー | `init_storage()`が`false`を返却 |
| INSERT失敗 | DB書き込みエラー | `save_clipboard_*`が`false`を返却 |
| 取得失敗 | IDが存在しない | `Option::None`を返却 |
| JSON返却関数のエラー | DBクエリ失敗、Storage未初期化 | `{"error": "..."}` JSONラッパーで返却。Swift側で区別可能 |
| Mutex汚染 | パニックによるlock poisoning | `eprintln!`でログ出力 + `false`/`{"error": "..."}`/`None`/`-1`を返却（パニックしない） |

---

## テスト

### テストファイル

| ファイル | テスト数 | 対象 |
|----------|----------|------|
| `crates/cb-core/src/storage.rs` | 28個 | Storage CRUD・暗号化・FTS5検索・ページネーション・クリーンアップ・touch_entry・ミリ秒精度ソート |

### 重要なテストケース

**正常系**（`test_insert_and_get_text_entry`）:
- テキストエントリの挿入と取得が正しく動作する

**正常系**（`test_insert_and_get_image_entry`）:
- 画像バイト列のINSERTとBLOB取得が正しく動作する

**境界値**（`test_empty_database`）:
- エントリなし状態で`get_recent_entries`が空配列を返す

**異常系**（`test_delete_entry`）:
- エントリ削除後の再削除が`Ok(false)`を返す

**暗号化**（`test_encrypted_db_roundtrip`）:
- 暗号化キー付きDBの書き込みと再オープン読み出しが正しく動作する

**暗号化異常系**（`test_encrypted_db_wrong_key_fails`）:
- 間違った暗号化キーでのDB読み出しが失敗する

**マイグレーション**（`test_migrate_to_encrypted`）:
- `sqlcipher_export`による平文→暗号化DB変換が正しく動作する

**FTS5検索**（`test_search_entries_basic` / `test_search_entries_prefix_match` / `test_search_entries_empty_query_fallback` / `test_search_entries_delete_sync`）:
- 基本的な全文検索、前方一致（`query*`）、空クエリのフォールバック、DELETE後のFTS同期

**クリーンアップ**（`test_cleanup_old_entries` / `test_cleanup_preserves_recent` / `test_cleanup_empty_db`）:
- 古いエントリの削除、最近のエントリの保持、空DBでの安全な動作

**ページネーション**（`test_get_entries_before_*`）:
- カーソルベースのページネーション、before_timestamp=0でのフォールバック、境界値

**touch_entry**（`test_touch_entry` / `test_touch_entry_moves_to_top` / `test_touch_nonexistent_entry` / `test_new_entry_has_copy_count_and_first_copied`）:
- copy_countインクリメント、created_at更新によるリスト先頭移動、存在しないID、新規エントリの初期値検証

---

## 関連ドキュメント

- [技術スタック ADR](../decisions/001-technology-stack.md)
- [データベース暗号化 ADR](../decisions/003-database-encryption.md)
- [クリップボードフロー](../flows/clipboard_flow.md)
