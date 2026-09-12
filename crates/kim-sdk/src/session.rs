use std::sync::atomic::Ordering;

use tokio_util::sync::CancellationToken;

use crate::command::StartSession;
use crate::error::SdkError;
use crate::ids::SessionEpoch;
use crate::KimSdk;

impl KimSdk {
    pub(crate) fn current_epoch(&self) -> SessionEpoch {
        SessionEpoch(self.inner.epoch.load(Ordering::SeqCst))
    }

    pub(crate) fn bump_epoch(&self) -> (CancellationToken, SessionEpoch) {
        let old = {
            let mut cancel = lock(&self.inner.cancel);
            let old = cancel.clone();
            *cancel = CancellationToken::new();
            old
        };
        old.cancel();
        let next = self.inner.epoch.fetch_add(1, Ordering::SeqCst) + 1;
        (old, SessionEpoch(next))
    }

    #[allow(dead_code)]
    pub(crate) fn child_token(&self) -> CancellationToken {
        lock(&self.inner.cancel).child_token()
    }

    pub(crate) fn replace_session(&self, session: StartSession) {
        *lock(&self.inner.session) = Some(session);
    }

    pub(crate) fn session_snapshot(&self) -> Result<StartSession, SdkError> {
        lock(&self.inner.session)
            .clone()
            .ok_or(SdkError::InvalidArgument {
                message: "session not started".into(),
            })
    }

    #[allow(dead_code)]
    pub(crate) fn update_account(&self, account: String, token: String) -> Result<(), SdkError> {
        let mut session = lock(&self.inner.session);
        let Some(s) = session.as_mut() else {
            return Err(SdkError::InvalidArgument {
                message: "session not started".into(),
            });
        };
        s.account = account;
        s.token = token;
        Ok(())
    }
}

pub(crate) fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}
