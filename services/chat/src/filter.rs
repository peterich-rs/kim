//! Talk content filters. Each impl inspects one concern; [`FilterChain`] runs them in order.

use std::sync::Arc;

use async_trait::async_trait;
use kim_protocol::pkt::{MessageReq, Status};
use kim_protocol::{MESSAGE_TYPE_IMAGE, MESSAGE_TYPE_TEXT};

/// Inspect a talk payload before insert. `Ok(())` continues; `Err(status)` rejects.
#[async_trait]
pub trait ContentFilter: Send + Sync {
    async fn check(&self, req: &MessageReq) -> Result<(), Status>;
}

/// Default: accept every payload. Tests and unconfigured Chat use this.
pub struct NoopFilter;

#[async_trait]
impl ContentFilter for NoopFilter {
    async fn check(&self, _req: &MessageReq) -> Result<(), Status> {
        Ok(())
    }
}

/// Text intercept: `type=1` body substring match. Other types pass.
pub struct TextWordFilter {
    words: Vec<String>,
}

impl TextWordFilter {
    pub fn new(words: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            words: words
                .into_iter()
                .map(Into::into)
                .filter(|w| !w.is_empty())
                .collect(),
        }
    }
}

#[async_trait]
impl ContentFilter for TextWordFilter {
    async fn check(&self, req: &MessageReq) -> Result<(), Status> {
        if req.r#type != MESSAGE_TYPE_TEXT {
            return Ok(());
        }
        if self.words.iter().any(|w| req.body.contains(w.as_str())) {
            return Err(Status::ContentBlocked);
        }
        Ok(())
    }
}

/// Image intercept: `type=2` body / extra substring match (URL denylist). Other types pass.
pub struct ImageFilter {
    blocked: Vec<String>,
}

impl ImageFilter {
    pub fn new(blocked: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            blocked: blocked
                .into_iter()
                .map(Into::into)
                .filter(|w| !w.is_empty())
                .collect(),
        }
    }
}

#[async_trait]
impl ContentFilter for ImageFilter {
    async fn check(&self, req: &MessageReq) -> Result<(), Status> {
        if req.r#type != MESSAGE_TYPE_IMAGE {
            return Ok(());
        }
        if self
            .blocked
            .iter()
            .any(|w| req.body.contains(w.as_str()) || req.extra.contains(w.as_str()))
        {
            return Err(Status::ContentBlocked);
        }
        Ok(())
    }
}

/// Ordered list of filters. First `Err` wins. Empty chain behaves like [`NoopFilter`].
#[derive(Default)]
pub struct FilterChain {
    filters: Vec<Arc<dyn ContentFilter>>,
}

impl FilterChain {
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with<F: ContentFilter + 'static>(mut self, filter: F) -> Self {
        self.filters.push(Arc::new(filter));
        self
    }
}

#[async_trait]
impl ContentFilter for FilterChain {
    async fn check(&self, req: &MessageReq) -> Result<(), Status> {
        for filter in &self.filters {
            filter.check(req).await?;
        }
        Ok(())
    }
}

/// Production pipeline: text intercept then image intercept. Empty lists pass.
pub fn builtin_talk_filter(
    text_words: Vec<String>,
    image_blocked: Vec<String>,
) -> Arc<dyn ContentFilter> {
    Arc::new(
        FilterChain::new()
            .with(TextWordFilter::new(text_words))
            .with(ImageFilter::new(image_blocked)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kim_protocol::{MESSAGE_TYPE_VIDEO, MESSAGE_TYPE_VOICE};

    fn text(body: &str) -> MessageReq {
        MessageReq {
            r#type: MESSAGE_TYPE_TEXT,
            body: body.into(),
            extra: String::new(),
            client_id: String::new(),
        }
    }

    fn image(body: &str, extra: &str) -> MessageReq {
        MessageReq {
            r#type: MESSAGE_TYPE_IMAGE,
            body: body.into(),
            extra: extra.into(),
            client_id: String::new(),
        }
    }

    #[tokio::test]
    async fn noop_allows_everything() {
        let f = NoopFilter;
        assert!(f.check(&text("badword")).await.is_ok());
        assert!(f
            .check(&image("http://evil.example/x.png", ""))
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn text_filter_hits_only_text() {
        let f = TextWordFilter::new(["badword"]);
        assert_eq!(
            f.check(&text("say badword now")).await,
            Err(Status::ContentBlocked)
        );
        assert!(f.check(&text("hello")).await.is_ok());
        assert!(f.check(&image("badword.png", "")).await.is_ok());
        assert!(f
            .check(&MessageReq {
                r#type: MESSAGE_TYPE_VOICE,
                body: "badword".into(),
                extra: String::new(),
                client_id: String::new(),
            })
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn empty_text_list_passes() {
        let f = TextWordFilter::new(Vec::<String>::new());
        assert!(f.check(&text("anything")).await.is_ok());
    }

    #[tokio::test]
    async fn image_filter_hits_body_or_extra() {
        let f = ImageFilter::new(["evil.example"]);
        assert_eq!(
            f.check(&image("http://evil.example/a.png", "")).await,
            Err(Status::ContentBlocked)
        );
        assert_eq!(
            f.check(&image("ok.png", "cdn=evil.example")).await,
            Err(Status::ContentBlocked)
        );
        assert!(f.check(&image("http://cdn.ok/a.png", "")).await.is_ok());
        assert!(f.check(&text("http://evil.example/a.png")).await.is_ok());
        assert!(f
            .check(&MessageReq {
                r#type: MESSAGE_TYPE_VIDEO,
                body: "http://evil.example/a.mp4".into(),
                extra: String::new(),
                client_id: String::new(),
            })
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn chain_runs_text_then_image() {
        let chain = FilterChain::new()
            .with(TextWordFilter::new(["badword"]))
            .with(ImageFilter::new(["evil.example"]));
        assert_eq!(
            chain.check(&text("badword")).await,
            Err(Status::ContentBlocked)
        );
        assert_eq!(
            chain.check(&image("http://evil.example/a.png", "")).await,
            Err(Status::ContentBlocked)
        );
        assert!(chain.check(&text("hello")).await.is_ok());
    }

    #[tokio::test]
    async fn builtin_empty_lists_pass() {
        let f = builtin_talk_filter(Vec::new(), Vec::new());
        assert!(f.check(&text("hello")).await.is_ok());
        assert!(f.check(&image("ok.png", "")).await.is_ok());
    }
}
