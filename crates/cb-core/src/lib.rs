pub mod models;
pub mod storage;

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

#[swift_bridge::bridge]
mod ffi {
    extern "Rust" {
        fn init_storage(db_path: String, encryption_key: String) -> bool;
        fn migrate_database(plain_path: String, encrypted_path: String, encryption_key: String) -> bool;
        fn save_clipboard_entry(content_type: String, text: String, source_app: String) -> bool;
        fn save_clipboard_image(image_data: &[u8], source_app: String) -> bool;
        fn get_recent_entries(limit: i32) -> String;
        fn delete_entry(id: i64) -> bool;
        fn get_entry_text(id: i64) -> Option<String>;
        fn get_entry_image(id: i64) -> Option<Vec<u8>>;
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
                Err(e) => {
                    eprintln!("Storage lock poisoned: {e}");
                    return false;
                }
            };
            *guard = Some(s);
            true
        }
        Err(e) => {
            eprintln!("Failed to init storage: {e}");
            false
        }
    }
}

fn migrate_database(plain_path: String, encrypted_path: String, encryption_key: String) -> bool {
    match Storage::migrate_to_encrypted(&plain_path, &encrypted_path, &encryption_key) {
        Ok(_) => true,
        Err(e) => {
            eprintln!("Migration failed: {e}");
            false
        }
    }
}

fn save_clipboard_entry(content_type: String, text: String, source_app: String) -> bool {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return false;
        }
    };
    if let Some(ref storage) = *guard {
        let ct = ContentType::from_str(&content_type);
        match storage.insert_text_entry(&ct, &text, &source_app) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Failed to save entry: {e}");
                false
            }
        }
    } else {
        eprintln!("Storage not initialized");
        false
    }
}

fn save_clipboard_image(image_data: &[u8], source_app: String) -> bool {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return false;
        }
    };
    if let Some(ref storage) = *guard {
        match storage.insert_image_entry(image_data, &source_app) {
            Ok(_) => true,
            Err(e) => {
                eprintln!("Failed to save image: {e}");
                false
            }
        }
    } else {
        eprintln!("Storage not initialized");
        false
    }
}

fn get_recent_entries(limit: i32) -> String {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return json_error(&format!("Storage lock poisoned: {e}"));
        }
    };
    if let Some(ref storage) = *guard {
        match storage.get_recent_entries(limit) {
            Ok(entries) => json_ok(&entries),
            Err(e) => {
                eprintln!("Failed to get entries: {e}");
                json_error(&format!("Failed to get entries: {e}"))
            }
        }
    } else {
        json_error("Storage not initialized")
    }
}

fn delete_entry(id: i64) -> bool {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return false;
        }
    };
    if let Some(ref storage) = *guard {
        match storage.delete_entry(id) {
            Ok(deleted) => deleted,
            Err(e) => {
                eprintln!("Failed to delete entry: {e}");
                false
            }
        }
    } else {
        false
    }
}

fn get_entry_text(id: i64) -> Option<String> {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return None;
        }
    };
    if let Some(ref storage) = *guard {
        match storage.get_entry_text(id) {
            Ok(text) => text,
            Err(e) => {
                eprintln!("Failed to get entry text: {e}");
                None
            }
        }
    } else {
        None
    }
}

fn get_entry_image(id: i64) -> Option<Vec<u8>> {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return None;
        }
    };
    if let Some(ref storage) = *guard {
        match storage.get_entry_image(id) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("Failed to get entry image: {e}");
                None
            }
        }
    } else {
        None
    }
}

fn search_entries(query: String, limit: i32) -> String {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return json_error(&format!("Storage lock poisoned: {e}"));
        }
    };
    if let Some(ref storage) = *guard {
        match storage.search_entries(&query, limit) {
            Ok(entries) => json_ok(&entries),
            Err(e) => {
                eprintln!("Failed to search entries: {e}");
                json_error(&format!("Failed to search entries: {e}"))
            }
        }
    } else {
        json_error("Storage not initialized")
    }
}

fn get_entries_before(before_timestamp: i64, limit: i32) -> String {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return json_error(&format!("Storage lock poisoned: {e}"));
        }
    };
    if let Some(ref storage) = *guard {
        match storage.get_entries_before(before_timestamp, limit) {
            Ok(entries) => json_ok(&entries),
            Err(e) => {
                eprintln!("Failed to get entries before: {e}");
                json_error(&format!("Failed to get entries before: {e}"))
            }
        }
    } else {
        json_error("Storage not initialized")
    }
}

fn touch_entry(id: i64) -> bool {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return false;
        }
    };
    if let Some(ref storage) = *guard {
        match storage.touch_entry(id) {
            Ok(touched) => touched,
            Err(e) => {
                eprintln!("Failed to touch entry: {e}");
                false
            }
        }
    } else {
        eprintln!("Storage not initialized");
        false
    }
}

fn cleanup_old_entries(max_age_days: i32) -> i64 {
    let guard = match STORAGE.lock() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("Storage lock poisoned: {e}");
            return -1;
        }
    };
    if let Some(ref storage) = *guard {
        match storage.cleanup_old_entries(max_age_days) {
            Ok(count) => count as i64,
            Err(e) => {
                eprintln!("Failed to cleanup old entries: {e}");
                -1
            }
        }
    } else {
        -1
    }
}
