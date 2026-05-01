use std::collections::VecDeque;

use super::foreground_runtime::ForegroundRuntimeError;
pub(crate) use super::foreground_runtime::{
    ActiveRunCancelState, ActiveRunStatus, ForegroundRunHandle, ForegroundRuntime, RunId,
    merge_interrupt_requests,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ForegroundTurnState {
    #[default]
    Idle,
    Running,
    Interrupting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum BusyInputMode {
    #[default]
    Queue,
    Interrupt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ForegroundInputKind {
    PlainText,
    SlashCommand,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PendingPrompt {
    pub(crate) text: String,
    pub(crate) kind: ForegroundInputKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InterruptRequest {
    pub(crate) text: String,
    pub(crate) kind: ForegroundInputKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BusySubmitOutcome {
    QueuedPrompt(PendingPrompt),
    RecordedInterrupt(InterruptRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ForegroundControllerError {
    AlreadyRunning,
    NotRunning,
}

#[derive(Debug)]
pub(crate) struct ForegroundTurnController {
    busy_input_mode: BusyInputMode,
    runtime: ForegroundRuntime,
    pending_prompts: VecDeque<PendingPrompt>,
    interrupt_requests: VecDeque<InterruptRequest>,
}

impl PendingPrompt {
    fn new(text: impl Into<String>, kind: ForegroundInputKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }
}

impl InterruptRequest {
    fn new(text: impl Into<String>, kind: ForegroundInputKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }
}

impl ForegroundTurnController {
    pub(crate) fn new(busy_input_mode: BusyInputMode) -> Self {
        Self {
            busy_input_mode,
            runtime: ForegroundRuntime::new(),
            pending_prompts: VecDeque::new(),
            interrupt_requests: VecDeque::new(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn state(&self) -> ForegroundTurnState {
        match self.runtime.active_run_status() {
            None => ForegroundTurnState::Idle,
            Some(ActiveRunStatus {
                cancel_state: ActiveRunCancelState::CancelRequested,
                ..
            }) => ForegroundTurnState::Interrupting,
            Some(_) => ForegroundTurnState::Running,
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn active_run_id(&self) -> Option<RunId> {
        self.runtime.active_run_id()
    }

    #[allow(dead_code)]
    pub(crate) fn active_run_handle(&self) -> Option<ForegroundRunHandle> {
        self.runtime.active_run_handle()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn active_run_status(&self) -> Option<ActiveRunStatus> {
        self.runtime.active_run_status()
    }

    pub(crate) fn start_run(&mut self) -> Result<RunId, ForegroundControllerError> {
        let handle = self
            .runtime
            .start_run()
            .map_err(ForegroundControllerError::from)?;
        Ok(handle.run_id())
    }

    pub(crate) fn finish_run(&mut self, run_id: RunId) -> Result<(), ForegroundControllerError> {
        self.runtime
            .finish_run(run_id)
            .map_err(ForegroundControllerError::from)
    }

    pub(crate) fn submit_text_while_busy(
        &mut self,
        text: impl Into<String>,
    ) -> Result<BusySubmitOutcome, ForegroundControllerError> {
        self.submit_busy_input(text, ForegroundInputKind::PlainText)
    }

    pub(crate) fn submit_slash_while_busy(
        &mut self,
        slash: impl Into<String>,
    ) -> Result<BusySubmitOutcome, ForegroundControllerError> {
        self.submit_busy_input(slash, ForegroundInputKind::SlashCommand)
    }

    pub(crate) fn take_next_pending_prompt(&mut self) -> Option<PendingPrompt> {
        self.pending_prompts.pop_front()
    }

    pub(crate) fn take_next_interrupt_request(&mut self) -> Option<InterruptRequest> {
        self.interrupt_requests.pop_front()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn take_next_interrupt_prompt(&mut self) -> Option<String> {
        let requests = self.interrupt_requests.drain(..).collect::<Vec<_>>();
        merge_interrupt_requests(requests.iter().map(|request| request.text.as_str()))
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn pending_len(&self) -> usize {
        self.pending_prompts.len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn interrupt_len(&self) -> usize {
        self.interrupt_requests.len()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn clear_active_run_cancel_state(
        &mut self,
        run_id: RunId,
    ) -> Result<(), ForegroundControllerError> {
        self.runtime
            .clear_cancel_state(run_id)
            .map_err(ForegroundControllerError::from)?;
        Ok(())
    }

    pub(crate) fn record_interrupt_request(
        &mut self,
        request: InterruptRequest,
    ) -> Result<(), ForegroundControllerError> {
        self.runtime
            .request_cancel()
            .map_err(ForegroundControllerError::from)?;
        self.interrupt_requests.push_back(request);
        Ok(())
    }

    fn submit_busy_input(
        &mut self,
        input: impl Into<String>,
        kind: ForegroundInputKind,
    ) -> Result<BusySubmitOutcome, ForegroundControllerError> {
        if self.runtime.active_run_id().is_none() {
            return Err(ForegroundControllerError::NotRunning);
        }

        let input = input.into();
        if self.should_record_interrupt(&input, kind) {
            let request = InterruptRequest::new(input, kind);
            self.record_interrupt_request(request.clone())?;
            return Ok(BusySubmitOutcome::RecordedInterrupt(request));
        }

        let prompt = PendingPrompt::new(input, kind);
        self.pending_prompts.push_back(prompt.clone());
        Ok(BusySubmitOutcome::QueuedPrompt(prompt))
    }

    fn should_record_interrupt(&self, input: &str, kind: ForegroundInputKind) -> bool {
        matches!(kind, ForegroundInputKind::PlainText)
            && matches!(self.busy_input_mode, BusyInputMode::Interrupt)
            || matches!(kind, ForegroundInputKind::SlashCommand) && is_interrupt_slash(input)
    }
}

fn is_interrupt_slash(input: &str) -> bool {
    let command = input
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_start_matches('/');

    matches!(command, "interrupt" | "stop" | "cancel")
}

impl Default for ForegroundTurnController {
    fn default() -> Self {
        Self::new(BusyInputMode::default())
    }
}

impl From<ForegroundRuntimeError> for ForegroundControllerError {
    fn from(value: ForegroundRuntimeError) -> Self {
        match value {
            ForegroundRuntimeError::AlreadyRunning => Self::AlreadyRunning,
            ForegroundRuntimeError::NotRunning => Self::NotRunning,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_mode_routes_busy_plain_text_and_non_interrupt_slash_to_pending_queue() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Queue);
        controller.start_run().expect("run starts");

        let plain = controller
            .submit_text_while_busy("follow-up prompt")
            .expect("plain text routes");
        let slash = controller
            .submit_slash_while_busy("/status")
            .expect("slash routes");

        assert_eq!(
            plain,
            BusySubmitOutcome::QueuedPrompt(PendingPrompt {
                text: "follow-up prompt".to_string(),
                kind: ForegroundInputKind::PlainText,
            })
        );
        assert_eq!(
            slash,
            BusySubmitOutcome::QueuedPrompt(PendingPrompt {
                text: "/status".to_string(),
                kind: ForegroundInputKind::SlashCommand,
            })
        );
        assert_eq!(controller.state(), ForegroundTurnState::Running);
        assert_eq!(controller.pending_len(), 2);
        assert_eq!(controller.interrupt_len(), 0);
    }

    #[test]
    fn interrupt_mode_routes_busy_plain_text_and_slash_to_interrupt_queue() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Interrupt);
        controller.start_run().expect("run starts");

        let plain = controller
            .submit_text_while_busy("please stop after this turn")
            .expect("plain text routes");
        let slash = controller
            .submit_slash_while_busy("/status")
            .expect("slash routes");

        assert_eq!(
            plain,
            BusySubmitOutcome::RecordedInterrupt(InterruptRequest {
                text: "please stop after this turn".to_string(),
                kind: ForegroundInputKind::PlainText,
            })
        );
        assert_eq!(
            slash,
            BusySubmitOutcome::QueuedPrompt(PendingPrompt {
                text: "/status".to_string(),
                kind: ForegroundInputKind::SlashCommand,
            })
        );
        assert_eq!(controller.state(), ForegroundTurnState::Interrupting);
        assert_eq!(controller.pending_len(), 1);
        assert_eq!(controller.interrupt_len(), 1);
    }

    #[test]
    fn queue_mode_routes_interrupt_slash_to_interrupt_queue() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Queue);
        controller.start_run().expect("run starts");

        let interrupt = controller
            .submit_slash_while_busy("/interrupt current turn")
            .expect("interrupt slash routes");

        assert_eq!(
            interrupt,
            BusySubmitOutcome::RecordedInterrupt(InterruptRequest {
                text: "/interrupt current turn".to_string(),
                kind: ForegroundInputKind::SlashCommand,
            })
        );
        assert_eq!(controller.state(), ForegroundTurnState::Interrupting);
        assert_eq!(controller.pending_len(), 0);
        assert_eq!(controller.interrupt_len(), 1);
        assert_eq!(
            controller.take_next_interrupt_request(),
            Some(InterruptRequest {
                text: "/interrupt current turn".to_string(),
                kind: ForegroundInputKind::SlashCommand,
            })
        );
    }

    #[test]
    fn finish_run_leaves_next_pending_prompt_available_for_dequeue() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Queue);
        let run_id = controller.start_run().expect("run starts");
        controller
            .submit_text_while_busy("next prompt")
            .expect("prompt queues");

        controller.finish_run(run_id).expect("run finishes");

        assert_eq!(controller.state(), ForegroundTurnState::Idle);
        assert_eq!(
            controller.take_next_pending_prompt(),
            Some(PendingPrompt {
                text: "next prompt".to_string(),
                kind: ForegroundInputKind::PlainText,
            })
        );
        assert_eq!(controller.take_next_pending_prompt(), None);
    }

    #[test]
    fn active_run_query_tracks_current_run_lifecycle() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Queue);

        assert_eq!(controller.active_run_id(), None);

        let first_run = controller.start_run().expect("first run starts");
        assert_eq!(controller.active_run_id(), Some(first_run));

        controller
            .finish_run(first_run)
            .expect("first run finishes");
        assert_eq!(controller.active_run_id(), None);

        let second_run = controller.start_run().expect("second run starts");
        assert_eq!(controller.active_run_id(), Some(second_run));
        assert_ne!(first_run, second_run);
    }

    #[test]
    fn take_next_interrupt_prompt_drains_interrupts_in_arrival_order() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Interrupt);
        controller.start_run().expect("run starts");
        controller
            .submit_text_while_busy("first interrupt")
            .expect("plain text interrupt records");
        controller
            .submit_slash_while_busy("/interrupt second interrupt")
            .expect("interrupt slash records");
        controller
            .record_interrupt_request(InterruptRequest::new(
                "/stop third interrupt",
                ForegroundInputKind::SlashCommand,
            ))
            .expect("manual interrupt records");

        assert_eq!(
            controller.take_next_interrupt_prompt(),
            Some("first interrupt\n/interrupt second interrupt\n/stop third interrupt".to_string())
        );
        assert_eq!(controller.take_next_interrupt_prompt(), None);
        assert_eq!(controller.interrupt_len(), 0);
    }

    #[test]
    fn busy_slash_routing_only_interrupts_for_exact_interrupt_commands() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Interrupt);
        controller.start_run().expect("run starts");

        let status = controller
            .submit_slash_while_busy("/status")
            .expect("status slash routes");
        let stopwatch = controller
            .submit_slash_while_busy("/stopwatch")
            .expect("stopwatch slash routes");
        let cancelled = controller
            .submit_slash_while_busy("/cancelled")
            .expect("cancelled slash routes");
        let interruption = controller
            .submit_slash_while_busy("/interruptions")
            .expect("interruptions slash routes");
        let stop = controller
            .submit_slash_while_busy("/stop now")
            .expect("stop slash routes");

        assert!(matches!(status, BusySubmitOutcome::QueuedPrompt(_)));
        assert!(matches!(stopwatch, BusySubmitOutcome::QueuedPrompt(_)));
        assert!(matches!(cancelled, BusySubmitOutcome::QueuedPrompt(_)));
        assert!(matches!(interruption, BusySubmitOutcome::QueuedPrompt(_)));
        assert_eq!(
            stop,
            BusySubmitOutcome::RecordedInterrupt(InterruptRequest {
                text: "/stop now".to_string(),
                kind: ForegroundInputKind::SlashCommand,
            })
        );
        assert_eq!(controller.pending_len(), 4);
        assert_eq!(controller.interrupt_len(), 1);
    }

    #[test]
    fn busy_plain_text_only_marks_active_run_cancelled_in_interrupt_mode() {
        let mut queue_controller = ForegroundTurnController::new(BusyInputMode::Queue);
        let queue_run = queue_controller.start_run().expect("queue run starts");
        queue_controller
            .submit_text_while_busy("follow-up prompt")
            .expect("plain text queues");

        assert_eq!(
            queue_controller.active_run_status(),
            Some(ActiveRunStatus {
                run_id: queue_run,
                cancel_state: ActiveRunCancelState::Active,
            })
        );
        assert_eq!(queue_controller.pending_len(), 1);
        assert_eq!(queue_controller.interrupt_len(), 0);

        let mut interrupt_controller = ForegroundTurnController::new(BusyInputMode::Interrupt);
        let interrupt_run = interrupt_controller
            .start_run()
            .expect("interrupt run starts");
        interrupt_controller
            .submit_text_while_busy("stop after this turn")
            .expect("plain text records interrupt");

        assert_eq!(
            interrupt_controller.active_run_status(),
            Some(ActiveRunStatus {
                run_id: interrupt_run,
                cancel_state: ActiveRunCancelState::CancelRequested,
            })
        );
        assert_eq!(interrupt_controller.pending_len(), 0);
        assert_eq!(interrupt_controller.interrupt_len(), 1);
    }

    #[test]
    fn busy_interrupt_slash_keeps_cancel_state_until_explicitly_cleared() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Queue);
        let run_id = controller.start_run().expect("run starts");

        controller
            .submit_slash_while_busy("/interrupt revise with bullets")
            .expect("interrupt slash records");

        assert_eq!(
            controller.active_run_status(),
            Some(ActiveRunStatus {
                run_id,
                cancel_state: ActiveRunCancelState::CancelRequested,
            })
        );
        assert_eq!(
            controller.take_next_interrupt_request(),
            Some(InterruptRequest {
                text: "/interrupt revise with bullets".to_string(),
                kind: ForegroundInputKind::SlashCommand,
            })
        );
        assert_eq!(controller.interrupt_len(), 0);
        assert_eq!(
            controller.active_run_status(),
            Some(ActiveRunStatus {
                run_id,
                cancel_state: ActiveRunCancelState::CancelRequested,
            })
        );

        controller
            .clear_active_run_cancel_state(run_id)
            .expect("cancel state clears");

        assert_eq!(
            controller.active_run_status(),
            Some(ActiveRunStatus {
                run_id,
                cancel_state: ActiveRunCancelState::Active,
            })
        );
        assert_eq!(controller.state(), ForegroundTurnState::Running);
    }

    #[test]
    fn finish_run_clears_active_run_cancel_state_but_keeps_follow_up_interrupt_payload() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Interrupt);
        let run_id = controller.start_run().expect("run starts");

        controller
            .submit_text_while_busy("interrupt me please")
            .expect("plain text records interrupt");
        controller.finish_run(run_id).expect("run finishes");

        assert_eq!(controller.active_run_status(), None);
        assert_eq!(controller.active_run_id(), None);
        assert_eq!(controller.state(), ForegroundTurnState::Idle);
        assert_eq!(
            controller.take_next_interrupt_request(),
            Some(InterruptRequest {
                text: "interrupt me please".to_string(),
                kind: ForegroundInputKind::PlainText,
            })
        );
        assert_eq!(controller.take_next_interrupt_request(), None);
    }
}
