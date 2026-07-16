use serde::{Deserialize, Serialize};

/// UI 表示 + FTS5 index 対象となる preview の最大バイト長。
/// Swift 側の切り出しと SQLite 側 backfill の両方で参照する。
pub const PREVIEW_BYTES: usize = 8192;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    PlainText,
    RichText,
    Image,
    FilePath,
}

impl ContentType {
    pub fn as_str(&self) -> &str {
        match self {
            ContentType::PlainText => "PlainText",
            ContentType::RichText => "RichText",
            ContentType::Image => "Image",
            ContentType::FilePath => "FilePath",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "PlainText" => ContentType::PlainText,
            "RichText" => ContentType::RichText,
            "Image" => ContentType::Image,
            "FilePath" => ContentType::FilePath,
            _ => ContentType::PlainText,
        }
    }
}

/// クリップボード履歴エントリ。
///
/// フルコンテンツの保存場所は 2 通り（排他）:
/// - `text_content` が Some: DB 内 inline（小さいテキスト）
/// - `blob_sha256` が Some: 外部 blob ファイル参照（大きいテキスト・全画像）
///
/// 両方 None なら preview のみ生存（blob 欠損時の fallback 状態）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardEntry {
    pub id: i64,
    pub content_type: ContentType,
    pub text_preview: Option<String>,
    pub text_content: Option<String>,
    pub blob_sha256: Option<String>,
    pub byte_size: i64,
    #[serde(skip)]
    pub image_data: Option<Vec<u8>>,
    pub source_app: Option<String>,
    pub created_at: i64,
    pub copy_count: i64,
    pub first_copied_at: i64,
}
