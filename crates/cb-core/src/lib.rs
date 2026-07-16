pub mod models;
pub mod storage;
pub mod blob_store;

use std::sync::Mutex;
use storage::Storage;
use models::ContentType;

static STORAGE: Mutex<Option<Storage>> = Mutex::new(None);

fn json_ok<T: serde::Serialize>(data: &T) -> String {
    match serde_json::to_string(data) {
        Ok(json) => format!("{{\"ok\":{}}}", json),
        Err(e) => {
            eprintln!("Failed to serialize: {e}");
            json_error(&format!("Serialization failed: {e}"))
        }
    }
}

fn json_error(msg: &str) -> String {
    format!("{{\"error\":{}}}", serde_json::to_string(msg).unwrap_or_else(|_| "\"unknown error\"".to_string()))
}

fn opt_str(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        fn init_storage(db_path: String, encryption_key: String) -> bool;
        fn migrate_database(plain_path: String, encrypted_path: String, encryption_key: String) -> bool;
        // 旧 FFI (BC 維持): Rust 側で自動 externalize
        fn save_clipboard_entry(content_type: String, text: String, source_app: String) -> bool;
        fn save_clipboard_image(image_data: &[u8], source_app: String) -> bool;
        // 新 FFI: Swift 側で外部化判断・SHA-256 計算・blob 書込み済み
        fn save_clipboard_text_v2(
            content_type: String,
            preview: String,
            text_content_or_empty: String,
            blob_sha256_or_empty: String,
            byte_size: i64,
            source_app: String,
        ) -> bool;
        fn save_clipboard_blob_ref(
            content_type: String,
            preview: String,
            blob_sha256: String,
            byte_size: i64,
            source_app: String,
        ) -> bool;
        fn get_recent_entries(limit: i32) -> String;
        fn delete_entry(id: i64) -> bool;
        fn get_entry_text(id: i64) -> Option<String>;
        fn get_entry_image(id: i64) -> Option<Vec<u8>>;
        fn get_entry_blob_sha256(id: i64) -> Option<String>;
        fn is_blob_missing(id: i64) -> bool;
        fn blob_dir_path() -> Option<String>;
        fn search_entries(query: String, limit: i32) -> String;
        fn get_entries_before(before_timestamp: i64, limit: i32) -> String;
        fn touch_entry(id: i64) -> bool;
        fn cleanup_old_entries(max_age_days: i32) -> i64;
    }
}

fn init_storage(db_path: String, encryption_key: String) -> bool {
    let key = if encryption_key.is_empty() { None } else { Some(encryption_key.as_str()) };
    match Storage::new(&db_path, key) {
        Ok(s) => {
            let mut guard = match STORAGE.lock() {
                Ok(g) => g,
                Err(e) => { eprintln!("Storage lock poisoned: {e}"); return false; }
            };
            *guard = Some(s);
            true
        }
        Err(e) => { eprintln!("Failed to init storage: {e}"); false }
    }
}

fn migrate_database(plain_path: String, encrypted_path: String, encryption_key: String) -> bool {
    match Storage::migrate_to_encrypted(&plain_path, &encrypted_path, &encryption_key) {
        Ok(_) => true,
        Err(e) => { eprintln!("Migration failed: {e}"); false }
    }
}

fn save_clipboard_entry(content_type: String, text: String, source_app: String) -> bool {
    with_storage(|storage| {
        let ct = ContentType::from_str(&content_type);
        match storage.insert_text_entry(&ct, &text, &source_app) {
            Ok(_) => true,
            Err(e) => { eprintln!("Failed to save entry: {e}"); false }
        }
    })
}

fn save_clipboard_image(image_data: &[u8], source_app: String) -> bool {
    with_storage(|storage| {
        match storage.insert_image_entry(image_data, &source_app) {
            Ok(_) => true,
            Err(e) => { eprintln!("Failed to save image: {e}"); false }
        }
    })
}

fn save_clipboard_text_v2(
    content_type: String,
    preview: String,
    text_content_or_empty: String,
    blob_sha256_or_empty: String,
    byte_size: i64,
    source_app: String,
) -> bool {
    with_storage(|storage| {
        let ct = ContentType::from_str(&content_type);
        match storage.insert_prepared_text(
            &ct,
            opt_str(&preview),
            opt_str(&text_content_or_empty),
            opt_str(&blob_sha256_or_empty),
            byte_size,
            &source_app,
        ) {
            Ok(_) => true,
            Err(e) => { eprintln!("Failed to save entry (v2): {e}"); false }
        }
    })
}

fn save_clipboard_blob_ref(
    content_type: String,
    preview: String,
    blob_sha256: String,
    byte_size: i64,
    source_app: String,
) -> bool {
    with_storage(|storage| {
        let ct = ContentType::from_str(&content_type);
        match storage.insert_prepared_blob_ref(&ct, opt_str(&preview), &blob_sha256, byte_size, &source_app) {
            Ok(_) => true,
            Err(e) => { eprintln!("Failed to save blob ref: {e}"); false }
        }
    })
}

fn get_recent_entries(limit: i32) -> String {
    with_storage_str(|storage| match storage.get_recent_entries(limit) {
        Ok(entries) => json_ok(&entries),
        Err(e) => { eprintln!("Failed to get entries: {e}"); json_error(&format!("Failed to get entries: {e}")) }
    })
}

fn delete_entry(id: i64) -> bool {
    with_storage(|storage| match storage.delete_entry(id) {
        Ok(deleted) => deleted,
        Err(e) => { eprintln!("Failed to delete entry: {e}"); false }
    })
}

fn get_entry_text(id: i64) -> Option<String> {
    with_storage_opt(|storage| match storage.get_entry_text(id) {
        Ok(text) => text,
        Err(e) => { eprintln!("Failed to get entry text: {e}"); None }
    })
}

fn get_entry_image(id: i64) -> Option<Vec<u8>> {
    with_storage_opt(|storage| match storage.get_entry_image(id) {
        Ok(data) => data,
        Err(e) => { eprintln!("Failed to get entry image: {e}"); None }
    })
}

fn get_entry_blob_sha256(id: i64) -> Option<String> {
    with_storage_opt(|storage| match storage.get_entry_blob_sha256(id) {
        Ok(v) => v,
        Err(e) => { eprintln!("Failed to get blob sha256: {e}"); None }
    })
}

fn is_blob_missing(id: i64) -> bool {
    with_storage(|storage| match storage.is_blob_missing(id) {
        Ok(v) => v,
        Err(e) => { eprintln!("Failed to check blob missing: {e}"); false }
    })
}

/// blob 保管ディレクトリの絶対パスを返す。
///
/// Storage 未初期化時 / lock poisoning 時は `None` を返す。以前は
/// `with_storage_str` を流用しており `{"error":"..."}` という JSON 文字列を
/// パスとして返してしまい、呼び出し側が実在しないディレクトリを掴んで
/// サイレント失敗する不具合があった (PR #17 review 指摘)。
fn blob_dir_path() -> Option<String> {
    with_storage_opt(|storage| Some(storage.blob_store().root().to_string_lossy().to_string()))
}

fn search_entries(query: String, limit: i32) -> String {
    with_storage_str(|storage| match storage.search_entries(&query, limit) {
        Ok(entries) => json_ok(&entries),
        Err(e) => { eprintln!("Failed to search entries: {e}"); json_error(&format!("Failed to search entries: {e}")) }
    })
}

fn get_entries_before(before_timestamp: i64, limit: i32) -> String {
    with_storage_str(|storage| match storage.get_entries_before(before_timestamp, limit) {
        Ok(entries) => json_ok(&entries),
        Err(e) => { eprintln!("Failed to get entries before: {e}"); json_error(&format!("Failed to get entries before: {e}")) }
    })
}

fn touch_entry(id: i64) -> bool {
    with_storage(|storage| match storage.touch_entry(id) {
        Ok(v) => v,
        Err(e) => { eprintln!("Failed to touch entry: {e}"); false }
    })
}

fn cleanup_old_entries(max_age_days: i32) -> i64 {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => { eprintln!("Storage lock poisoned: {e}"); return -1; }
    };
    let Some(ref storage) = *guard else { return -1; };
    match storage.cleanup_old_entries(max_age_days) {
        Ok((entries, blobs)) => {
            eprintln!("cleanup: {entries} entries, {blobs} orphan blobs removed");
            entries as i64
        }
        Err(e) => { eprintln!("Failed to cleanup old entries: {e}"); -1 }
    }
}

// ─────────────────────────────────────────────────
// Storage lock ラッパ
// ─────────────────────────────────────────────────

fn with_storage<F: FnOnce(&Storage) -> bool>(f: F) -> bool {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => { eprintln!("Storage lock poisoned: {e}"); return false; }
    };
    match *guard {
        Some(ref s) => f(s),
        None => { eprintln!("Storage not initialized"); false }
    }
}

fn with_storage_str<F: FnOnce(&Storage) -> String>(f: F) -> String {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => return json_error(&format!("Storage lock poisoned: {e}")),
    };
    match *guard {
        Some(ref s) => f(s),
        None => json_error("Storage not initialized"),
    }
}

fn with_storage_opt<T, F: FnOnce(&Storage) -> Option<T>>(f: F) -> Option<T> {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => { eprintln!("Storage lock poisoned: {e}"); return None; }
    };
    match *guard {
        Some(ref s) => f(s),
        None => None,
    }
}
