//! Pure session-phase machine. Side effects (yield-wait, events) stay in `session`.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum SessionPhase {
    Idle,
    Running,
    Yielded,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum PhaseInput {
    Prompt,
    Yield,
    Abort,
    Finish,
    Continue,
}

impl SessionPhase {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Running => "running",
            Self::Yielded => "yielded",
        }
    }
}

/// `None` means the input is illegal in `current` (caller keeps the phase and returns an error).
pub(crate) fn transition(current: SessionPhase, input: PhaseInput) -> Option<SessionPhase> {
    match (current, input) {
        (SessionPhase::Idle, PhaseInput::Prompt) => Some(SessionPhase::Running),
        (SessionPhase::Running, PhaseInput::Yield) => Some(SessionPhase::Yielded),
        (_, PhaseInput::Abort) => Some(SessionPhase::Idle),
        (SessionPhase::Running | SessionPhase::Yielded, PhaseInput::Finish) => {
            Some(SessionPhase::Idle)
        }
        (SessionPhase::Yielded, PhaseInput::Continue) => Some(SessionPhase::Running),
        _ => None,
    }
}

pub(crate) fn stale(current_gen: u64, event_gen: u64) -> bool {
    current_gen != event_gen
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_prompt_runs_then_yield_then_abort() {
        let running = transition(SessionPhase::Idle, PhaseInput::Prompt).unwrap();
        assert_eq!(running, SessionPhase::Running);
        let yielded = transition(running, PhaseInput::Yield).unwrap();
        assert_eq!(yielded, SessionPhase::Yielded);
        assert_eq!(
            transition(yielded, PhaseInput::Abort),
            Some(SessionPhase::Idle)
        );
        assert!(transition(SessionPhase::Running, PhaseInput::Prompt).is_none());
        assert!(stale(2, 1));
        assert!(!stale(3, 3));
    }
}
