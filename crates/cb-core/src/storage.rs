use rusqlite::{Connection, params};
use crate::models::{ClipboardEntry, ContentType};

pub struct Storage {
    conn: Connection,
}

impl Storage {
    pub fn new(db_path: &str, encryption_key: Option<&str>) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(db_path)?;

        if let Some(key) = encryption_key {
            conn.pragma_update(None, "key", key)?;
        }

        let storage = Storage { conn };
        storage.init_schema()?;
        Ok(storage)
    }

    pub fn new_in_memory() -> Result<Self, rusqlite::Error> {
        let conn = Connection::open_in_memory()?;
        let storage = Storage { conn };
        storage.init_schema()?;
        Ok(storage)
    }

    fn init_schema(&self) -> Result<(), rusqlite::Error> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS clipboard_entries (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                content_type  TEXT NOT NULL,
                text_content  TEXT,
                image_data    BLOB,
                source_app    TEXT,
                created_at    INTEGER NOT NULL,
                copy_count    INTEGER NOT NULL DEFAULT 1,
                first_copied_at INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_created_at
            ON clipboard_entries(created_at DESC);

            CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts
            USING fts5(text_content, content='clipboard_entries', content_rowid='id');

            CREATE TRIGGER IF NOT EXISTS clipboard_entries_ai
            AFTER INSERT ON clipboard_entries
            BEGIN
                INSERT INTO clipboard_fts(rowid, text_content)
                VALUES (new.id, new.text_content);
            END;

            CREATE TRIGGER IF NOT EXISTS clipboard_entries_ad
            AFTER DELETE ON clipboard_entries
            BEGIN
                INSERT INTO clipboard_fts(clipboard_fts, rowid, text_content)
                VALUES ('delete', old.id, old.text_content);
            END;"
        )?;

        // Migrate existing tables: add copy_count and first_copied_at if missing
        self.migrate_add_columns()?;

        // Migrate timestamps from seconds to milliseconds
        self.conn.execute_batch(
            "UPDATE clipboard_entries SET created_at = created_at * 1000 WHERE created_at > 0 AND created_at < 10000000000;
             UPDATE clipboard_entries SET first_copied_at = first_copied_at * 1000 WHERE first_copied_at > 0 AND first_copied_at < 10000000000;"
        )?;

        // Rebuild FTS index only if it's out of sync (e.g., after table creation with existing data)
        let fts_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_fts", [], |row| row.get(0)
        ).unwrap_or(0);
        let main_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM clipboard_entries WHERE text_content IS NOT NULL", [], |row| row.get(0)
        ).unwrap_or(0);
        if fts_count != main_count {
            self.conn.execute_batch("INSERT INTO clipboard_fts(clipboard_fts) VALUES ('rebuild');")?;
        }

        Ok(())
    }

    fn migrate_add_columns(&self) -> Result<(), rusqlite::Error> {
        let has_copy_count: bool = self.conn
            .prepare("SELECT copy_count FROM clipboard_entries LIMIT 0")
            .is_ok();
        if !has_copy_count {
            self.conn.execute_batch(
                "ALTER TABLE clipboard_entries ADD COLUMN copy_count INTEGER NOT NULL DEFAULT 1;"
            )?;
        }

        let has_first_copied_at: bool = self.conn
            .prepare("SELECT first_copied_at FROM clipboard_entries LIMIT 0")
            .is_ok();
        if !has_first_copied_at {
            self.conn.execute_batch(
                "ALTER TABLE clipboard_entries ADD COLUMN first_copied_at INTEGER NOT NULL DEFAULT 0;"
            )?;
            self.conn.execute_batch(
                "UPDATE clipboard_entries SET first_copied_at = created_at WHERE first_copied_at = 0;"
            )?;
        }
        Ok(())
    }

    pub fn insert_text_entry(
        &self,
        content_type: &ContentType,
        text: &str,
        source_app: &str,
    ) -> Result<i64, rusqlite::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime before UNIX_EPOCH")
            .as_millis() as i64;

        self.conn.execute(
            "INSERT INTO clipboard_entries (content_type, text_content, source_app, created_at, copy_count, first_copied_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?4)",
            params![content_type.as_str(), text, source_app, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn insert_image_entry(
        &self,
        image_data: &[u8],
        source_app: &str,
    ) -> Result<i64, rusqlite::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime before UNIX_EPOCH")
            .as_millis() as i64;

        self.conn.execute(
            "INSERT INTO clipboard_entries (content_type, image_data, source_app, created_at, copy_count, first_copied_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?4)",
            params![ContentType::Image.as_str(), image_data, source_app, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_recent_entries(&self, limit: i32) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content_type, text_content, source_app, created_at, copy_count, first_copied_at
             FROM clipboard_entries
             ORDER BY created_at DESC, id DESC
             LIMIT ?1"
        )?;

        let entries = stmt.query_map(params![limit], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content_type: ContentType::from_str(
                    &row.get::<_, String>(1)?
                ),
                text_content: row.get(2)?,
                image_data: None,
                source_app: row.get(3)?,
                created_at: row.get(4)?,
                copy_count: row.get(5)?,
                first_copied_at: row.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn get_entries_before(&self, before_timestamp: i64, limit: i32) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
        if before_timestamp <= 0 {
            return self.get_recent_entries(limit);
        }
        let mut stmt = self.conn.prepare(
            "SELECT id, content_type, text_content, source_app, created_at, copy_count, first_copied_at
             FROM clipboard_entries
             WHERE created_at < ?1
             ORDER BY created_at DESC, id DESC
             LIMIT ?2"
        )?;

        let entries = stmt.query_map(params![before_timestamp, limit], |row| {
            Ok(ClipboardEntry {
                id: row.get(0)?,
                content_type: ContentType::from_str(&row.get::<_, String>(1)?),
                text_content: row.get(2)?,
                image_data: None,
                source_app: row.get(3)?,
                created_at: row.get(4)?,
                copy_count: row.get(5)?,
                first_copied_at: row.get(6)?,
            })
        })?.collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn delete_entry(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let affected = self.conn.execute(
            "DELETE FROM clipboard_entries WHERE id = ?1",
            params![id],
        )?;
        Ok(affected > 0)
    }

    pub fn get_entry_text(&self, id: i64) -> Result<Option<String>, rusqlite::Error> {
        let result = self.conn.query_row(
            "SELECT text_content FROM clipboard_entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        );
        match result {
            Ok(text) => Ok(text),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn migrate_to_encrypted(
        plain_path: &str,
        encrypted_path: &str,
        encryption_key: &str,
    ) -> Result<(), rusqlite::Error> {
        // Validate inputs to prevent SQL injection in ATTACH DATABASE
        // (parameterized queries are not supported for ATTACH)
        if encrypted_path.contains('\'') || encrypted_path.contains('\0') {
            return Err(rusqlite::Error::InvalidParameterName(
                "encrypted_path contains invalid characters".to_string(),
            ));
        }
        if encryption_key.contains('\'') || encryption_key.contains('\0') {
            return Err(rusqlite::Error::InvalidParameterName(
                "encryption_key contains invalid characters".to_string(),
            ));
        }

        let conn = Connection::open(plain_path)?;
        conn.execute_batch(&format!(
            "ATTACH DATABASE '{}' AS encrypted KEY '{}';
             SELECT sqlcipher_export('encrypted');
             DETACH DATABASE encrypted;",
            encrypted_path, encryption_key
        ))?;
        Ok(())
    }

    pub fn get_entry_image(&self, id: i64) -> Result<Option<Vec<u8>>, rusqlite::Error> {
        let result = self.conn.query_row(
            "SELECT image_data FROM clipboard_entries WHERE id = ?1",
            params![id],
            |row| row.get(0),
        );
        match result {
            Ok(data) => Ok(data),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn search_entries(&self, query: &str, limit: i32) -> Result<Vec<ClipboardEntry>, rusqlite::Error> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return self.get_recent_entries(limit);
        }

        // Sanitize FTS5 special characters: remove * (prefix operator) and escape double quotes
        let sanitized = trimmed.replace('*', "");
        let escaped = sanitized.replace('"', "\"\"");
        let fts_query = if escaped.is_empty() {
            return self.get_recent_entries(limit);
        } else {
            format!("\"{}\"*", escaped)
        };

        let mut stmt = self.conn.prepare(
            "SELECT e.id, e.content_type, e.text_content, e.source_app, e.created_at, e.copy_count, e.first_copied_at
             FROM clipboard_entries e
             INNER JOIN clipboard_fts f ON e.id = f.rowid
             WHERE f.text_content MATCH ?1
               AND e.content_type != ?2
             ORDER BY e.created_at DESC
             LIMIT ?3"
        )?;

        let entries = stmt.query_map(
            params![fts_query, ContentType::Image.as_str(), limit],
            |row| {
                Ok(ClipboardEntry {
                    id: row.get(0)?,
                    content_type: ContentType::from_str(
                        &row.get::<_, String>(1)?
                    ),
                    text_content: row.get(2)?,
                    image_data: None,
                    source_app: row.get(3)?,
                    created_at: row.get(4)?,
                    copy_count: row.get(5)?,
                    first_copied_at: row.get(6)?,
                })
            }
        )?.collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    pub fn touch_entry(&self, id: i64) -> Result<bool, rusqlite::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime before UNIX_EPOCH")
            .as_millis() as i64;

        let affected = self.conn.execute(
            "UPDATE clipboard_entries SET created_at = ?1, copy_count = copy_count + 1 WHERE id = ?2",
            params![now, id],
        )?;
        Ok(affected > 0)
    }

    pub fn cleanup_old_entries(&self, max_age_days: i32) -> Result<u64, rusqlite::Error> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("SystemTime before UNIX_EPOCH")
            .as_millis() as i64;

        let cutoff = now - (max_age_days as i64 * 86_400_000);

        let affected = self.conn.execute(
            "DELETE FROM clipboard_entries WHERE created_at < ?1",
            params![cutoff],
        )?;

        Ok(affected as u64)
    }
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
        assert_eq!(entries[0].source_app.as_deref(), Some("TestApp"));
    }

    #[test]
    fn test_insert_and_get_image_entry() {
        let storage = Storage::new_in_memory().unwrap();
        let image_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let id = storage.insert_image_entry(&image_data, "Preview").unwrap();
        assert!(id > 0);

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].image_data.is_none()); // list queries don't fetch image_data
        let fetched = storage.get_entry_image(entries[0].id).unwrap();
        assert_eq!(fetched.as_deref(), Some(image_data.as_slice()));
    }

    #[test]
    fn test_delete_entry() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(
            &ContentType::PlainText,
            "Delete me",
            "TestApp",
        ).unwrap();

        assert!(storage.delete_entry(id).unwrap());
        assert!(!storage.delete_entry(id).unwrap());

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_get_entry_text() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(
            &ContentType::PlainText,
            "Find me",
            "TestApp",
        ).unwrap();

        let text = storage.get_entry_text(id).unwrap();
        assert_eq!(text.as_deref(), Some("Find me"));

        let missing = storage.get_entry_text(9999).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_entry_image() {
        let storage = Storage::new_in_memory().unwrap();
        let image_data = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let id = storage.insert_image_entry(&image_data, "Preview").unwrap();

        let data = storage.get_entry_image(id).unwrap();
        assert_eq!(data.as_deref(), Some(image_data.as_slice()));

        let missing = storage.get_entry_image(9999).unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_get_entry_image_for_text_entry() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(&ContentType::PlainText, "Hello", "App").unwrap();

        let data = storage.get_entry_image(id).unwrap();
        assert!(data.is_none());
    }

    #[test]
    fn test_empty_database() {
        let storage = Storage::new_in_memory().unwrap();
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_limit() {
        let storage = Storage::new_in_memory().unwrap();
        for i in 0..5 {
            storage.insert_text_entry(
                &ContentType::PlainText,
                &format!("Entry {i}"),
                "TestApp",
            ).unwrap();
        }

        let entries = storage.get_recent_entries(3).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_ordering() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "First", "App").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        storage.insert_text_entry(&ContentType::PlainText, "Second", "App").unwrap();

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries[0].text_content.as_deref(), Some("Second"));
        assert_eq!(entries[1].text_content.as_deref(), Some("First"));
    }

    #[test]
    fn test_encrypted_db_roundtrip() {
        let dir = std::env::temp_dir().join("cb_test_encrypted");
        let _ = std::fs::create_dir_all(&dir);
        let db_path = dir.join("encrypted.db");
        let _ = std::fs::remove_file(&db_path);

        let key = "test-encryption-key-256bit-base64";

        // Write data with encryption key
        {
            let storage = Storage::new(db_path.to_str().unwrap(), Some(key)).unwrap();
            storage.insert_text_entry(&ContentType::PlainText, "Secret data", "TestApp").unwrap();
        }

        // Reopen with same key — data should be readable
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
        let dir = std::env::temp_dir().join("cb_test_wrong_key");
        let _ = std::fs::create_dir_all(&dir);
        let db_path = dir.join("encrypted.db");
        let _ = std::fs::remove_file(&db_path);

        // Create encrypted DB
        {
            let storage = Storage::new(db_path.to_str().unwrap(), Some("correct-key")).unwrap();
            storage.insert_text_entry(&ContentType::PlainText, "Secret", "App").unwrap();
        }

        // Open with wrong key — should fail
        let result = Storage::new(db_path.to_str().unwrap(), Some("wrong-key"));
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_migrate_to_encrypted() {
        let dir = std::env::temp_dir().join("cb_test_migrate");
        let _ = std::fs::create_dir_all(&dir);
        let plain_path = dir.join("plain.db");
        let encrypted_path = dir.join("migrated.db");
        let _ = std::fs::remove_file(&plain_path);
        let _ = std::fs::remove_file(&encrypted_path);

        let key = "migration-test-key";

        // Create plain DB with data
        {
            let storage = Storage::new(plain_path.to_str().unwrap(), None).unwrap();
            storage.insert_text_entry(&ContentType::PlainText, "Migrate me", "App").unwrap();
            storage.insert_image_entry(&[0xFF, 0xD8], "Preview").unwrap();
        }

        // Migrate to encrypted
        Storage::migrate_to_encrypted(
            plain_path.to_str().unwrap(),
            encrypted_path.to_str().unwrap(),
            key,
        ).unwrap();

        // Open encrypted DB and verify data
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
        assert_eq!(results[0].text_content.as_deref(), Some("Hello world"));
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

        let results = storage.search_entries("", 10).unwrap();
        assert_eq!(results.len(), 2);

        let results_whitespace = storage.search_entries("   ", 10).unwrap();
        assert_eq!(results_whitespace.len(), 2);
    }

    #[test]
    fn test_search_entries_delete_sync() {
        let storage = Storage::new_in_memory().unwrap();
        let id = storage.insert_text_entry(&ContentType::PlainText, "Delete me", "App").unwrap();

        let results_before = storage.search_entries("Delete", 10).unwrap();
        assert_eq!(results_before.len(), 1);

        storage.delete_entry(id).unwrap();

        let results_after = storage.search_entries("Delete", 10).unwrap();
        assert_eq!(results_after.len(), 0);
    }

    #[test]
    fn test_cleanup_old_entries() {
        let storage = Storage::new_in_memory().unwrap();

        // Insert old entry (simulate old timestamp)
        let old_timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64 - (100 * 86_400_000); // 100 days ago

        storage.conn.execute(
            "INSERT INTO clipboard_entries (content_type, text_content, source_app, created_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![ContentType::PlainText.as_str(), "Old entry", "App", old_timestamp],
        ).unwrap();

        // Insert recent entry
        storage.insert_text_entry(&ContentType::PlainText, "Recent entry", "App").unwrap();

        // Cleanup entries older than 30 days
        let deleted = storage.cleanup_old_entries(30).unwrap();
        assert_eq!(deleted, 1);

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].text_content.as_deref(), Some("Recent entry"));
    }

    #[test]
    fn test_cleanup_preserves_recent() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Entry 1", "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Entry 2", "App").unwrap();

        let deleted = storage.cleanup_old_entries(30).unwrap();
        assert_eq!(deleted, 0);

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_cleanup_empty_db() {
        let storage = Storage::new_in_memory().unwrap();
        let deleted = storage.cleanup_old_entries(30).unwrap();
        assert_eq!(deleted, 0);
    }

    #[test]
    fn test_get_entries_before_with_cursor() {
        let storage = Storage::new_in_memory().unwrap();

        // Insert 5 entries with controlled timestamps
        let mut timestamps = Vec::new();
        for i in 0..5 {
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64 + i;

            storage.conn.execute(
                "INSERT INTO clipboard_entries (content_type, text_content, source_app, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![ContentType::PlainText.as_str(), format!("Entry {i}"), "App", ts],
            ).unwrap();
            timestamps.push(ts);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }

        // Get entries before the 3rd entry's timestamp (should return entries 0 and 1)
        let entries = storage.get_entries_before(timestamps[2], 10).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].text_content.as_deref(), Some("Entry 1"));
        assert_eq!(entries[1].text_content.as_deref(), Some("Entry 0"));
    }

    #[test]
    fn test_get_entries_before_zero_timestamp() {
        let storage = Storage::new_in_memory().unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Entry 1", "App").unwrap();
        storage.insert_text_entry(&ContentType::PlainText, "Entry 2", "App").unwrap();

        // before_timestamp=0 should behave like get_recent_entries
        let entries = storage.get_entries_before(0, 10).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_get_entries_before_empty_db() {
        let storage = Storage::new_in_memory().unwrap();
        let entries = storage.get_entries_before(999999999, 10).unwrap();
        assert_eq!(entries.len(), 0);
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

        // Second is on top
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries[0].text_content.as_deref(), Some("Second"));

        std::thread::sleep(std::time::Duration::from_millis(10));

        // Touch first, it should move to top
        storage.touch_entry(id1).unwrap();
        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries[0].text_content.as_deref(), Some("First"));
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

        // Insert 3 entries with same timestamp
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for i in 0..3 {
            storage.conn.execute(
                "INSERT INTO clipboard_entries (content_type, text_content, source_app, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![ContentType::PlainText.as_str(), format!("Entry {i}"), "App", ts],
            ).unwrap();
        }

        // Query with that exact timestamp should exclude all entries
        let entries = storage.get_entries_before(ts, 10).unwrap();
        assert_eq!(entries.len(), 0);

        // Query with timestamp+1 should include all entries
        let entries = storage.get_entries_before(ts + 1, 10).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_millisecond_precision_ordering() {
        let storage = Storage::new_in_memory().unwrap();
        // Insert entries with 1ms difference using direct SQL
        let base_ts: i64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;

        for i in 0..3 {
            storage.conn.execute(
                "INSERT INTO clipboard_entries (content_type, text_content, source_app, created_at, first_copied_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)",
                params![ContentType::PlainText.as_str(), format!("Entry {i}"), "App", base_ts + i],
            ).unwrap();
        }

        let entries = storage.get_recent_entries(10).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].text_content.as_deref(), Some("Entry 2"));
        assert_eq!(entries[2].text_content.as_deref(), Some("Entry 0"));
    }
}
