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
| `save_clipboard_entry` | `fn(content_type: String, text: String, source_app: String) -> bool` | (旧FFI) テキスト系エントリ保存。Rust 側で閾値判定して自動 externalize |
| `save_clipboard_image` | `fn(image_data: &[u8], source_app: String) -> bool` | (旧FFI) 画像エントリ保存 |
| `save_clipboard_text_v2` | `fn(content_type: String, preview: String, text_content_or_empty: String, blob_sha256_or_empty: String, byte_size: i64, source_app: String) -> bool` | (新FFI) Swift 側で外部化判断・sha256 計算済のテキスト保存。inline は `text_content_or_empty`、外部化は `blob_sha256_or_empty` に |
| `save_clipboard_blob_ref` | `fn(content_type: String, preview: String, blob_sha256: String, byte_size: i64, source_app: String) -> bool` | (新FFI) Swift 側で blob ファイルを書き終えた大サイズエントリの参照だけを DB に登録 |
| `get_recent_entries` | `fn(limit: i32) -> String` | 最新N件をJSONラッパー `{"ok": [...]}` で返却。エラー時は `{"error": "..."}` |
| `delete_entry` | `fn(id: i64) -> bool` | ID指定で削除 |
| `get_entry_text` | `fn(id: i64) -> Option<String>` | テキスト内容取得。外部化エントリは blob から読み出す (失敗時は preview へ fallback) |
| `get_entry_image` | `fn(id: i64) -> Option<Vec<u8>>` | 画像バイト列取得。外部化エントリは blob から読み出す |
| `get_entry_blob_sha256` | `fn(id: i64) -> Option<String>` | 外部化エントリの blob SHA-256 を返却 (inline エントリは `None`) |
| `is_blob_missing` | `fn(id: i64) -> bool` | 外部化エントリの blob ファイルが実在しないか (整合性チェック用) |
| `blob_dir_path` | `fn() -> String` | blob 保管ディレクトリの絶対パス |
| `search_entries` | `fn(query: String, limit: i32) -> String` | FTS5全文検索（前方一致）。JSONラッパー形式 |
| `get_entries_before` | `fn(before_timestamp: i64, limit: i32) -> String` | カーソルベースページネーション（ミリ秒タイムスタンプ）。JSONラッパー形式 |
| `touch_entry` | `fn(id: i64) -> bool` | `created_at`を現在時刻に更新 + `copy_count`をインクリメント |
| `cleanup_old_entries` | `fn(max_age_days: i32) -> i64` | 指定日数より古いエントリを削除。同時に参照が消えた blob ファイルも GC |

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
    pub text_preview: Option<String>,     // 先頭 PREVIEW_BYTES (8KB) の UTF-8 抜粋 (FTS5 索引対象)
    pub text_content: Option<String>,     // 閾値未満のフルテキスト (外部化時は None)
    pub blob_sha256: Option<String>,      // 外部化時のみ設定される blob 参照
    pub byte_size: i64,                   // フル本文/画像のバイト数
    #[serde(skip)]
    pub image_data: Option<Vec<u8>>,      // 常に外部化されるので JSON からも除外
    pub source_app: Option<String>,
    pub created_at: i64,
    pub copy_count: i64,
    pub first_copied_at: i64,
}
```

大サイズコンテンツは inline (`text_content` / `image_data`) から外部 blob ファイル (`~/Library/Application Support/CB/blobs/<sha256>.bin`) に切り出される:

- テキストは `TEXT_EXTERNALIZE_THRESHOLD_BYTES` (256KB) を超えたら外部化。`text_content` は `None` になり、`blob_sha256` が設定される
- 画像は常に外部化 (`save_clipboard_blob_ref` 経由)
- 検索用の `text_preview` は最大 `PREVIEW_BYTES` (8KB) のみ FTS5 に載る → 8KB を超える部分は検索不可
- `byte_size` はフルサイズを保持 (UI のサイズ表示用)
- `image_data`は`#[serde(skip)]`でJSONシリアライズから除外され、`get_entry_image()`で個別取得する設計
- `copy_count`は再コピー回数（初回は1）、`first_copied_at`は最初のコピー日時（`touch_entry`で`created_at`が更新されても保持）
- `created_at`と`first_copied_at`はミリ秒単位のUnixタイムスタンプ

### Storage（`storage.rs`）

| メソッド | 説明 |
|---------|------|
| `Storage::new(db_path, encryption_key)` | DB初期化・暗号化キー設定（`PRAGMA key`）・スキーマ作成。blob 保管ディレクトリは DB と同階層の `blobs/` |
| `Storage::new_with_blob_dir(db_path, encryption_key, blob_dir)` | blob 保管ディレクトリを明示指定して初期化 (テスト用) |
| `Storage::new_in_memory()` | テスト用インメモリDB (blob は一意な一時ディレクトリ) |
| `Storage::migrate_to_encrypted(plain_path, encrypted_path, key)` | `sqlcipher_export`による平文→暗号化DB変換。両パスは空/NUL のみ拒否し、`encrypted_path` を format! で ATTACH 文に埋め込む前に `'` を `''` にエスケープ (`escape_sql_string_literal`)。`encryption_key` は `pragma_update` 経由で設定し format! に埋め込まない (併せて `validate_encryption_key` で空・不正文字を拒否)。macOS の `O'Brien` のような特殊文字を含むユーザディレクトリ配下でも動作する |
| `blob_store()` | 内部の `BlobStore` への参照。外部から blob 保管ルートを取得する用途 |
| `insert_text_entry(content_type, text, source_app)` | テキスト系INSERT。閾値超なら Rust 側で自動 externalize |
| `insert_image_entry(image_data, source_app)` | 画像INSERT。常に blob 外部化 |
| `insert_prepared_text(...)` | Swift 側で preview / text_content / blob_sha256 / byte_size を用意済のテキスト保存 |
| `insert_prepared_blob_ref(...)` | Swift 側で blob 書込済の大サイズエントリの参照だけを DB に登録 |
| `get_recent_entries(limit)` | `created_at DESC, id DESC` で最新N件取得（ソート安定性保証） |
| `delete_entry(id)` | ID指定DELETE |
| `get_entry_text(id)` | フル本文取得。外部化エントリは blob から読み、欠損時は preview へ fallback |
| `get_entry_image(id)` | 画像バイト列取得。外部化エントリは blob から読み出す |
| `get_entry_blob_sha256(id)` | 外部化エントリの blob SHA-256 (inline は None) |
| `is_blob_missing(id)` | 外部化エントリの blob ファイルが実在しないか (整合性チェック) |
| `search_entries(query, limit)` | FTS5 MATCHクエリ（フレーズ前方一致 `"query"*`）。特殊文字 `*` / `^` / `+` を除去しダブルクォートを `""` にエスケープ。boolean 演算子 (AND/OR/NOT/NEAR) は phrase 内では元々演算子として解釈されないため意図的に触らない (通常英文の破壊回避)。空クエリ・サニタイズ後空文字列時は`get_recent_entries`にフォールバック。画像エントリを除外。**索引対象は `text_preview` (最大 8KB) なのでそれを超える本文は検索不可** |
| `get_entries_before(before_timestamp, limit)` | カーソルベースページネーション（ミリ秒タイムスタンプ）。`before_timestamp <= 0`の場合は`get_recent_entries`にフォールバック。`ORDER BY created_at DESC, id DESC` |
| `touch_entry(id)` | `created_at`を現在時刻に更新し`copy_count`をインクリメント。エントリがリスト先頭に移動する |
| `cleanup_old_entries(max_age_days)` | `created_at < (now - max_age_days * 86_400_000)` のエントリをDELETE（ミリ秒単位）+ 参照が消えた blob を GC。削除件数を返却 |
| `list_referenced_blobs()` | DB から参照されている blob SHA-256 の一覧を返す (GC 判定用) |

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
- `migrate_to_encrypted()`で既存の平文DBを`sqlcipher_export`で暗号化DBへ変換。両パスは空/NULのみ拒否し、`encrypted_path`は format! 埋め込み前に `'` を `''` にエスケープ (`escape_sql_string_literal`)。`encryption_key`は`validate_encryption_key`で空・不正文字を拒否した上で`pragma_update`経由で設定 (format!に埋め込まない)。`O'Brien` のようなアポストロフィを含むユーザディレクトリ配下でも動作する
- 暗号化キーはSwift側の`KeychainManager`がmacOS Keychainから取得・管理

### DBスキーマ

```sql
CREATE TABLE IF NOT EXISTS clipboard_entries (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    content_type    TEXT NOT NULL,
    text_preview    TEXT,                 -- 先頭 PREVIEW_BYTES (8KB) の UTF-8 抜粋 (FTS5 索引対象)
    text_content    TEXT,                 -- 閾値未満のフルテキスト (外部化時は NULL)
    image_data      BLOB,                 -- レガシー画像 inline (新規は blob_sha256 経由)
    blob_sha256     TEXT,                 -- 外部化時のみ設定される blob 参照
    byte_size       INTEGER NOT NULL DEFAULT 0,  -- フル本文/画像のバイト数
    source_app      TEXT,
    created_at      INTEGER NOT NULL,     -- ミリ秒単位のUnixタイムスタンプ
    copy_count      INTEGER NOT NULL DEFAULT 1,
    first_copied_at INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_created_at ON clipboard_entries(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_blob_sha256 ON clipboard_entries(blob_sha256) WHERE blob_sha256 IS NOT NULL;

-- FTS5仮想テーブル (text_preview を索引化。8KB を超える本文は検索不可)
CREATE VIRTUAL TABLE clipboard_fts
USING fts5(text_preview, content='clipboard_entries', content_rowid='id');

-- INSERT時の自動同期トリガー
CREATE TRIGGER clipboard_entries_ai
AFTER INSERT ON clipboard_entries BEGIN
    INSERT INTO clipboard_fts(rowid, text_preview) VALUES (new.id, new.text_preview);
END;

-- DELETE時の自動同期トリガー
CREATE TRIGGER clipboard_entries_ad
AFTER DELETE ON clipboard_entries BEGIN
    INSERT INTO clipboard_fts(clipboard_fts, rowid, text_preview)
    VALUES ('delete', old.id, old.text_preview);
END;

-- スキーママイグレーション (各カラムを独立チェックし、未存在の場合のみ追加)
-- ALTER TABLE clipboard_entries ADD COLUMN copy_count / first_copied_at / text_preview / blob_sha256 / byte_size ...;
-- 追加後のバックフィル (未設定行のみ):
-- UPDATE clipboard_entries SET first_copied_at = created_at WHERE first_copied_at = 0;
-- UPDATE clipboard_entries SET text_preview = SUBSTR(text_content, 1, 8192) WHERE text_preview IS NULL AND text_content IS NOT NULL;
-- UPDATE clipboard_entries SET byte_size = COALESCE(LENGTH(text_content), LENGTH(image_data), 0) WHERE byte_size = 0 AND (text_content IS NOT NULL OR image_data IS NOT NULL);

-- タイムスタンプマイグレーション (秒→ミリ秒、冪等)
-- UPDATE clipboard_entries SET created_at = created_at * 1000 WHERE created_at > 0 AND created_at < 10000000000;

-- FTS5 インデックスの再構築 (rebuild_fts で init_schema 内から実行)
-- 旧スキーマ (text_content 索引) を破棄して text_preview 索引で作り直す
INSERT INTO clipboard_fts(clipboard_fts) VALUES ('rebuild');
```

- DBファイル: `~/Library/Application Support/CB/clipboard.db`
- blob 保管ディレクトリ: DB と同階層の `blobs/`

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
| `crates/cb-core/src/storage.rs` | 53個 | Storage CRUD・暗号化・FTS5検索・ページネーション・クリーンアップ・touch_entry・ミリ秒精度ソート・blob 外部化 (dedup/GC/欠損fallback/UTF-8境界)・FTS5サニタイズ (特殊文字/クォート/演算子語含む英文の回帰)・migrate_to_encrypted (E2E/両パス記号許容/再実行/エスケープ/インジェクション防御) |
| `crates/cb-core/src/blob_store.rs` | 7個 | blob 書き込み・読み出し・存在チェック・dedup・GC・削除 |

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

**マイグレーション**（`test_migrate_to_encrypted` / `test_migrate_accepts_valid_path_and_key` / `test_migrate_accepts_plain_and_encrypted_path_with_special_chars` / `test_migrate_rerun_requires_caller_to_remove_target`）:
- `sqlcipher_export`による平文→暗号化DB変換が正しく動作する。E2E で暗号化 DB を鍵付きで開いて読み出しできる
- `plain_path` / `encrypted_path` の両方に `'` `(` `)` 等の記号を含む macOS 上正当なパスもそのまま通す (`O'Brien` ユーザ想定)
- 既存 `encrypted_path` への再実行は残骸を削除してから行うことで通る (SQLCipher の未定義挙動をテストで固定)

**マイグレーション異常系 / インジェクション防御**（`test_migrate_rejects_special_chars_in_key` / `test_migrate_rejects_empty_{plain_path,encrypted_path,key}` / `test_migrate_rejects_nul_in_path` / `test_escape_sql_string_literal_doubles_apostrophes` / `test_migrate_defends_against_encrypted_path_injection`）:
- 空文字 / NUL バイト / 鍵内の非許可文字 (`"abc'; DROP--"` みたいな `'` `;` スペース `-` 混在パターン) を `InvalidParameterName` で拒否する
- `'` は `''` に確実にエスケープされる (unit test)
- `evil'; DROP TABLE ...; --` みたいな SQL 破壊パターンを `encrypted_path` に混ぜても、元 DB の clipboard_entries テーブルが破壊されないことを E2E で担保する

**FTS5検索**（`test_search_entries_basic` / `test_search_entries_prefix_match` / `test_search_entries_empty_query_fallback` / `test_search_entries_delete_sync`）:
- 基本的な全文検索、前方一致（`query*`）、空クエリのフォールバック、DELETE後のFTS同期

**FTS5サニタイズ**（`test_search_special_chars_do_not_break_query` / `test_search_double_quotes_are_escaped` / `test_search_finds_text_containing_{and,or,not,near}`）:
- `*` `^` `+` 混在クエリ・ダブルクォート混在クエリで FTS5 構文エラーにならない
- "salt and pepper" / "cash or credit" / "do not disturb" / "walk near park" のように boolean 演算子相当の英単語を含む実データが、自己再検索でヒットする (回帰防止)

**blob 外部化**（`test_large_text_externalized_to_blob` / `test_small_text_stays_inline` / `test_blob_dedup_between_entries` / `test_cleanup_gc_removes_orphan_blobs` / `test_missing_blob_falls_back_to_preview` / `test_utf8_prefix_respects_char_boundary` / `test_image_always_externalized` / `test_search_ignores_full_text_beyond_preview`）:
- 閾値超過時のみ blob 化、小サイズは inline 維持、SHA-256 dedup、GC、blob 欠損時の preview fallback、UTF-8 境界での安全な切り詰め、preview 外文字列は FTS で検索不可

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
