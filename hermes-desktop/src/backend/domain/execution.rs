//! Execution 领域模型

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionMode {
    Api,
    Cli,
    Browser,
    Desktop,
}

impl ExecutionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Cli => "cli",
            Self::Browser => "browser",
            Self::Desktop => "desktop",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "api" => Self::Api,
            "browser" => Self::Browser,
            "desktop" => Self::Desktop,
            _ => Self::Cli,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "low" => Self::Low,
            "high" => Self::High,
            _ => Self::Medium,
        }
    }

    pub fn requires_approval(&self) -> bool {
        matches!(self, Self::High)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStepStatus {
    Pending,
    AwaitingApproval,
    Running,
    Paused,
    Completed,
    Failed,
    Skipped,
}

impl ExecutionStepStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "awaiting_approval" => Self::AwaitingApproval,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "skipped" => Self::Skipped,
            _ => Self::Pending,
        }
    }

    pub fn can_transition_to(&self, next: &Self) -> bool {
        matches!(
            (self, next),
            (Self::Pending, Self::Pending)
                | (Self::Pending, Self::AwaitingApproval)
                | (Self::Pending, Self::Running)
                | (Self::Pending, Self::Skipped)
                | (Self::AwaitingApproval, Self::AwaitingApproval)
                | (Self::AwaitingApproval, Self::Running)
                | (Self::AwaitingApproval, Self::Skipped)
                | (Self::Running, Self::Running)
                | (Self::Running, Self::Paused)
                | (Self::Running, Self::Completed)
                | (Self::Running, Self::Failed)
                | (Self::Paused, Self::Paused)
                | (Self::Paused, Self::Running)
                | (Self::Completed, Self::Completed)
                | (Self::Failed, Self::Pending)
                | (Self::Failed, Self::Failed)
                | (Self::Skipped, Self::Pending)
                | (Self::Skipped, Self::Skipped)
        )
    }

    pub fn next_status_for_approve(&self) -> Option<Self> {
        match self {
            Self::AwaitingApproval => Some(Self::Running),
            _ => None,
        }
    }

    pub fn next_status_for_start(&self) -> Option<Self> {
        match self {
            Self::Pending => Some(Self::Running),
            _ => None,
        }
    }

    pub fn next_status_for_pause(&self) -> Option<Self> {
        match self {
            Self::Running => Some(Self::Paused),
            _ => None,
        }
    }

    pub fn next_status_for_resume(&self) -> Option<Self> {
        match self {
            Self::Paused => Some(Self::Running),
            _ => None,
        }
    }

    pub fn next_status_for_complete(&self) -> Option<Self> {
        match self {
            Self::Running => Some(Self::Completed),
            _ => None,
        }
    }

    pub fn next_status_for_fail(&self) -> Option<Self> {
        match self {
            Self::Running => Some(Self::Failed),
            _ => None,
        }
    }

    pub fn next_status_for_retry(&self) -> Option<Self> {
        match self {
            Self::Failed => Some(Self::Pending),
            _ => None,
        }
    }

    pub fn next_status_for_rerun(&self) -> Option<Self> {
        match self {
            Self::Skipped => Some(Self::Pending),
            _ => None,
        }
    }

    pub fn next_status_for_confirm_skip(&self) -> Option<Self> {
        match self {
            Self::Skipped => Some(Self::Skipped),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionStep {
    pub id: String,
    pub mission_id: String,
    pub run_id: String,
    pub title: String,
    pub mode: ExecutionMode,
    pub risk_level: RiskLevel,
    pub status: ExecutionStepStatus,
    pub input_payload: Option<String>,
    pub output_summary: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateExecutionStepInput {
    pub mission_id: String,
    pub run_id: String,
    pub title: String,
    pub mode: ExecutionMode,
    pub risk_level: RiskLevel,
    pub input_payload: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_mode_and_status_use_contract_strings() {
        assert_eq!(ExecutionMode::Api.as_str(), "api");
        assert_eq!(ExecutionMode::from_key("desktop"), ExecutionMode::Desktop);
        assert_eq!(
            ExecutionStepStatus::AwaitingApproval.as_str(),
            "awaiting_approval"
        );
        assert_eq!(
            ExecutionStepStatus::from_key("completed"),
            ExecutionStepStatus::Completed
        );
    }

    #[test]
    fn high_risk_requires_approval() {
        assert!(!RiskLevel::Low.requires_approval());
        assert!(!RiskLevel::Medium.requires_approval());
        assert!(RiskLevel::High.requires_approval());
    }

    #[test]
    fn execution_status_transition_matrix_matches_contract() {
        assert!(
            ExecutionStepStatus::Pending.can_transition_to(&ExecutionStepStatus::AwaitingApproval)
        );
        assert!(ExecutionStepStatus::Pending.can_transition_to(&ExecutionStepStatus::Running));
        assert!(ExecutionStepStatus::Pending.can_transition_to(&ExecutionStepStatus::Skipped));
        assert!(
            ExecutionStepStatus::AwaitingApproval.can_transition_to(&ExecutionStepStatus::Running)
        );
        assert!(
            ExecutionStepStatus::AwaitingApproval.can_transition_to(&ExecutionStepStatus::Skipped)
        );
        assert!(ExecutionStepStatus::Running.can_transition_to(&ExecutionStepStatus::Completed));
        assert!(ExecutionStepStatus::Running.can_transition_to(&ExecutionStepStatus::Failed));

        assert!(!ExecutionStepStatus::Pending.can_transition_to(&ExecutionStepStatus::Completed));
        assert!(!ExecutionStepStatus::Completed.can_transition_to(&ExecutionStepStatus::Running));
        assert!(!ExecutionStepStatus::Skipped.can_transition_to(&ExecutionStepStatus::Running));
    }

    #[test]
    fn approve_start_and_complete_follow_action_specific_paths() {
        assert_eq!(
            ExecutionStepStatus::AwaitingApproval
                .next_status_for_approve()
                .expect("approve should transition awaiting approval"),
            ExecutionStepStatus::Running
        );
        assert_eq!(
            ExecutionStepStatus::Pending
                .next_status_for_start()
                .expect("start should transition pending"),
            ExecutionStepStatus::Running
        );
        assert_eq!(
            ExecutionStepStatus::Running
                .next_status_for_complete()
                .expect("complete should transition running"),
            ExecutionStepStatus::Completed
        );
        assert_eq!(
            ExecutionStepStatus::Running
                .next_status_for_fail()
                .expect("fail should transition running"),
            ExecutionStepStatus::Failed
        );
    }

    #[test]
    fn approve_start_and_complete_reject_illegal_statuses() {
        assert!(
            ExecutionStepStatus::Pending
                .next_status_for_approve()
                .is_none()
        );
        assert!(
            ExecutionStepStatus::Running
                .next_status_for_approve()
                .is_none()
        );

        assert!(
            ExecutionStepStatus::AwaitingApproval
                .next_status_for_start()
                .is_none()
        );
        assert!(
            ExecutionStepStatus::Completed
                .next_status_for_start()
                .is_none()
        );

        assert!(
            ExecutionStepStatus::Pending
                .next_status_for_complete()
                .is_none()
        );
        assert!(
            ExecutionStepStatus::AwaitingApproval
                .next_status_for_complete()
                .is_none()
        );
        assert!(
            ExecutionStepStatus::Failed
                .next_status_for_complete()
                .is_none()
        );
    }

    #[test]
    fn paused_retry_resume_and_rerun_paths_are_supported_for_recovery() {
        assert_eq!(
            ExecutionStepStatus::from_key("paused"),
            ExecutionStepStatus::Paused
        );
        assert_eq!(ExecutionStepStatus::Paused.as_str(), "paused");

        assert!(ExecutionStepStatus::Running.can_transition_to(&ExecutionStepStatus::Paused));
        assert!(ExecutionStepStatus::Paused.can_transition_to(&ExecutionStepStatus::Running));
        assert!(ExecutionStepStatus::Failed.can_transition_to(&ExecutionStepStatus::Pending));
        assert!(ExecutionStepStatus::Skipped.can_transition_to(&ExecutionStepStatus::Pending));

        assert_eq!(
            ExecutionStepStatus::Running
                .next_status_for_pause()
                .expect("running steps can be paused"),
            ExecutionStepStatus::Paused
        );
        assert_eq!(
            ExecutionStepStatus::Paused
                .next_status_for_resume()
                .expect("paused steps can resume"),
            ExecutionStepStatus::Running
        );
        assert_eq!(
            ExecutionStepStatus::Failed
                .next_status_for_retry()
                .expect("failed steps can retry"),
            ExecutionStepStatus::Pending
        );
        assert_eq!(
            ExecutionStepStatus::Skipped
                .next_status_for_rerun()
                .expect("skipped steps can rerun"),
            ExecutionStepStatus::Pending
        );
        assert_eq!(
            ExecutionStepStatus::Skipped
                .next_status_for_confirm_skip()
                .expect("skipped steps can be explicitly confirmed"),
            ExecutionStepStatus::Skipped
        );
    }
}
