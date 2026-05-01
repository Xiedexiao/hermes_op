use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RunId(pub(crate) u64);

#[derive(Debug, Clone)]
pub(crate) struct CancelToken {
    cancelled: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
pub(crate) struct ForegroundRunHandle {
    status: ActiveRunStatus,
    cancel_token: CancelToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActiveRunCancelState {
    Active,
    CancelRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActiveRunStatus {
    pub(crate) run_id: RunId,
    pub(crate) cancel_state: ActiveRunCancelState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForegroundRuntimeError {
    AlreadyRunning,
    NotRunning,
}

#[derive(Debug)]
pub(crate) struct ForegroundRuntime {
    next_run_id: u64,
    active_run: Option<ForegroundRunHandle>,
}

impl CancelToken {
    #[allow(dead_code)]
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

impl Default for CancelToken {
    fn default() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ForegroundRunHandle {
    pub(crate) fn run_id(&self) -> RunId {
        self.status.run_id
    }

    #[allow(dead_code)]
    pub(crate) fn cancel_token(&self) -> CancelToken {
        self.cancel_token.clone()
    }

    pub(crate) fn status(&self) -> ActiveRunStatus {
        self.status
    }

    #[allow(dead_code)]
    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

impl ForegroundRuntime {
    pub(crate) fn new() -> Self {
        Self {
            next_run_id: 1,
            active_run: None,
        }
    }

    pub(crate) fn active_run_id(&self) -> Option<RunId> {
        self.active_run.as_ref().map(ForegroundRunHandle::run_id)
    }

    pub(crate) fn active_run_handle(&self) -> Option<ForegroundRunHandle> {
        self.active_run.clone()
    }

    pub(crate) fn active_run_status(&self) -> Option<ActiveRunStatus> {
        self.active_run.as_ref().map(ForegroundRunHandle::status)
    }

    #[allow(dead_code)]
    pub(crate) fn interrupted(&self) -> bool {
        matches!(
            self.active_run_status(),
            Some(ActiveRunStatus {
                cancel_state: ActiveRunCancelState::CancelRequested,
                ..
            })
        )
    }

    pub(crate) fn start_run(&mut self) -> Result<ForegroundRunHandle, ForegroundRuntimeError> {
        if self.active_run.is_some() {
            return Err(ForegroundRuntimeError::AlreadyRunning);
        }

        let handle = ForegroundRunHandle {
            status: ActiveRunStatus {
                run_id: RunId(self.next_run_id),
                cancel_state: ActiveRunCancelState::Active,
            },
            cancel_token: CancelToken::default(),
        };
        self.next_run_id += 1;
        self.active_run = Some(handle.clone());
        Ok(handle)
    }

    pub(crate) fn finish_run(&mut self, run_id: RunId) -> Result<(), ForegroundRuntimeError> {
        if self.active_run_id() != Some(run_id) {
            return Err(ForegroundRuntimeError::NotRunning);
        }

        self.active_run = None;
        Ok(())
    }

    pub(crate) fn request_cancel(&mut self) -> Result<ForegroundRunHandle, ForegroundRuntimeError> {
        let handle = self
            .active_run
            .as_mut()
            .ok_or(ForegroundRuntimeError::NotRunning)?;
        handle.status.cancel_state = ActiveRunCancelState::CancelRequested;
        handle.cancel_token.cancel();
        Ok(handle.clone())
    }

    pub(crate) fn clear_cancel_state(
        &mut self,
        run_id: RunId,
    ) -> Result<ForegroundRunHandle, ForegroundRuntimeError> {
        let handle = self
            .active_run
            .as_mut()
            .ok_or(ForegroundRuntimeError::NotRunning)?;
        if handle.run_id() != run_id {
            return Err(ForegroundRuntimeError::NotRunning);
        }

        handle.status.cancel_state = ActiveRunCancelState::Active;
        Ok(handle.clone())
    }
}

impl Default for ForegroundRuntime {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn merge_interrupt_requests<I, S>(requests: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let parts = requests
        .into_iter()
        .map(|part| part.as_ref().trim().to_string())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_run_handle_tracks_run_lifecycle() {
        let mut runtime = ForegroundRuntime::new();

        assert_eq!(runtime.active_run_id(), None);
        assert!(runtime.active_run_handle().is_none());

        let first_run = runtime.start_run().expect("first run starts");

        assert_eq!(runtime.active_run_id(), Some(first_run.run_id()));
        assert_eq!(
            runtime.active_run_handle().map(|handle| handle.run_id()),
            Some(first_run.run_id())
        );

        runtime
            .finish_run(first_run.run_id())
            .expect("first run finishes");

        assert_eq!(runtime.active_run_id(), None);
        assert!(runtime.active_run_handle().is_none());

        let second_run = runtime.start_run().expect("second run starts");
        assert_ne!(first_run.run_id(), second_run.run_id());
    }

    #[test]
    fn request_cancel_sets_token_and_interrupt_flag() {
        let mut runtime = ForegroundRuntime::new();
        let handle = runtime.start_run().expect("run starts");

        assert!(!handle.is_cancelled());
        assert!(!runtime.interrupted());

        let cancelled = runtime.request_cancel().expect("cancel request succeeds");

        assert_eq!(cancelled.run_id(), handle.run_id());
        assert!(handle.is_cancelled());
        assert!(runtime.interrupted());
    }

    #[test]
    fn clear_cancel_state_keeps_run_active_but_resets_interrupt_flag() {
        let mut runtime = ForegroundRuntime::new();
        let handle = runtime.start_run().expect("run starts");
        runtime.request_cancel().expect("cancel succeeds");

        let restored = runtime
            .clear_cancel_state(handle.run_id())
            .expect("clear succeeds");

        assert_eq!(
            restored.status(),
            ActiveRunStatus {
                run_id: handle.run_id(),
                cancel_state: ActiveRunCancelState::Active,
            }
        );
        assert!(!runtime.interrupted());
        assert_eq!(runtime.active_run_id(), Some(handle.run_id()));
    }

    #[test]
    fn merge_interrupt_requests_preserves_arrival_order() {
        assert_eq!(
            merge_interrupt_requests([
                "first interrupt",
                "/interrupt second interrupt",
                "/stop third interrupt",
            ]),
            Some("first interrupt\n/interrupt second interrupt\n/stop third interrupt".to_string())
        );
    }
}
