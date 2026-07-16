use rusqlite::{Connection, params};
use std::path::{Path, PathBuf};
use crate::blob_store::BlobStore;
use crate::models::{ClipboardEntry, ContentType, PREVIEW_BYTES};

/// テキストコンテンツを DB inline から外部 blob に切り替える閾値（バイト）。
/// これを超えたら sha256 名で blob ファイルに書き出し、DB には参照のみ残す。
///
/// 変更しても既存データは壊れない: read パスは `blob_sha256 IS NOT NULL`
/// でdispatchしており、サイズを見ないため。
pub const TEXT_EXTERNALIZE_THRESHOLD_BYTES: usize = 262_144;

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    Io(std::io::Error),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::Sqlite(e) => write!(f, "sqlite: {e}"),
            StorageError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<rusqlite::Error> for StorageError {
    fn from(e: rusqlite::Error) -> Self { StorageError::Sqlite(e) }
}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self { StorageError::Io(e) }
}

pub type StorageResult<T> = Result<T, StorageError>;

pub struct Storage {
    conn: Connection,
    blob_store: BlobStore,
}

impl Storage {
    /// 通常起動用。DB パス親ディレクトリの `blobs/` を blob 保管所として使う。
    /// 例: db_path = `~/.../CB/clipboard.db` → blobs = `~/.../CB/blobs/`
    pub fn new(db_path: &str, encryption_key: Option<&str>) -> StorageResult<Self> {
        let blob_dir = default_blob_dir(Path::new(db_path));
        Self::new_with_blob_dir(db_path, encryption_key, blob_dir)
    }

    pub fn new_with_blob_dir(
        db_path: &str,
        encryption_key: Option<&str>,
        blob_dir: PathBuf,
    ) -> StorageResult<Self> {
        let conn = Connection::open(db_path)?;
        if let Some(key) = encryption_key {
            // migrate_to_encrypted と同じホワイトリストで検証し、
            // Storage::new と migrate_to_encrypted の間で受理条件を揃える。
            // (PR #15 review 指摘: 同じ鍵が片方で通り片方で弾かれる非対称の解消)
            Self::validate_encryption_key(key)?;
            conn.pragma_update(None, "key", key)?;
        }
        let blob_store = BlobStore::new(blob_dir)?;
        let storage = Storage { conn, blob_store };
        storage.init_schema()?;
        Ok(storage)
    }

    /// テスト用: DB は :memory:、blob は一意な一時ディレクトリに書き出す。
    pub fn new_in_memory() -> StorageResult<Self> {
        let conn = Connection::open_in_memory()?;
        let blob_dir = tmp_blob_dir();
        let blob_store = BlobStore::new(blob_dir)?;
        let storage = Storage { conn, blob_store };
        storage.init_schema()?;
        Ok(storage)
    }

    pub fn blob_store(&self) -> &BlobStore { &self.blob_store }

    fn init_schema(&self) -> StorageResult<()> {
        // 新規作成は最初から新スキーマで。既存 DB は下の migrate_* が拡張する。
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS clipboard_entries (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type    TEXT NOT NULL,
                text_preview    TEXT,
                text_content    TEXT,
                image_data      BLOB,
                blob_sha256     TEXT,
                byte_size       INTEGER NOT NULL DEFAULT 0,
                source_app      TEXT,
                created_at      INTEGER NOT NULL,
                copy_count      INTEGER NOT NULL DEFAULT 1,
                first_copied_at INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_created_at
            ON clipboard_entries(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_blob_sha256
            ON clipboard_entries(blob_sha256) WHERE blob_sha256 IS NOT NULL;"
        )?;

        // 既存テーブルへのカラム追加（idempotent）
        self.migrate_add_columns()?;

        // タイムスタンプ秒→ミリ秒
        self.conn.execute_batch(
            "UPDATE clipboard_entries SET created_at = created_at * 1000 WHERE created_at > 0 AND created_at < 10000000000;
             UPDATE clipboard_entries SET first_copied_at = first_copied_at * 1000 WHERE first_copied_at > 0 AND first_copied_at < 10000000000;"
        )?;

        // FTS5 は text_preview を索引化する形に統一。旧 FTS(text_content 索引)は
        // ここでリセットして作り直す。
        self.rebuild_fts()?;

        Ok(())
    }

    fn migrate_add_columns(&self) -> StorageResult<()> {
        for (col, ddl) in [
            ("copy_count",      "ALTER TABLE clipboard_entries ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 1"),
            ("first_copied_at", "ALTER TABLE clipboard_entries ADD COLUMN first_copied_at INTEGER NOT NULL DEFAULT 0"),
            ("text_preview",    "ALTER TABLE clipboard_entries ADD COLUMN text_preview TEXT"),
            ("blob_sha256",     "ALTER TABLE clipboard_entries ADD COLUMN blob_sha256 TEXT"),
            ("byte_size",       "ALTER TABLE clipboard_entries ADD COLUMN byte_size INTEGER NOT NULL DEFAULT 0"),
        ] {
            if !self.column_exists("clipboard_entries", col)? {
                self.conn.execute_batch(&format!("{ddl};"))?;
            }
        }

        // first_copied_at のバックフィル (旧 code そのまま踏襲)
        self.conn.execute_batch(
            "UPDATE clipboard_entries SET first_copied_at = created_at WHERE first_copied_at = 0;"
        )?;

        // text_preview / byte_size のバックフィル (未設定の行のみ)
        let preview_limit = PREVIEW_BYTES as i64;
        self.conn.execute(
            "UPDATE clipboard_entries
                SET text_preview = SUBSTR(text_content, 1, ?1)
              WHERE text_preview IS NULL AND text_content IS NOT NULL",
            params![preview_limit],
        )?;
        self.conn.execute_batch(
            "UPDATE clipboard_entries
                SET byte_size = COALESCE(LENGTH(text_content), LENGTH(image_data), 0)
              WHERE byte_size = 0 AND (text_content IS NOT NULL OR image_data IS NOT NULL);"
        )?;

        Ok(())
    }

    fn column_exists(&self, table: &str, column: &str) -> StorageResult<bool> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
        for name in rows {
            if name? == column { return Ok(true); }
        }
        Ok(false)
    }

    /// FTS5 スキーマのバージョン。旧 (text_content 索引) からのマイグレーションが済んで
    /// text_preview 索引になっていれば FTS_SCHEMA_VERSION と一致する。
    const FTS_SCHEMA_VERSION: i32 = 2;

    /// FTS5 を text_preview 索引で構築する。
    ///
    /// PRAGMA user_version で「text_preview 索引に移行済みか」を判定し、
    /// 既に移行済みならスキップする。以前は init_schema() のたびに無条件で
    /// DROP + rebuild していたので、大規模履歴のユーザは毎起動で全件再構築の
    /// コストを踏んでいた (PR #17 review 指摘)。
    fn rebuild_fts(&self) -> StorageResult<()> {
        let version: i32 = self
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version >= Self::FTS_SCHEMA_VERSION {
            return Ok(());
        }
        // 既存トリガー/仮想テーブルをまず削除 (旧スキーマの text_content 索引を廃棄)
        self.conn.execute_batch(
            "DROP TRIGGER IF EXISTS clipboard_entries_ai;
             DROP TRIGGER IF EXISTS clipboard_entries_ad;
             DROP TRIGGER IF EXISTS clipboard_entries_au;
             DROP TABLE IF EXISTS clipboard_fts;

             CREATE VIRTUAL TABLE clipboard_fts
             USING fts5(text_preview, content='clipboard_entries', content_rowid='id');

             CREATE TRIGGER clipboard_entries_ai
             AFTER INSERT ON clipboard_entries
             BEGIN
                 INSERT INTO clipboard_fts(rowid, text_preview)
                 VALUES (new.id, new.text_preview);
             END;

             CREATE TRIGGER clipboard_entries_ad
             AFTER DELETE ON clipboard_entries
             BEGIN
                 INSERT INTO clipboard_fts(clipboard_fts, rowid, text_preview)
                 VALUES ('delete', old.id, old.text_preview);
             END;

             INSERT INTO clipboard_fts(clipboard_fts) VALUES ('rebuild');",
        )?;
        self.conn
            .pragma_update(None, "user_version", Self::FTS_SCHEMA_VERSION)?;
        Ok(())
    }

    /// テキストエントリを保存する。閾値を超えるフルテキストは外部 blob に切り出す。
    pub fn insert_text_entry(
        &self,
        content_type: &ContentType,
        text: &str,
        source_app: &str,
    ) -> StorageResult<i64> {
        let byte_size = text.len() as i64;
        let preview = utf8_prefix(text, PREVIEW_BYTES);
        let (text_content, blob_sha256) = if text.len() > TEXT_EXTERNALIZE_THRESHOLD_BYTES {
            let sha = self.blob_store.write(text.as_bytes())?;
            (None, Some(sha))
        } else {
            (Some(text.to_string()), None)
        };
        self.insert_prepared_text(content_type, Some(&preview), text_content.as_deref(), blob_sha256.as_deref(), byte_size, source_app)
    }

    /// 画像エントリを保存する。画像は常に外部 blob に置く（DB inline せず）。
    pub fn insert_image_entry(
        &self,
        image_data: &[u8],
        source_app: &str,
    ) -> StorageResult<i64> {
        let byte_size = image_data.len() as i64;
        let sha = self.blob_store.write(image_data)?;
        self.insert_prepared_text(&ContentType::Image, None, None, Some(&sha), byte_size, source_app)
    }

    /// 事前に外部準備された preview / full / blob_sha256 / byte_size を DB に書き込む低レイヤ。
    /// Swift 側で SHA-256 と preview を先に計算してからここに直接送ってくる用途。
    ///
    /// content_type != Image のときは text_preview を必ず与えること（FTS 対象）。
    pub fn insert_prepared_text(
        &self,
        content_type: &ContentType,
        text_preview: Option<&str>,
        text_content: Option<&str>,
        blob_sha256: Option<&str>,
        byte_size: i64,
        source_app: &str,
    ) -> StorageResult<i64> {
        let now = now_millis();
        self.conn.execute(
            "INSERT INTO clipboard_entries
               (content_type, text_preview, text_content, blob_sha256, byte_size, source_app, created_at, copy_count, first_copied_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, ?7)",
            params![
                content_type.as_str(),
                text_preview,
                text_content,
                blob_sha256,
                byte_size,
                source_app,
                now,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Swift 側が既に blob をディスクに書いた状態で、DB 行だけを追加する用途。
    pub fn insert_prepared_blob_ref(
        &self,
        content_type: &ContentType,
        text_preview: Option<&str>,
        blob_sha256: &str,
        byte_size: i64,
        source_app: &str,
    ) -> StorageResult<i64> {
        self.insert_prepared_text(content_type, text_preview, None, Some(blob_sha256), byte_size, source_app)
    }

    pub fn get_recent_entries(&self, limit: i32) -> StorageResult<Vec<ClipboardEntry>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLS} FROM clipboard_entries
             ORDER BY created_at DESC, id DESC
             LIMIT ?1"
        ))?;
        let entries = stmt.query_map(params![limit], row_to_entry)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    pub fn get_entries_before(&self, before_timestamp: i64, limit: i32) -> StorageResult<Vec<ClipboardEntry>> {
        if before_timestamp <= 0 {
            return self.get_recent_entries(limit);
        }
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLS} FROM clipboard_entries
             WHERE created_at < ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2"
        ))?;
        let entries = stmt.query_map(params![before_timestamp, limit], row_to_entry)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(entries)
    }

    pub fn delete_entry(&self, id: i64) -> StorageResult<bool> {
        let affected = self.conn.execute(
            "DELETE FROM clipboard_entries WHERE id = ?1",
            params![id],
        )?;
        Ok(affected > 0)
    }

    /// テキスト取得。優先順: text_content(inline) > blob 参照 > preview fallback。
    /// blob 欠損時は preview を返し、呼び出し側でフラグ提示する用途を想定。
    pub fn get_entry_text(&self, id: i64) -> StorageResult<Option<String>> {
        let (preview, text_content, sha): (Option<String>, Option<String>, Option<String>) =
            match self.conn.query_row(
                "SELECT text_preview, text_content, blob_sha256 FROM clipboard_entries WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            ) {
                Ok(v) => v,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(e.into()),
            };

        if let Some(inline) = text_content {
            return Ok(Some(inline));
        }
        if let Some(sha) = sha {
            if let Some(bytes) = self.blob_store.read(&sha)? {
                match String::from_utf8(bytes) {
                    Ok(s) => return Ok(Some(s)),
                    Err(_) => return Ok(preview),
                }
            }
            // blob 欠損 → preview で fallback
            return Ok(preview);
        }
        Ok(preview)
    }

    /// 画像取得。blob 優先、旧 DB inline BLOB へも fallback。
    pub fn get_entry_image(&self, id: i64) -> StorageResult<Option<Vec<u8>>> {
        let (sha, legacy): (Option<String>, Option<Vec<u8>>) =
            match self.conn.query_row(
                "SELECT blob_sha256, image_data FROM clipboard_entries WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            ) {
                Ok(v) => v,
                Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
                Err(e) => return Err(e.into()),
            };

        if let Some(sha) = sha {
            return Ok(self.blob_store.read(&sha)?);
        }
        Ok(legacy)
    }

    /// エントリの blob_sha256 を単体取得。UI からのフル読み込み経路で使う。
    pub fn get_entry_blob_sha256(&self, id: i64) -> StorageResult<Option<String>> {
        match self.conn.query_row(
            "SELECT blob_sha256 FROM clipboard_entries WHERE id = ?1",
            params![id],
            |row| row.get::<_, Option<String>>(0),
        ) {
            Ok(v) => Ok(v),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// blob 欠損検知用: エントリが blob 参照を持っており、かつ実ファイルが無いか。
    /// UI 側で警告表示する判定用。
    pub fn is_blob_missing(&self, id: i64) -> StorageResult<bool> {
        let sha: Option<String> = match self.conn.query_row(
            "SELECT blob_sha256 FROM clipboard_entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        ) {
            Ok(v) => v,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        Ok(match sha {
            Some(s) => !self.blob_store.exists(&s),
            None => false,
        })
    }

    /// パスの最小限の検証: 空文字と NUL バイトのみ拒否する。
    /// NUL は C 文字列を切ってしまい、以降が黙って落ちる (silent truncation)
    /// ため必ず弾く必要がある。それ以外の記号は macOS の実運用パス (例:
    /// `~/Library/Application Support/CB/`、`O'Brien (backup).db`) を通したいので
    /// 許容する。SQL 文字列に埋め込む場合は呼び出し側で `escape_sql_string_literal`
    /// を通してから format! すること。
    fn validate_path(path: &str, param_name: &str) -> Result<(), rusqlite::Error> {
        if path.is_empty() {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "{} is empty",
                param_name,
            )));
        }
        if path.contains('\0') {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "{} contains a NUL byte",
                param_name,
            )));
        }
        Ok(())
    }

    /// SQLite の文字列リテラル (`'...'`) に埋め込むために `'` を `''` にエスケープする。
    /// これにより ATTACH DATABASE 文の paths に含まれるアポストロフィが
    /// 構文を破らずリテラル扱いになる。
    fn escape_sql_string_literal(s: &str) -> String {
        s.replace('\'', "''")
    }

    /// 暗号化キーの許可文字集合をホワイトリスト検証する。
    /// hex 単独ではなく、UUID(hyphen) や base64(+/=) 由来のキーも許可する。
    /// ASCII 限定 (`is_ascii_alphanumeric`) にして、全角英数字等の意図しない文字を弾く。
    fn validate_encryption_key(key: &str) -> Result<(), rusqlite::Error> {
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '+' | '/' | '='))
        {
            return Err(rusqlite::Error::InvalidParameterName(
                "encryption_key contains invalid characters (only ASCII alphanumeric, -, +, /, = allowed)"
                    .to_string(),
            ));
        }
        Ok(())
    }

    /// 平文 DB を暗号化された DB へコピーする（sqlcipher_export ベース）。
    ///
    /// - blob ファイルは DB とは別管理なのでここでは移動しない
    /// - `encrypted_path` は **存在しないファイル** を指すこと。既存ファイルが
    ///   ある状態で呼ぶと `InvalidParameterName` エラーを返す (SQLCipher の
    ///   `sqlcipher_export` が未定義動作になる + 生きている暗号化 DB を
    ///   silently 破壊してしまうリスクがあるため)。呼び出し側で既存ファイルが
    ///   「壊れた残骸か」「生きている暗号化 DB か」を判別し、後者ならそもそも
    ///   マイグレーション不要と判断すること
    pub fn migrate_to_encrypted(
        plain_path: &str,
        encrypted_path: &str,
        encryption_key: &str,
    ) -> Result<(), rusqlite::Error> {
        // 両パスとも空/NUL のみ拒否する軽量検証。以前は encrypted_path に厳しい
        // ホワイトリストを掛けていたが、macOS のユーザディレクトリに `'` `(` `)`
        // 等を含むケース (例: `O'Brien` account) を弾いてしまい、非対称になる問題
        // があった (PR #15 review round 6 指摘)。SQL インジェクション対策は
        // ATTACH 文への埋め込み前に `escape_sql_string_literal` で `'` を `''` に
        // エスケープする方式に変更する。
        Self::validate_path(plain_path, "plain_path")?;
        Self::validate_path(encrypted_path, "encrypted_path")?;
        // encryption_key は PRAGMA 経由で渡すので format! には入らないが、
        // 空キーや制御文字を拒否するために検証する。
        Self::validate_encryption_key(encryption_key)?;
        // 契約: encrypted_path が既存の場合はここでガードする (docstring 参照)。
        // 呼び出し側の削除忘れ / データ喪失リスクをコードで防ぐ defense-in-depth。
        if std::path::Path::new(encrypted_path).exists() {
            return Err(rusqlite::Error::InvalidParameterName(format!(
                "encrypted_path already exists at {}; refuse to overwrite (caller must remove or reuse it)",
                encrypted_path,
            )));
        }

        let conn = Connection::open(plain_path)?;
        // encryption_key を format! に埋めず、ATTACH は KEY 無しで実行してから
        // pragma_update で安全に鍵をセットする。encrypted_path は SQL 文字列
        // リテラルに埋め込むので `'` を `''` にエスケープする。
        let escaped_encrypted = Self::escape_sql_string_literal(encrypted_path);
        conn.execute_batch(&format!(
            "ATTACH DATABASE '{}' AS encrypted;",
            escaped_encrypted
        ))?;
        // ATTACH は encrypted_path にファイルを作る。以降のステップで失敗した場合、
        // 不完全なファイルが残ると次回起動時のリトライで別の障害要因になるので
        // best-effort で削除する (PR #15 review Low 指摘)。
        let result: Result<(), rusqlite::Error> = (|| {
            conn.pragma_update(Some("encrypted"), "key", encryption_key)?;
            conn.execute_batch(
                "SELECT sqlcipher_export('encrypted');
                 DETACH DATABASE encrypted;",
            )?;
            Ok(())
        })();
        if let Err(e) = result {
            let _ = std::fs::remove_file(encrypted_path);
            return Err(e);
        }
        Ok(())
    }

    pub fn search_entries(&self, query: &str, limit: i32) -> StorageResult<Vec<ClipboardEntry>> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.get_recent_entries(limit);
        }
        // FTS5 サニタイズ:
        // 1) `*` はプレフィクス指示子として phrase 内でも解釈されうるので除去。
        //    `^` `+` は現行の `unicode61` トークナイザでは元々セパレータとして
        //    扱われるので実害はないが、将来トークナイザを変更した際に FTS5
        //    構文文字として扱われる可能性があるため defense-in-depth で除去する
        // 2) ダブルクォートは phrase 区切りなのでエスケープ
        //
        // AND/OR/NOT/NEAR 等の boolean 演算子はここでは触らない: 最終的に
        // 全体を `"..."*` フレーズで括るので、フレーズ内では既に演算子として
        // 解釈されない (FTS5 仕様)。単語単位で除去すると "salt and pepper" 等の
        // 通常英文が破壊されて regression になる (PR #15 review 指摘)。
        let sanitized: String = trimmed
            .chars()
            .filter(|c| !matches!(c, '*' | '^' | '+'))
            .collect();
        let escaped = sanitized.replace('"', "\"\"");
        let escaped = escaped.trim();
        if escaped.is_empty() {
            return self.get_recent_entries(limit);
        }
        let fts_query = format!("\"{}\"*", escaped);

        let mut stmt = self.conn.prepare(&format!(
            "SELECT {SELECT_COLS_QUALIFIED}
             FROM clipboard_entries e
             INNER JOIN clipboard_fts f ON e.id = f.rowid
             WHERE f.text_preview MATCH ?1
               AND e.content_type != ?2
             ORDER BY e.created_at DESC
             LIMIT ?3"
        ))?;

        let entries = stmt.query_map(
            params![fts_query, ContentType::Image.as_str(), limit],
            row_to_entry,
        )?.collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn touch_entry(&self, id: i64) -> StorageResult<bool> {
        let now = now_millis();
        let affected = self.conn.execute(
            "UPDATE clipboard_entries SET created_at = ?1, copy_count = copy_count + 1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(affected > 0)
    }

    /// 期限切れエントリを削除し、参照が消えた blob を GC する。
    /// 戻り値: (削除された DB エントリ数, 削除された blob ファイル数)。
    pub fn cleanup_old_entries(&self, max_age_days: i32) -> StorageResult<(u64, u64)> {
        let now = now_millis();
        let cutoff = now - (max_age_days as i64 * 86_400_000);

        let deleted_entries = self.conn.execute(
            "DELETE FROM clipboard_entries WHERE created_at < ?1",
            params![cutoff],
        )? as u64;

        let referenced = self.list_referenced_blobs()?;
        let removed = self.blob_store.gc_orphans(&referenced)?;
        Ok((deleted_entries, removed.len() as u64))
    }

    /// DB 内で現在参照されている全 blob_sha256 の一覧。GC 判定用。
    pub fn list_referenced_blobs(&self) -> StorageResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT blob_sha256 FROM clipboard_entries WHERE blob_sha256 IS NOT NULL"
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows { out.push(r?); }
        Ok(out)
    }
}

// ─────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────

const SELECT_COLS: &str = "id, content_type, text_preview, text_content, blob_sha256, byte_size, source_app, created_at, copy_count, first_copied_at";
const SELECT_COLS_QUALIFIED: &str = "e.id, e.content_type, e.text_preview, e.text_content, e.blob_sha256, e.byte_size, e.source_app, e.created_at, e.copy_count, e.first_copied_at";

fn row_to_entry(row: &rusqlite::Row) -> rusqlite::Result<ClipboardEntry> {
    Ok(ClipboardEntry {
        id: row.get(0)?,
        content_type: ContentType::from_str(&row.get::<_, String>(1)?),
        text_preview: row.get(2)?,
        text_content: row.get(3)?,
        blob_sha256: row.get(4)?,
        byte_size: row.get(5)?,
        image_data: None,
        source_app: row.get(6)?,
        created_at: row.get(7)?,
        copy_count: row.get(8)?,
        first_copied_at: row.get(9)?,
    })
}

fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("SystemTime before UNIX_EPOCH")
        .as_millis() as i64
}

/// UTF-8 の char 境界を尊重した先頭 N バイト切り出し。
/// 途中で multi-byte を割らないよう境界まで戻す。
pub(crate) fn utf8_prefix(s: &str, byte_limit: usize) -> String {
    if s.len() <= byte_limit {
        return s.to_string();
    }
    let mut end = byte_limit;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

fn default_blob_dir(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .map(|p| p.join("blobs"))
        .unwrap_or_else(|| PathBuf::from("blobs"))
}

fn tmp_blob_dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let now_ns = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("cb_storage_test_{pid}_{now_ns}_{n}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_get_text_entry() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(
            &ContentType::PlainText,
            "Hello, world!",
            "TestApp",
        ).unwrap();
        assert!(id > 0);

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text_content.as_deref(), Some("Hello, world!"));
        assert_eq!(entries[0].text_preview.as_deref(), Some("Hello, world!"));
        assert_eq!(entries[0].source_app.as_deref(), Some("TestApp"));
        assert_eq!(entries[0].byte_size, 13);
        assert!(entries[0].blob_sha256.is_none());
    }

    #[test]
    fn test_insert_and_get_image_entry() {
        let storage = Storage::new_in_memory().unwrap();
        let image_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let id = storage.insert_image_entry(&image_data, "Preview").unwrap();
        assert!(id > 0);

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].image_data.is_none());
        assert!(entries[0].blob_sha256.is_some());
        assert_eq!(entries[0].byte_size, image_data.len() as i64);
        let fetched = storage.get_entry_image(entries[0].id).unwrap();
        assert_eq!(fetched.as_deref(), Some(image_data.as_slice()));
    }

    #[test]
    fn test_delete_entry() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(&ContentType::PlainText, "Delete me", "TestApp").unwrap();
        assert!(storage.delete_entry(id).unwrap());
        assert!(!storage.delete_entry(id).unwrap());
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_get_entry_text() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(&ContentType::PlainText, "Find me", "TestApp").unwrap();
        assert_eq!(storage.get_entry_text(id).unwrap().as_deref(), Some("Find me"));
        assert!(storage.get_entry_text(9999).unwrap().is_none());
    }

    #[test]
    fn test_get_entry_image() {
        let storage = Storage::new_in_memory().unwrap();
        let image_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let id = storage.insert_image_entry(&image_data, "Preview").unwrap();
        let data = storage.get_entry_image(id).unwrap();
        assert_eq!(data.as_deref(), Some(image_data.as_slice()));
        assert!(storage.get_entry_image(9999).unwrap().is_none());
    }

    #[test]
    fn test_get_entry_image_for_text_entry() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(&ContentType::PlainText, "Hello", "App").unwrap();
        assert!(storage.get_entry_image(id).unwrap().is_none());
    }

    #[test]
    fn test_empty_database() {
        let storage = Storage::new_in_memory().unwrap();
        assert!(storage.get_recent_entries(10).unwrap().is_empty());
    }

    #[test]
    fn test_limit() {
        let storage = Storage::new_in_memory().unwrap();
        for i in 0..5 {
            storage.insert_text_entry(&ContentType::PlainText, &format!("Entry {i}"), "TestApp").unwrap();
        }
        assert_eq!(storage.get_recent_entries(3).unwrap().len(), 3);
    }

    #[test]
    fn test_ordering() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "First", "App").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        storage.insert_text_entry(&ContentType::PlainText, "Second", "App").unwrap();
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries[0].text_preview.as_deref(), Some("Second"));
        assert_eq!(entries[1].text_preview.as_deref(), Some("First"));
    }

    #[test]
    fn test_encrypted_db_roundtrip() {
        let dir = std::env::temp_dir().join("cb_test_encrypted_v2");
        let _ = std::fs::create_dir_all(&dir);
        let db_path = dir.join("encrypted.db");
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(dir.join("blobs"));
        let key = "test-encryption-key-256bit-base64";

        {
            let storage = Storage::new(db_path.to_str().unwrap(), Some(key)).unwrap();
            storage.insert_text_entry(&ContentType::PlainText, "Secret data", "TestApp").unwrap();
        }
        {
            let storage = Storage::new(db_path.to_str().unwrap(), Some(key)).unwrap();
            let entries = storage.get_recent_entries(10).unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].text_content.as_deref(), Some("Secret data"));
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_encrypted_db_wrong_key_fails() {
        let dir = std::env::temp_dir().join("cb_test_wrong_key_v2");
        let _ = std::fs::create_dir_all(&dir);
        let db_path = dir.join("encrypted.db");
        let _ = std::fs::remove_file(&db_path);
        let _ = std::fs::remove_dir_all(dir.join("blobs"));
        {
            let storage = Storage::new(db_path.to_str().unwrap(), Some("correct-key")).unwrap();
            storage.insert_text_entry(&ContentType::PlainText, "Secret", "App").unwrap();
        }
        let result = Storage::new(db_path.to_str().unwrap(), Some("wrong-key"));
        assert!(result.is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_migrate_to_encrypted() {
        let dir = std::env::temp_dir().join("cb_test_migrate_v2");
        let _ = std::fs::create_dir_all(&dir);
        let plain_path = dir.join("plain.db");
        let encrypted_path = dir.join("migrated.db");
        let _ = std::fs::remove_file(&plain_path);
        let _ = std::fs::remove_file(&encrypted_path);
        let _ = std::fs::remove_dir_all(dir.join("blobs"));
        let key = "migration-test-key";

        {
            let storage = Storage::new(plain_path.to_str().unwrap(), None).unwrap();
            storage.insert_text_entry(&ContentType::PlainText, "Migrate me", "App").unwrap();
            storage.insert_image_entry(&[0xFF, 0xD8], "Preview").unwrap();
        }
        Storage::migrate_to_encrypted(
            plain_path.to_str().unwrap(),
            encrypted_path.to_str().unwrap(),
            key,
        ).unwrap();
        {
            let storage = Storage::new(encrypted_path.to_str().unwrap(), Some(key)).unwrap();
            let entries = storage.get_recent_entries(10).unwrap();
            assert_eq!(entries.len(), 2);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_search_entries_basic() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Hello world", "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Goodbye", "App").unwrap();
        let results = storage.search_entries("Hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].text_preview.as_deref(), Some("Hello world"));
    }

    #[test]
    fn test_search_entries_prefix_match() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Testing prefix", "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Another test", "App").unwrap();
        let results = storage.search_entries("test", 10).unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_entries_empty_query_fallback() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Entry 1", "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Entry 2", "App").unwrap();
        assert_eq!(storage.search_entries("", 10).unwrap().len(), 2);
        assert_eq!(storage.search_entries("   ", 10).unwrap().len(), 2);
    }

    #[test]
    fn test_search_entries_delete_sync() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(&ContentType::PlainText, "Delete me", "App").unwrap();
        assert_eq!(storage.search_entries("Delete", 10).unwrap().len(), 1);
        storage.delete_entry(id).unwrap();
        assert_eq!(storage.search_entries("Delete", 10).unwrap().len(), 0);
    }

    #[test]
    fn test_cleanup_old_entries() {
        let storage = Storage::new_in_memory().unwrap();
        let old_ts = now_millis() - (100 * 86_400_000);
        storage.conn.execute(
            "INSERT INTO clipboard_entries (content_type, text_preview, text_content, byte_size, source_app, created_at, first_copied_at)
             VALUES (?1, ?2, ?2, LENGTH(?2), ?3, ?4, ?4)",
            params![ContentType::PlainText.as_str(), "Old entry", "App", old_ts],
        ).unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Recent entry", "App").unwrap();

        let (deleted_entries, _) = storage.cleanup_old_entries(30).unwrap();
        assert_eq!(deleted_entries, 1);
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text_preview.as_deref(), Some("Recent entry"));
    }

    #[test]
    fn test_cleanup_preserves_recent() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "E1", "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "E2", "App").unwrap();
        let (deleted_entries, _) = storage.cleanup_old_entries(30).unwrap();
        assert_eq!(deleted_entries, 0);
        assert_eq!(storage.get_recent_entries(10).unwrap().len(), 2);
    }

    #[test]
    fn test_cleanup_empty_db() {
        let storage = Storage::new_in_memory().unwrap();
        let (deleted_entries, deleted_blobs) = storage.cleanup_old_entries(30).unwrap();
        assert_eq!(deleted_entries, 0);
        assert_eq!(deleted_blobs, 0);
    }

    #[test]
    fn test_get_entries_before_with_cursor() {
        let storage = Storage::new_in_memory().unwrap();
        let mut timestamps = Vec::new();
        for i in 0..5 {
            let ts = now_millis() + i;
            storage.conn.execute(
                "INSERT INTO clipboard_entries (content_type, text_preview, text_content, byte_size, source_app, created_at, first_copied_at)
                 VALUES (?1, ?2, ?2, LENGTH(?2), ?3, ?4, ?4)",
                params![ContentType::PlainText.as_str(), format!("Entry {i}"), "App", ts],
            ).unwrap();
            timestamps.push(ts);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let entries = storage.get_entries_before(timestamps[2], 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text_preview.as_deref(), Some("Entry 1"));
        assert_eq!(entries[1].text_preview.as_deref(), Some("Entry 0"));
    }

    #[test]
    fn test_get_entries_before_zero_timestamp() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "E1", "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "E2", "App").unwrap();
        assert_eq!(storage.get_entries_before(0, 10).unwrap().len(), 2);
    }

    #[test]
    fn test_get_entries_before_empty_db() {
        let storage = Storage::new_in_memory().unwrap();
        assert!(storage.get_entries_before(999999999, 10).unwrap().is_empty());
    }

    #[test]
    fn test_touch_entry() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(&ContentType::PlainText, "Touch me", "App").unwrap();
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries[0].copy_count, 1);
        let original_first_copied = entries[0].first_copied_at;
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(storage.touch_entry(id).unwrap());
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries[0].copy_count, 2);
        assert!(entries[0].created_at > original_first_copied);
        assert_eq!(entries[0].first_copied_at, original_first_copied);
    }

    #[test]
    fn test_touch_entry_moves_to_top() {
        let storage = Storage::new_in_memory().unwrap();
        let id1 = storage.insert_text_entry(&ContentType::PlainText, "First", "App").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        storage.insert_text_entry(&ContentType::PlainText, "Second", "App").unwrap();
        assert_eq!(storage.get_recent_entries(10).unwrap()[0].text_preview.as_deref(), Some("Second"));
        std::thread::sleep(std::time::Duration::from_millis(10));
        storage.touch_entry(id1).unwrap();
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries[0].text_preview.as_deref(), Some("First"));
        assert_eq!(entries[0].copy_count, 2);
    }

    #[test]
    fn test_touch_nonexistent_entry() {
        let storage = Storage::new_in_memory().unwrap();
        assert!(!storage.touch_entry(9999).unwrap());
    }

    #[test]
    fn test_new_entry_has_copy_count_and_first_copied() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Hello", "App").unwrap();
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries[0].copy_count, 1);
        assert_eq!(entries[0].first_copied_at, entries[0].created_at);
    }

    #[test]
    fn test_get_entries_before_boundary() {
        let storage = Storage::new_in_memory().unwrap();
        let ts = now_millis();
        for i in 0..3 {
            storage.conn.execute(
                "INSERT INTO clipboard_entries (content_type, text_preview, text_content, byte_size, source_app, created_at, first_copied_at)
                 VALUES (?1, ?2, ?2, LENGTH(?2), ?3, ?4, ?4)",
                params![ContentType::PlainText.as_str(), format!("Entry {i}"), "App", ts],
            ).unwrap();
        }
        assert_eq!(storage.get_entries_before(ts, 10).unwrap().len(), 0);
        assert_eq!(storage.get_entries_before(ts + 1, 10).unwrap().len(), 3);
    }

    #[test]
    fn test_millisecond_precision_ordering() {
        let storage = Storage::new_in_memory().unwrap();
        let base_ts = now_millis();
        for i in 0..3 {
            storage.conn.execute(
                "INSERT INTO clipboard_entries (content_type, text_preview, text_content, byte_size, source_app, created_at, first_copied_at)
                 VALUES (?1, ?2, ?2, LENGTH(?2), ?3, ?4, ?4)",
                params![ContentType::PlainText.as_str(), format!("Entry {i}"), "App", base_ts + i],
            ).unwrap();
        }
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].text_preview.as_deref(), Some("Entry 2"));
        assert_eq!(entries[2].text_preview.as_deref(), Some("Entry 0"));
    }

    // ─────────────────────────────────────────────
    // 新規: blob externalization / 閾値変更耐性 / GC
    // ─────────────────────────────────────────────

    #[test]
    fn test_large_text_externalized_to_blob() {
        let storage = Storage::new_in_memory().unwrap();
        let big = "A".repeat(TEXT_EXTERNALIZE_THRESHOLD_BYTES + 100);
        let id = storage.insert_text_entry(&ContentType::PlainText, &big, "App").unwrap();

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 1);
        // 大サイズは text_content が None、blob_sha256 が Some
        assert!(entries[0].text_content.is_none());
        assert!(entries[0].blob_sha256.is_some());
        assert_eq!(entries[0].byte_size, big.len() as i64);
        // preview は先頭 PREVIEW_BYTES バイトまで
        assert!(entries[0].text_preview.as_ref().unwrap().len() <= PREVIEW_BYTES);
        // フル取得は blob 経由で復元される
        assert_eq!(storage.get_entry_text(id).unwrap().as_deref(), Some(big.as_str()));
    }

    #[test]
    fn test_small_text_stays_inline() {
        let storage = Storage::new_in_memory().unwrap();
        let small = "small text";
        storage.insert_text_entry(&ContentType::PlainText, small, "App").unwrap();
        let entries = storage.get_recent_entries(10).unwrap();
        assert!(entries[0].text_content.is_some());
        assert!(entries[0].blob_sha256.is_none());
    }

    #[test]
    fn test_threshold_agnostic_read_after_change() {
        // 閾値T1相当で大サイズ保存 → 閾値T2に相当する読み出しでも問題なし。
        // 実際には THRESHOLD は const だが「保存済みデータの読み方は保存時の閾値に依存しない」不変を確認。
        let storage = Storage::new_in_memory().unwrap();

        // inline エントリと blob エントリを同一DBに混在
        let inline_id = storage.insert_text_entry(&ContentType::PlainText, "inline", "App").unwrap();
        let big = "B".repeat(TEXT_EXTERNALIZE_THRESHOLD_BYTES + 500);
        let blob_id = storage.insert_text_entry(&ContentType::PlainText, &big, "App").unwrap();

        assert_eq!(storage.get_entry_text(inline_id).unwrap().as_deref(), Some("inline"));
        assert_eq!(storage.get_entry_text(blob_id).unwrap().as_deref(), Some(big.as_str()));
    }

    #[test]
    fn test_missing_blob_falls_back_to_preview() {
        let storage = Storage::new_in_memory().unwrap();
        let big = "C".repeat(TEXT_EXTERNALIZE_THRESHOLD_BYTES + 200);
        let id = storage.insert_text_entry(&ContentType::PlainText, &big, "App").unwrap();

        // blob ファイルを手動削除
        let sha = storage.get_recent_entries(10).unwrap()[0].blob_sha256.clone().unwrap();
        assert!(storage.blob_store.delete(&sha).unwrap());
        assert!(storage.is_blob_missing(id).unwrap());

        // preview で fallback (最大 PREVIEW_BYTES 分の C)
        let recovered = storage.get_entry_text(id).unwrap();
        assert!(recovered.is_some());
        assert!(recovered.unwrap().starts_with("CCC"));
    }

    #[test]
    fn test_image_always_externalized() {
        let storage = Storage::new_in_memory().unwrap();
        let data = vec![0xDE, 0xAD, 0xBE, 0xEF];
        let id = storage.insert_image_entry(&data, "App").unwrap();

        let entries = storage.get_recent_entries(10).unwrap();
        assert!(entries[0].blob_sha256.is_some());
        assert_eq!(storage.get_entry_image(id).unwrap().as_deref(), Some(data.as_slice()));
    }

    #[test]
    fn test_blob_dedup_between_entries() {
        let storage = Storage::new_in_memory().unwrap();
        let big = "D".repeat(TEXT_EXTERNALIZE_THRESHOLD_BYTES + 10);
        storage.insert_text_entry(&ContentType::PlainText, &big, "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, &big, "App").unwrap();

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].blob_sha256, entries[1].blob_sha256);

        // blob ストア上のファイル数は 1 のまま
        assert_eq!(storage.blob_store.list_all().unwrap().len(), 1);
    }

    #[test]
    fn test_cleanup_gc_removes_orphan_blobs() {
        let storage = Storage::new_in_memory().unwrap();
        let old_ts = now_millis() - (100 * 86_400_000);
        let big = "E".repeat(TEXT_EXTERNALIZE_THRESHOLD_BYTES + 10);
        // 古い blob エントリ 2 件（内容は別々にして 2 blob）
        let big_a = format!("{big}A");
        let big_b = format!("{big}B");
        let sha_a = storage.blob_store.write(big_a.as_bytes()).unwrap();
        let sha_b = storage.blob_store.write(big_b.as_bytes()).unwrap();
        for (sha, text) in [(&sha_a, &big_a), (&sha_b, &big_b)] {
            storage.conn.execute(
                "INSERT INTO clipboard_entries (content_type, text_preview, blob_sha256, byte_size, source_app, created_at, first_copied_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![ContentType::PlainText.as_str(), &text[..100], sha, text.len() as i64, "App", old_ts],
            ).unwrap();
        }
        // 新しい blob エントリ 1 件（残る）
        let recent = format!("{big}KEEP");
        storage.insert_text_entry(&ContentType::PlainText, &recent, "App").unwrap();
        let recent_sha = storage.get_recent_entries(1).unwrap()[0].blob_sha256.clone().unwrap();

        assert_eq!(storage.blob_store.list_all().unwrap().len(), 3);

        let (deleted_entries, deleted_blobs) = storage.cleanup_old_entries(30).unwrap();
        assert_eq!(deleted_entries, 2);
        assert_eq!(deleted_blobs, 2);
        assert!(storage.blob_store.exists(&recent_sha));
        assert!(!storage.blob_store.exists(&sha_a));
        assert!(!storage.blob_store.exists(&sha_b));
    }

    #[test]
    fn test_utf8_prefix_respects_char_boundary() {
        let s = "あいうえお"; // 3バイト×5 = 15バイト
        // 4バイトで切ると "あ" (3バイト) までにトリムされるはず
        let out = utf8_prefix(s, 4);
        assert_eq!(out, "あ");
        // 6バイトなら "あい"
        let out = utf8_prefix(s, 6);
        assert_eq!(out, "あい");
        // 制限が長いなら全体
        let out = utf8_prefix(s, 100);
        assert_eq!(out, s);
    }

    #[test]
    fn test_search_ignores_full_text_beyond_preview() {
        // preview は先頭 PREVIEW_BYTES のみ FTS に載る前提の確認。
        // 大きな文字列の末尾にしか出現しないキーワードは検索でヒットしない。
        let storage = Storage::new_in_memory().unwrap();
        let mut big = "F".repeat(PREVIEW_BYTES + 100);
        big.push_str(" NEEDLE_AT_END");
        // 十分大きく、かつ blob 化される値にする
        while big.len() <= TEXT_EXTERNALIZE_THRESHOLD_BYTES { big.push('X'); }
        storage.insert_text_entry(&ContentType::PlainText, &big, "App").unwrap();

        let results = storage.search_entries("NEEDLE_AT_END", 10).unwrap();
        assert!(results.is_empty(), "preview 外の文字列は FTS で見つからない");
    }

    // ─────────────────────────────────────────────────────────────
    // FTS5 サニタイズ / migrate_to_encrypted バリデーション (#2, #3)
    // ─────────────────────────────────────────────────────────────

    #[test]
    fn test_search_special_chars_do_not_break_query() {
        // `*` `^` `+` を混ぜても FTS5 構文エラーにならず、除去された残り文字列で
        // フレーズ検索されること。
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "prefix content", "App").unwrap();
        let results = storage.search_entries("prefix*^+", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_only_removed_chars_falls_back_to_recent() {
        // 除去対象文字だけで構成されたクエリはサニタイズ後に空になり、
        // get_recent_entries フォールバックで全件返る。
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "a", "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "b", "App").unwrap();
        let results = storage.search_entries("^+*^+", 10).unwrap();
        assert_eq!(results.len(), 2, "全て除去対象なので empty query fallback で全件");
    }

    #[test]
    fn test_search_double_quotes_are_escaped() {
        // クエリ内の `"` を `""` にエスケープした上でフレーズ検索されること。
        // エスケープを忘れると `"foo"bar"` みたいなクエリでフレーズが早く閉じて
        // 構文エラーになる。
        let storage = Storage::new_in_memory().unwrap();
        // ダブルクォートを含む本文をそのまま保存し、
        // 検索側でも同じ文字列を投げてヒットすることを確認する。
        storage
            .insert_text_entry(&ContentType::PlainText, r#"say "hello""#, "App")
            .unwrap();
        let results = storage.search_entries(r#"say "hello""#, 10).unwrap();
        assert_eq!(
            results.len(),
            1,
            r#"ダブルクォート含みのクエリでもエスケープされて "say ""hello"""* として実マッチする"#
        );
    }

    // Regression tests (PR #15 review): 演算子相当の英単語を含む実データが
    // 検索できなくなっていた回帰。フレーズ内では AND/OR/NOT/NEAR は元々演算子
    // として解釈されないので、単語除去は撤去済み。

    #[test]
    fn test_search_finds_text_containing_and() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "salt and pepper", "App").unwrap();
        let results = storage.search_entries("salt and pepper", 10).unwrap();
        assert_eq!(results.len(), 1, "\"salt and pepper\" は自己再検索でヒットすべき");
    }

    #[test]
    fn test_search_finds_text_containing_or() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "cash or credit", "App").unwrap();
        let results = storage.search_entries("cash or credit", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_finds_text_containing_not() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "do not disturb", "App").unwrap();
        let results = storage.search_entries("do not disturb", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_search_finds_text_containing_near() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "walk near park", "App").unwrap();
        let results = storage.search_entries("walk near park", 10).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_migrate_rejects_nul_in_encrypted_path() {
        // NUL は C 文字列を切ってしまう silent truncation なので必ず拒否する。
        let err = Storage::migrate_to_encrypted("/tmp/a.db", "/tmp/b\0evil.db", "abcd").unwrap_err();
        assert!(matches!(err, rusqlite::Error::InvalidParameterName(_)));
    }

    #[test]
    fn test_migrate_rejects_nul_in_plain_path() {
        // plain_path 側の NUL 拒否も対称に担保する。
        let err = Storage::migrate_to_encrypted("/tmp/a\0evil.db", "/tmp/b.db", "abcd").unwrap_err();
        assert!(matches!(err, rusqlite::Error::InvalidParameterName(_)));
    }

    #[test]
    fn test_escape_sql_string_literal_doubles_apostrophes() {
        // format! に埋め込む前段でアポストロフィが `''` にエスケープされることを保証する。
        assert_eq!(Storage::escape_sql_string_literal("O'Brien"), "O''Brien");
        assert_eq!(Storage::escape_sql_string_literal("no quote"), "no quote");
        assert_eq!(Storage::escape_sql_string_literal("multi 'quote' string"), "multi ''quote'' string");
    }

    #[test]
    fn test_migrate_rejects_special_chars_in_key() {
        let err = Storage::migrate_to_encrypted("/tmp/a.db", "/tmp/b.db", "abc'; DROP--").unwrap_err();
        assert!(matches!(err, rusqlite::Error::InvalidParameterName(_)));
    }

    #[test]
    fn test_migrate_rejects_empty_plain_path() {
        let err = Storage::migrate_to_encrypted("", "/tmp/b.db", "abcd").unwrap_err();
        assert!(matches!(err, rusqlite::Error::InvalidParameterName(_)));
    }

    #[test]
    fn test_migrate_rejects_empty_encrypted_path() {
        let err = Storage::migrate_to_encrypted("/tmp/a.db", "", "abcd").unwrap_err();
        assert!(matches!(err, rusqlite::Error::InvalidParameterName(_)));
    }

    #[test]
    fn test_migrate_rejects_empty_key() {
        let err = Storage::migrate_to_encrypted("/tmp/a.db", "/tmp/b.db", "").unwrap_err();
        assert!(matches!(err, rusqlite::Error::InvalidParameterName(_)));
    }

    #[test]
    fn test_migrate_accepts_valid_path_and_key() {
        // 実 DB 生成でエンドツーエンドに通ること。
        let dir = std::env::temp_dir().join("cb_test_migrate_accepts_v2");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("plain.db");
        let encrypted = dir.join("encrypted.db");
        {
            let s = Storage::new(plain.to_str().unwrap(), None).unwrap();
            s.insert_text_entry(&ContentType::PlainText, "secret", "App").unwrap();
        }
        Storage::migrate_to_encrypted(
            plain.to_str().unwrap(),
            encrypted.to_str().unwrap(),
            "abcdefghijklmnop",
        )
        .expect("migration should succeed with valid inputs");
        // 暗号化された DB を鍵付きで開けて、内容が読めることを確認
        let s = Storage::new(encrypted.to_str().unwrap(), Some("abcdefghijklmnop")).unwrap();
        let all = s.get_recent_entries(10).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text_content.as_deref(), Some("secret"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_migrate_accepts_plain_and_encrypted_path_with_special_chars() {
        // macOS の実運用では `O'Brien` のようなユーザ名を持つアカウント配下に
        // plain / encrypted の兄弟 DB を置くのでどちらも `'` / `(` / `)` を含む
        // ことになる。両方が同一ディレクトリ由来でも E2E で通ることを検証する。
        let dir = std::env::temp_dir().join("cb_test_migrate_special_chars_dir_(O'Brien)");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("O'Brien (backup).db");
        let encrypted = dir.join("O'Brien encrypted.db");
        {
            let s = Storage::new(plain.to_str().unwrap(), None).unwrap();
            s.insert_text_entry(&ContentType::PlainText, "hi", "App").unwrap();
        }
        Storage::migrate_to_encrypted(
            plain.to_str().unwrap(),
            encrypted.to_str().unwrap(),
            "abcdefghijklmnop",
        )
        .expect("both paths with special chars must be accepted");

        // 実際に鍵付きで開けて内容が読めることまで確認
        let s = Storage::new(encrypted.to_str().unwrap(), Some("abcdefghijklmnop")).unwrap();
        let all = s.get_recent_entries(10).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].text_content.as_deref(), Some("hi"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_migrate_defends_against_encrypted_path_injection() {
        // encrypted_path に `';` の SQL 破壊シーケンスが含まれていても、
        // format! 埋め込み前に `'` が `''` にエスケープされ、ATTACH 文が
        // 破壊されないこと。実 DB 生成までは行かず (パスとして無効なので)、
        // ATTACH レベルの構文エラーではなく IO/構文健全性エラーになる。
        let dir = std::env::temp_dir().join("cb_test_migrate_injection_defense");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("plain.db");
        {
            let s = Storage::new(plain.to_str().unwrap(), None).unwrap();
            s.insert_text_entry(&ContentType::PlainText, "hi", "App").unwrap();
        }
        // 意図的に SQL 破壊パターンを埋め込んだ encrypted_path
        let malicious = dir
            .join("evil'; DROP TABLE clipboard_entries; --.db")
            .to_string_lossy()
            .to_string();
        // ここでは「ATTACH の破壊が起きないこと」を担保するのが目的。
        // ファイル生成自体は成功する可能性 (macOS はほとんどの文字を許容) もあるので
        // 結果の Ok/Err は問わず、後段の DROP が実行されていないことを確認する。
        let _ = Storage::migrate_to_encrypted(
            plain.to_str().unwrap(),
            &malicious,
            "abcdefghijklmnop",
        );
        // 元 DB の clipboard_entries テーブルが破壊されずに読めること
        let s = Storage::new(plain.to_str().unwrap(), None).unwrap();
        let all = s.get_recent_entries(10).unwrap();
        assert_eq!(all.len(), 1, "元 DB の clipboard_entries が DROP されていないこと");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_migrate_refuses_existing_encrypted_path() {
        // 既存の encrypted_path に対しては明示的に InvalidParameterName を返す。
        // これによりデータ喪失リスクを避け、呼び出し側に判断を委ねる。
        let dir = std::env::temp_dir().join("cb_test_migrate_refuse_existing");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("plain.db");
        let encrypted = dir.join("encrypted.db");
        {
            let s = Storage::new(plain.to_str().unwrap(), None).unwrap();
            s.insert_text_entry(&ContentType::PlainText, "payload", "App").unwrap();
        }
        // 予め encrypted_path にダミーファイルを置いておく
        std::fs::write(&encrypted, b"pretend this is an existing DB").unwrap();

        let err = Storage::migrate_to_encrypted(
            plain.to_str().unwrap(),
            encrypted.to_str().unwrap(),
            "abcdefghijklmnop",
        )
        .unwrap_err();
        assert!(matches!(err, rusqlite::Error::InvalidParameterName(_)));
        // 既存ファイルは触られないこと
        let content = std::fs::read(&encrypted).unwrap();
        assert_eq!(content, b"pretend this is an existing DB");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_migrate_rerun_requires_caller_to_remove_target() {
        // 既存 encrypted_path があると Rust 側が Err を返すので、呼び出し側は
        // ファイルを削除してから再実行する。ここでは「残骸を消してから
        // 再実行すれば通る」ことを担保する。
        let dir = std::env::temp_dir().join("cb_test_migrate_rerun");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let plain = dir.join("plain.db");
        let encrypted = dir.join("encrypted.db");
        {
            let s = Storage::new(plain.to_str().unwrap(), None).unwrap();
            s.insert_text_entry(&ContentType::PlainText, "rerun-payload", "App").unwrap();
        }
        // 1回目
        Storage::migrate_to_encrypted(
            plain.to_str().unwrap(),
            encrypted.to_str().unwrap(),
            "abcdefghijklmnop",
        )
        .expect("first migration should succeed");
        // 残骸を削除してから 2 回目
        std::fs::remove_file(&encrypted).unwrap();
        Storage::migrate_to_encrypted(
            plain.to_str().unwrap(),
            encrypted.to_str().unwrap(),
            "abcdefghijklmnop",
        )
        .expect("migration after removing target should succeed");

        let s = Storage::new(encrypted.to_str().unwrap(), Some("abcdefghijklmnop")).unwrap();
        let all = s.get_recent_entries(10).unwrap();
        assert!(all.iter().any(|e| e.text_content.as_deref() == Some("rerun-payload")));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
