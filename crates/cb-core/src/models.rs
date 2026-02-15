use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
