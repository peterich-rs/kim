use std::path::{Component, Path};
use std::sync::Arc;

use tokio::sync::Semaphore;

use crate::error::SdkError;

#[derive(Clone, Debug)]
pub struct MediaRef {
    pub path: String,
    pub mime: String,
    pub width: i32,
    pub height: i32,
    pub byte_size: i64,
}

pub const MAX_IMAGE_BYTES: i64 = 5 * 1024 * 1024;
pub const UPLOAD_ORIGIN: &str = "https://upload.kim.ainexc.com";

#[must_use]
pub fn image_mime_ok(mime: &str) -> bool {
    matches!(
        mime,
        "image/jpeg" | "image/jpg" | "image/png" | "image/webp" | "image/gif"
    )
}

pub fn validate_media_path(path: &str) -> Result<(), SdkError> {
    if path.is_empty() {
        return Err(SdkError::InvalidArgument {
            message: "media path is required".into(),
        });
    }
    let p = Path::new(path);
    if !p.is_absolute() {
        return Err(SdkError::InvalidArgument {
            message: "media path must be absolute".into(),
        });
    }
    if p.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(SdkError::InvalidArgument {
            message: "media path must not contain ..".into(),
        });
    }
    Ok(())
}

pub struct MediaUploader {
    origin: String,
    inflight: Arc<Semaphore>,
    client: reqwest::Client,
}

impl MediaUploader {
    pub fn new(origin: impl Into<String>) -> Result<Self, SdkError> {
        let client = reqwest::Client::builder()
            .use_rustls_tls()
            .build()
            .map_err(|e| SdkError::Internal {
                message: format!("reqwest: {e}"),
            })?;
        Ok(Self {
            origin: origin.into(),
            inflight: Arc::new(Semaphore::new(2)),
            client,
        })
    }

    pub async fn upload_image(&self, token: &str, media: &MediaRef) -> Result<String, SdkError> {
        validate_media_path(&media.path)?;
        let mime = if media.mime == "image/jpg" {
            "image/jpeg"
        } else {
            media.mime.as_str()
        };
        if !image_mime_ok(mime) {
            return Err(SdkError::UnsupportedMedia {
                mime: media.mime.clone(),
            });
        }
        let meta =
            tokio::fs::metadata(&media.path)
                .await
                .map_err(|_| SdkError::InvalidArgument {
                    message: "media file missing".into(),
                })?;
        let size = i64::try_from(meta.len()).unwrap_or(i64::MAX);
        if size > MAX_IMAGE_BYTES {
            return Err(SdkError::PayloadTooLarge {
                bytes: size,
                max: MAX_IMAGE_BYTES,
            });
        }
        let _permit = self.inflight.acquire().await.map_err(|_| SdkError::Busy {
            queue: "upload".into(),
        })?;
        let file =
            tokio::fs::File::open(&media.path)
                .await
                .map_err(|_| SdkError::InvalidArgument {
                    message: "media file missing".into(),
                })?;
        let stream = tokio_util::io::ReaderStream::new(file);
        let body = reqwest::Body::wrap_stream(stream);
        let url = format!("{}/v1/objects", self.origin.trim_end_matches('/'));
        let resp = self
            .client
            .post(url)
            .bearer_auth(token)
            .header(reqwest::header::CONTENT_TYPE, mime)
            .header(reqwest::header::CONTENT_LENGTH, meta.len())
            .body(body)
            .send()
            .await
            .map_err(|e| SdkError::Internal {
                message: format!("upload: {e}"),
            })?;
        if !resp.status().is_success() {
            return Err(SdkError::Protocol {
                status: i32::from(resp.status().as_u16()),
            });
        }
        let v: serde_json::Value = resp.json().await.map_err(|_| SdkError::Internal {
            message: "upload: bad response".into(),
        })?;
        v.get("url")
            .and_then(|u| u.as_str())
            .map(str::to_string)
            .ok_or(SdkError::Internal {
                message: "upload: missing url".into(),
            })
    }
}
