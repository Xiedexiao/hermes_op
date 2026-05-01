//! Pure MCP handshake evidence synthesis helpers.
//!
//! This module intentionally stays IO-free. It takes already-collected static
//! probe facts plus endpoint metadata and reduces them to a handshake-level
//! status that later parity wiring can expose without attempting a live MCP
//! network handshake.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpStaticEvidenceStatus {
    Unknown,
    Ready,
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct McpHandshakeSynthesisInput<'a> {
    pub transport: &'a str,
    pub endpoint: &'a str,
    pub static_status: McpStaticEvidenceStatus,
    pub command_available: Option<bool>,
    pub url_valid: Option<bool>,
    pub detail: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpHandshakeStatus {
    NotAttempted,
    NotSupported,
    StaticReady,
}

impl McpHandshakeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAttempted => "not_attempted",
            Self::NotSupported => "not_supported",
            Self::StaticReady => "static_ready",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpHandshakeEvidence {
    pub status: McpHandshakeStatus,
    pub reason: String,
}

impl McpHandshakeEvidence {
    pub fn status_label(&self) -> &'static str {
        self.status.as_str()
    }

    pub fn is_static_ready(&self) -> bool {
        matches!(self.status, McpHandshakeStatus::StaticReady)
    }
}

pub fn synthesize_handshake_evidence(
    input: McpHandshakeSynthesisInput<'_>,
) -> McpHandshakeEvidence {
    let transport = input.transport.trim().to_ascii_lowercase();

    match transport.as_str() {
        "stdio" => synthesize_stdio_handshake(input),
        "http" | "sse" => synthesize_remote_handshake(input, transport.as_str()),
        _ => McpHandshakeEvidence {
            status: McpHandshakeStatus::NotSupported,
            reason: format!(
                "Transport `{}` is not supported for static handshake synthesis.",
                input.transport.trim()
            ),
        },
    }
}

fn synthesize_stdio_handshake(input: McpHandshakeSynthesisInput<'_>) -> McpHandshakeEvidence {
    if input.command_available == Some(true)
        || matches!(input.static_status, McpStaticEvidenceStatus::Ready)
    {
        return McpHandshakeEvidence {
            status: McpHandshakeStatus::StaticReady,
            reason: "Static stdio launch checks passed.".to_string(),
        };
    }

    McpHandshakeEvidence {
        status: McpHandshakeStatus::NotAttempted,
        reason: not_attempted_reason(
            input.detail,
            "static stdio evidence is unavailable.",
            input.endpoint,
        ),
    }
}

fn synthesize_remote_handshake(
    input: McpHandshakeSynthesisInput<'_>,
    transport: &str,
) -> McpHandshakeEvidence {
    if input.url_valid == Some(true)
        || matches!(input.static_status, McpStaticEvidenceStatus::Ready)
    {
        return McpHandshakeEvidence {
            status: McpHandshakeStatus::StaticReady,
            reason: format!("Static {transport} endpoint validation passed."),
        };
    }

    let unavailable = format!("static {transport} evidence is unavailable.");
    McpHandshakeEvidence {
        status: McpHandshakeStatus::NotAttempted,
        reason: not_attempted_reason(input.detail, unavailable.as_str(), input.endpoint),
    }
}

fn not_attempted_reason(detail: Option<&str>, fallback: &str, endpoint: &str) -> String {
    match detail.map(str::trim).filter(|detail| !detail.is_empty()) {
        Some(detail) => format!("Handshake not attempted: {detail}"),
        None if endpoint.trim().is_empty() => {
            "Handshake not attempted: endpoint is empty.".to_string()
        }
        None => format!("Handshake not attempted: {fallback}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        McpHandshakeStatus, McpHandshakeSynthesisInput, McpStaticEvidenceStatus,
        synthesize_handshake_evidence,
    };

    #[test]
    fn synthesize_handshake_evidence_marks_stdio_probe_readiness_as_static_ready() {
        let evidence = synthesize_handshake_evidence(McpHandshakeSynthesisInput {
            transport: "stdio",
            endpoint: "uvx mcp-server --stdio",
            static_status: McpStaticEvidenceStatus::Ready,
            command_available: Some(true),
            url_valid: None,
            detail: Some("Parsed stdio command with 2 argument(s)."),
        });

        assert_eq!(evidence.status, McpHandshakeStatus::StaticReady);
        assert_eq!(evidence.status_label(), "static_ready");
        assert!(evidence.is_static_ready());
        assert_eq!(evidence.reason, "Static stdio launch checks passed.");
    }

    #[test]
    fn synthesize_handshake_evidence_marks_valid_remote_probe_as_static_ready() {
        let evidence = synthesize_handshake_evidence(McpHandshakeSynthesisInput {
            transport: "http",
            endpoint: "https://example.com/mcp",
            static_status: McpStaticEvidenceStatus::Ready,
            command_available: None,
            url_valid: Some(true),
            detail: Some("Valid https URL with host `example.com`."),
        });

        assert_eq!(evidence.status, McpHandshakeStatus::StaticReady);
        assert_eq!(evidence.reason, "Static http endpoint validation passed.");
    }

    #[test]
    fn synthesize_handshake_evidence_keeps_invalid_static_stdio_as_not_attempted() {
        let evidence = synthesize_handshake_evidence(McpHandshakeSynthesisInput {
            transport: "stdio",
            endpoint: "npx \"unterminated",
            static_status: McpStaticEvidenceStatus::Error,
            command_available: None,
            url_valid: None,
            detail: Some("Unterminated \" quote in endpoint command."),
        });

        assert_eq!(evidence.status, McpHandshakeStatus::NotAttempted);
        assert_eq!(
            evidence.reason,
            "Handshake not attempted: Unterminated \" quote in endpoint command."
        );
    }

    #[test]
    fn synthesize_handshake_evidence_reports_missing_static_facts_concisely() {
        let evidence = synthesize_handshake_evidence(McpHandshakeSynthesisInput {
            transport: "sse",
            endpoint: "https://example.com/events",
            static_status: McpStaticEvidenceStatus::Unknown,
            command_available: None,
            url_valid: None,
            detail: None,
        });

        assert_eq!(evidence.status, McpHandshakeStatus::NotAttempted);
        assert_eq!(
            evidence.reason,
            "Handshake not attempted: static sse evidence is unavailable."
        );
    }

    #[test]
    fn synthesize_handshake_evidence_marks_unknown_transports_as_not_supported() {
        let evidence = synthesize_handshake_evidence(McpHandshakeSynthesisInput {
            transport: "websocket",
            endpoint: "ws://localhost:3000/mcp",
            static_status: McpStaticEvidenceStatus::Warning,
            command_available: None,
            url_valid: None,
            detail: Some("No static probe evidence is available for transport `websocket`."),
        });

        assert_eq!(evidence.status, McpHandshakeStatus::NotSupported);
        assert_eq!(
            evidence.reason,
            "Transport `websocket` is not supported for static handshake synthesis."
        );
    }
}
