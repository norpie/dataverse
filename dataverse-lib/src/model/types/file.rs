//! File and Image reference types

use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

/// A reference to a file stored in a file column.
///
/// File columns in Dataverse store the file content separately and return
/// metadata about the file. Use the file APIs to download/upload actual content.
///
/// # Example
///
/// ```
/// use dataverse_lib::model::types::FileReference;
/// use uuid::Uuid;
///
/// let file_ref = FileReference {
///     id: Uuid::new_v4(),
///     file_name: Some("document.pdf".to_string()),
///     file_size: Some(1024),
///     mime_type: Some("application/pdf".to_string()),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileReference {
    /// The unique identifier of the file.
    pub id: Uuid,
    /// The original file name, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// The file size in bytes, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    /// The MIME type of the file, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

impl FileReference {
    /// Creates a new file reference with just the ID.
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            file_name: None,
            file_size: None,
            mime_type: None,
        }
    }

    /// Creates a new file reference with metadata.
    pub fn with_metadata(
        id: Uuid,
        file_name: impl Into<String>,
        file_size: i64,
        mime_type: impl Into<String>,
    ) -> Self {
        Self {
            id,
            file_name: Some(file_name.into()),
            file_size: Some(file_size),
            mime_type: Some(mime_type.into()),
        }
    }
}

/// A reference to an image stored in an image column.
///
/// Image columns in Dataverse can store both the image content and a thumbnail.
/// Use the image APIs to download/upload actual content.
///
/// # Example
///
/// ```
/// use dataverse_lib::model::types::ImageReference;
/// use uuid::Uuid;
///
/// let image_ref = ImageReference {
///     id: Uuid::new_v4(),
///     file_name: Some("photo.jpg".to_string()),
///     file_size: Some(2048),
///     mime_type: Some("image/jpeg".to_string()),
///     width: Some(800),
///     height: Some(600),
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageReference {
    /// The unique identifier of the image.
    pub id: Uuid,
    /// The original file name, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    /// The file size in bytes, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_size: Option<i64>,
    /// The MIME type of the image, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// The image width in pixels, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<i32>,
    /// The image height in pixels, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<i32>,
}

impl ImageReference {
    /// Creates a new image reference with just the ID.
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            file_name: None,
            file_size: None,
            mime_type: None,
            width: None,
            height: None,
        }
    }

    /// Creates a new image reference with metadata.
    pub fn with_metadata(
        id: Uuid,
        file_name: impl Into<String>,
        file_size: i64,
        mime_type: impl Into<String>,
    ) -> Self {
        Self {
            id,
            file_name: Some(file_name.into()),
            file_size: Some(file_size),
            mime_type: Some(mime_type.into()),
            width: None,
            height: None,
        }
    }

    /// Creates a new image reference with dimensions.
    pub fn with_dimensions(
        id: Uuid,
        file_name: impl Into<String>,
        file_size: i64,
        mime_type: impl Into<String>,
        width: i32,
        height: i32,
    ) -> Self {
        Self {
            id,
            file_name: Some(file_name.into()),
            file_size: Some(file_size),
            mime_type: Some(mime_type.into()),
            width: Some(width),
            height: Some(height),
        }
    }
}
