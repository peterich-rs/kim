#[derive(Clone, Debug)]
pub struct MediaRef {
    pub path: String,
    pub mime: String,
    pub width: i32,
    pub height: i32,
    pub byte_size: i64,
}

pub const MAX_IMAGE_BYTES: i64 = 5 * 1024 * 1024;

#[must_use]
pub fn image_mime_ok(mime: &str) -> bool {
    matches!(
        mime,
        "image/jpeg" | "image/jpg" | "image/png" | "image/webp" | "image/gif"
    )
}
