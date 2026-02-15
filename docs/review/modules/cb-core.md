# cb-core レビューガイドライン

## 対象

`crates/cb-core/` 配下の変更（lib.rs, storage.rs, models.rs）

## 評価項目

1. **FFIブリッジの安全性** — `Mutex<Option<Storage>>` のロック取得が `match` パターンで安全に処理されているか（lock poisoning時は `eprintln!` でログ出力し `false` / `"[]"` / `None` / `-1` を返却、パニックしない）。Storage未初期化時にサイレント失敗を返しているか
2. **SQLCipher暗号化** — `PRAGMA key` が `Storage::new()` で暗号化キー指定時のみ実行されるか。空キーで暗号化なし（テスト互換）が正しく動作するか
3. **FTS5同期の整合性** — INSERT/DELETEトリガーが `clipboard_entries` と `clipboard_fts` を正しく同期しているか。`init_schema()` で `rebuild` が実行されるか
4. **ページネーション境界値** — `get_entries_before` で `before_timestamp <= 0` 時に `get_recent_entries` にフォールバックするか。カーソル値の型（`i64`）とSQLite `INTEGER` の整合性
5. **マイグレーション安全性** — `migrate_to_encrypted` で `sqlcipher_export` によるATTACH/DETACHが正しく行われるか。元DBが破壊されないか。`migrate_add_columns` がべき等（既にカラムが存在する場合はスキップ）に動作するか。`first_copied_at` のバックフィル（`= created_at`）が正しいか
6. **検索クエリの安全性** — `search_entries` の `query*` 前方一致でFTS5構文インジェクション（`"`, `*`, `NEAR` 等）が発生しないか。空クエリ時のフォールバックが正しいか
7. **設計書との整合** — `docs/design/modules/cb-core.md` のFFI関数シグネチャ（12関数）・Storageメソッド・スキーマ定義（`copy_count` / `first_copied_at` カラム含む）と実装が一致するか
8. **テストカバレッジ** — CRUD・暗号化・FTS5・ページネーション・クリーンアップ・touch_entryの各カテゴリに正常系・異常系・境界値テストが存在するか（27テスト）
9. **touch_entryの正確性** — `touch_entry` が `created_at` を現在時刻に更新し `copy_count` をインクリメントするか。`first_copied_at` が変更されず保持されるか。存在しないIDに対して `Ok(false)` を返すか
