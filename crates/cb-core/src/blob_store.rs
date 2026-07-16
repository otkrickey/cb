use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// 大きなコンテンツを SHA-256 名で外部ファイルに保存するストア。
///
/// パスは `<root>/<sha256hex>.bin`。同一内容の書き込みはファイルが既に
/// 存在するためスキップされ、自然に dedup される。
pub struct BlobStore {
    root: PathBuf,
}

impl BlobStore {
    /// ストアを初期化。ディレクトリが無ければ作成する。
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Self { root })
    }

    fn path_for(&self, sha256: &str) -> PathBuf {
        self.root.join(format!("{sha256}.bin"))
    }

    /// バイト列を書き込み、SHA-256 hex 文字列を返す。同一 sha256 のファイルが
    /// 既に存在すれば書き込みをスキップする（dedup）。
    pub fn write(&self, bytes: &[u8]) -> io::Result<String> {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        let sha256 = format!("{:x}", hasher.finalize());
        let path = self.path_for(&sha256);
        if !path.exists() {
            let tmp = path.with_extension("bin.tmp");
            fs::write(&tmp, bytes)?;
            fs::rename(&tmp, &path)?;
        }
        Ok(sha256)
    }

    /// SHA-256 指定でバイト列を読み出す。存在しなければ Ok(None)。
    pub fn read(&self, sha256: &str) -> io::Result<Option<Vec<u8>>> {
        let path = self.path_for(sha256);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn exists(&self, sha256: &str) -> bool {
        self.path_for(sha256).exists()
    }

    /// blob ファイルを 1 件削除する。存在しなければ false を返す。
    pub fn delete(&self, sha256: &str) -> io::Result<bool> {
        let path = self.path_for(sha256);
        match fs::remove_file(&path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e),
        }
    }

    /// ストア内の全 blob の SHA-256 一覧を返す（拡張子 .bin のみ、.tmp は除外）。
    pub fn list_all(&self) -> io::Result<Vec<String>> {
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if let Some(stem) = name.strip_suffix(".bin") {
                if is_hex_sha256(stem) {
                    out.push(stem.to_string());
                }
            }
        }
        Ok(out)
    }

    /// `referenced` に含まれない blob（孤児）を削除し、削除した SHA-256 を返す。
    ///
    /// blob 数 * 参照数 の線形探索を避け、`referenced` を HashSet に載せてから
    /// 判定する (PR #17 review 指摘)。
    pub fn gc_orphans(&self, referenced: &[String]) -> io::Result<Vec<String>> {
        let referenced: std::collections::HashSet<&str> =
            referenced.iter().map(|s| s.as_str()).collect();
        let all = self.list_all()?;
        let mut removed = Vec::new();
        for sha in all {
            if !referenced.contains(sha.as_str()) && self.delete(&sha)? {
                removed.push(sha);
            }
        }
        Ok(removed)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

fn is_hex_sha256(s: &str) -> bool {
    s.len() == 64 && s.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let pid = std::process::id();
        let path = std::env::temp_dir().join(format!("cb_blob_test_{tag}_{pid}_{n}"));
        let _ = fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn test_write_read_roundtrip() {
        let dir = temp_dir("roundtrip");
        let store = BlobStore::new(&dir).unwrap();

        let sha = store.write(b"hello, world").unwrap();
        assert_eq!(sha.len(), 64);
        assert!(store.exists(&sha));

        let read = store.read(&sha).unwrap();
        assert_eq!(read.as_deref(), Some(&b"hello, world"[..]));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_dedup_same_content() {
        let dir = temp_dir("dedup");
        let store = BlobStore::new(&dir).unwrap();

        let sha1 = store.write(b"same bytes").unwrap();
        let sha2 = store.write(b"same bytes").unwrap();
        assert_eq!(sha1, sha2);

        let all = store.list_all().unwrap();
        assert_eq!(all.len(), 1);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_read_missing_returns_none() {
        let dir = temp_dir("missing");
        let store = BlobStore::new(&dir).unwrap();

        let none_sha = "0".repeat(64);
        assert!(store.read(&none_sha).unwrap().is_none());
        assert!(!store.exists(&none_sha));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_delete() {
        let dir = temp_dir("delete");
        let store = BlobStore::new(&dir).unwrap();

        let sha = store.write(b"to be deleted").unwrap();
        assert!(store.exists(&sha));

        assert!(store.delete(&sha).unwrap());
        assert!(!store.exists(&sha));

        assert!(!store.delete(&sha).unwrap());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_gc_orphans() {
        let dir = temp_dir("gc");
        let store = BlobStore::new(&dir).unwrap();

        let keep = store.write(b"keep me").unwrap();
        let orphan1 = store.write(b"orphan A").unwrap();
        let orphan2 = store.write(b"orphan B").unwrap();

        let removed = store.gc_orphans(&[keep.clone()]).unwrap();
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&orphan1));
        assert!(removed.contains(&orphan2));

        assert!(store.exists(&keep));
        assert!(!store.exists(&orphan1));
        assert!(!store.exists(&orphan2));

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_list_ignores_non_blob_files() {
        let dir = temp_dir("listfilter");
        let store = BlobStore::new(&dir).unwrap();

        let sha = store.write(b"real").unwrap();
        fs::write(dir.join("garbage.txt"), b"noise").unwrap();
        fs::write(dir.join("not_sha.bin"), b"noise").unwrap();

        let all = store.list_all().unwrap();
        assert_eq!(all, vec![sha]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_write_then_read_binary() {
        let dir = temp_dir("binary");
        let store = BlobStore::new(&dir).unwrap();

        let payload: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        let sha = store.write(&payload).unwrap();
        let read = store.read(&sha).unwrap().unwrap();
        assert_eq!(read, payload);

        fs::remove_dir_all(&dir).ok();
    }
}
