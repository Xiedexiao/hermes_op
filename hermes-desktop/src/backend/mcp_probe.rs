//! Deterministic MCP probe evidence helpers for later parity wiring.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpProbeStatus {
    Ready,
    Error,
    Warning,
}

impl McpProbeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedStdioCommand {
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpProbeEvidence {
    pub status: McpProbeStatus,
    pub message: String,
    pub command_available: Option<bool>,
    pub url_valid: Option<bool>,
    pub parsed_command: Option<String>,
    pub parsed_args: Vec<String>,
    pub endpoint_scheme: Option<String>,
    pub endpoint_host: Option<String>,
    pub endpoint_detail: Option<String>,
}

impl McpProbeEvidence {
    pub fn status_label(&self) -> &'static str {
        self.status.as_str()
    }

    pub fn is_ready(&self) -> bool {
        matches!(self.status, McpProbeStatus::Ready)
    }

    pub fn endpoint_is_valid(&self) -> bool {
        self.command_available.unwrap_or(false) || self.url_valid.unwrap_or(false)
    }
}

pub fn parse_stdio_command(endpoint: &str) -> Result<ParsedStdioCommand, String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err("Endpoint command is empty.".to_string());
    }

    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut active_quote = None;
    let mut escaping = false;

    for character in trimmed.chars() {
        if escaping {
            current.push(character);
            escaping = false;
            continue;
        }

        match character {
            '\\' if active_quote != Some('\'') => escaping = true,
            '\'' | '"' => {
                if active_quote == Some(character) {
                    active_quote = None;
                } else if active_quote.is_none() {
                    active_quote = Some(character);
                } else {
                    current.push(character);
                }
            }
            whitespace if whitespace.is_whitespace() && active_quote.is_none() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => current.push(character),
        }
    }

    if escaping {
        current.push('\\');
    }

    if let Some(quote) = active_quote {
        return Err(format!("Unterminated {} quote in endpoint command.", quote));
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    let mut tokens = tokens.into_iter();
    let command = tokens
        .next()
        .ok_or_else(|| "Endpoint command is empty.".to_string())?;

    Ok(ParsedStdioCommand {
        command,
        args: tokens.collect(),
    })
}

pub fn build_probe_evidence<F>(
    transport: &str,
    endpoint: &str,
    command_available: F,
) -> McpProbeEvidence
where
    F: Fn(&str) -> bool,
{
    let normalized_transport = transport.trim().to_ascii_lowercase();

    match normalized_transport.as_str() {
        "stdio" => match parse_stdio_command(endpoint) {
            Ok(parsed) => {
                let available = command_available(&parsed.command);
                let detail = format!(
                    "Parsed stdio command with {} argument(s).",
                    parsed.args.len()
                );

                if available {
                    McpProbeEvidence {
                        status: McpProbeStatus::Ready,
                        message: format!(
                            "stdio command `{}` is available for launch.",
                            parsed.command
                        ),
                        command_available: Some(true),
                        url_valid: None,
                        parsed_command: Some(parsed.command),
                        parsed_args: parsed.args,
                        endpoint_scheme: None,
                        endpoint_host: None,
                        endpoint_detail: Some(detail),
                    }
                } else {
                    McpProbeEvidence {
                        status: McpProbeStatus::Error,
                        message: format!(
                            "stdio command `{}` is not available on PATH.",
                            parsed.command
                        ),
                        command_available: Some(false),
                        url_valid: None,
                        parsed_command: Some(parsed.command),
                        parsed_args: parsed.args,
                        endpoint_scheme: None,
                        endpoint_host: None,
                        endpoint_detail: Some(detail),
                    }
                }
            }
            Err(err) => McpProbeEvidence {
                status: McpProbeStatus::Error,
                message: format!("stdio endpoint could not be parsed: {err}"),
                command_available: None,
                url_valid: None,
                parsed_command: None,
                parsed_args: Vec::new(),
                endpoint_scheme: None,
                endpoint_host: None,
                endpoint_detail: Some(err),
            },
        },
        "http" | "sse" => match parse_remote_endpoint(endpoint) {
            Ok(parsed) => {
                if !matches!(parsed.scheme.as_str(), "http" | "https") {
                    McpProbeEvidence {
                        status: McpProbeStatus::Error,
                        message: format!("{} endpoint URL is invalid.", normalized_transport),
                        command_available: None,
                        url_valid: Some(false),
                        parsed_command: None,
                        parsed_args: Vec::new(),
                        endpoint_scheme: Some(parsed.scheme.clone()),
                        endpoint_host: parsed.host.clone(),
                        endpoint_detail: Some(format!(
                            "Unsupported URL scheme `{}`; expected http or https.",
                            parsed.scheme
                        )),
                    }
                } else if let Some(host) = parsed.host {
                    McpProbeEvidence {
                        status: McpProbeStatus::Ready,
                        message: format!(
                            "{} endpoint URL is syntactically valid.",
                            normalized_transport
                        ),
                        command_available: None,
                        url_valid: Some(true),
                        parsed_command: None,
                        parsed_args: Vec::new(),
                        endpoint_scheme: Some(parsed.scheme.clone()),
                        endpoint_host: Some(host.clone()),
                        endpoint_detail: Some(format!(
                            "Valid {} URL with host `{}`.",
                            parsed.scheme, host
                        )),
                    }
                } else {
                    McpProbeEvidence {
                        status: McpProbeStatus::Error,
                        message: format!("{} endpoint URL is invalid.", normalized_transport),
                        command_available: None,
                        url_valid: Some(false),
                        parsed_command: None,
                        parsed_args: Vec::new(),
                        endpoint_scheme: Some(parsed.scheme),
                        endpoint_host: None,
                        endpoint_detail: Some("URL must include a host.".to_string()),
                    }
                }
            }
            Err(err) => McpProbeEvidence {
                status: McpProbeStatus::Error,
                message: format!("{} endpoint URL is invalid.", normalized_transport),
                command_available: None,
                url_valid: Some(false),
                parsed_command: None,
                parsed_args: Vec::new(),
                endpoint_scheme: None,
                endpoint_host: None,
                endpoint_detail: Some(err),
            },
        },
        _ => McpProbeEvidence {
            status: McpProbeStatus::Warning,
            message: format!("Unsupported transport {} for probe.", transport.trim()),
            command_available: None,
            url_valid: None,
            parsed_command: None,
            parsed_args: Vec::new(),
            endpoint_scheme: None,
            endpoint_host: None,
            endpoint_detail: Some(format!(
                "No static probe evidence is available for transport `{}`.",
                transport.trim()
            )),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedRemoteEndpoint {
    scheme: String,
    host: Option<String>,
}

fn parse_remote_endpoint(endpoint: &str) -> Result<ParsedRemoteEndpoint, String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return Err("URL parse failed: endpoint is empty.".to_string());
    }

    let separator = trimmed
        .find("://")
        .ok_or_else(|| "URL parse failed: missing scheme separator.".to_string())?;
    let scheme = trimmed[..separator].trim().to_ascii_lowercase();

    if scheme.is_empty() {
        return Err("URL parse failed: missing URL scheme.".to_string());
    }

    if !scheme
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.'))
    {
        return Err("URL parse failed: invalid URL scheme.".to_string());
    }

    let remainder = &trimmed[separator + 3..];
    let authority = split_url_authority(remainder);

    Ok(ParsedRemoteEndpoint {
        scheme,
        host: extract_host(authority),
    })
}

fn split_url_authority(remainder: &str) -> &str {
    let path_start = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    &remainder[..path_start]
}

fn extract_host(authority: &str) -> Option<String> {
    let without_userinfo = authority.rsplit('@').next().unwrap_or(authority).trim();
    if without_userinfo.is_empty() {
        return None;
    }

    if let Some(rest) = without_userinfo.strip_prefix('[') {
        let end = rest.find(']')?;
        let host = &rest[..end];
        if host.is_empty() {
            None
        } else {
            Some(host.to_string())
        }
    } else {
        let host = without_userinfo
            .rsplit_once(':')
            .filter(|(host, port)| !host.is_empty() && !port.is_empty() && !host.contains(':'))
            .map_or(without_userinfo, |(host, _)| host)
            .trim();

        if host.is_empty() {
            None
        } else {
            Some(host.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{McpProbeStatus, build_probe_evidence, parse_stdio_command};

    #[test]
    fn parse_stdio_command_preserves_quotes_and_escapes() {
        let parsed = parse_stdio_command(
            "npx -y @modelcontextprotocol/server-filesystem --root \"/tmp/hermes workspace\" --name escaped\\ value",
        )
        .expect("stdio endpoint should parse");

        assert_eq!(parsed.command, "npx");
        assert_eq!(
            parsed.args,
            vec![
                "-y",
                "@modelcontextprotocol/server-filesystem",
                "--root",
                "/tmp/hermes workspace",
                "--name",
                "escaped value",
            ]
        );
    }

    #[test]
    fn build_probe_evidence_reports_ready_stdio_command() {
        let evidence = build_probe_evidence("stdio", "uvx mcp-server --stdio", |command| {
            command == "uvx"
        });

        assert_eq!(evidence.status, McpProbeStatus::Ready);
        assert_eq!(evidence.status_label(), "ready");
        assert!(evidence.is_ready());
        assert!(evidence.endpoint_is_valid());
        assert_eq!(evidence.command_available, Some(true));
        assert_eq!(evidence.parsed_command.as_deref(), Some("uvx"));
        assert_eq!(evidence.parsed_args, vec!["mcp-server", "--stdio"]);
        assert_eq!(
            evidence.endpoint_detail.as_deref(),
            Some("Parsed stdio command with 2 argument(s).")
        );
        assert_eq!(
            evidence.message,
            "stdio command `uvx` is available for launch."
        );
    }

    #[test]
    fn build_probe_evidence_reports_stdio_parse_errors() {
        let evidence = build_probe_evidence("stdio", "npx \"unterminated", |_| true);

        assert_eq!(evidence.status, McpProbeStatus::Error);
        assert_eq!(evidence.command_available, None);
        assert_eq!(evidence.url_valid, None);
        assert_eq!(evidence.parsed_command, None);
        assert_eq!(evidence.parsed_args, Vec::<String>::new());
        assert_eq!(
            evidence.message,
            "stdio endpoint could not be parsed: Unterminated \" quote in endpoint command."
        );
        assert_eq!(
            evidence.endpoint_detail.as_deref(),
            Some("Unterminated \" quote in endpoint command.")
        );
    }

    #[test]
    fn build_probe_evidence_reports_valid_http_url() {
        let evidence = build_probe_evidence("http", "https://example.com/mcp", |_| false);

        assert_eq!(evidence.status, McpProbeStatus::Ready);
        assert_eq!(evidence.command_available, None);
        assert_eq!(evidence.url_valid, Some(true));
        assert!(evidence.endpoint_is_valid());
        assert_eq!(evidence.endpoint_scheme.as_deref(), Some("https"));
        assert_eq!(evidence.endpoint_host.as_deref(), Some("example.com"));
        assert_eq!(
            evidence.endpoint_detail.as_deref(),
            Some("Valid https URL with host `example.com`.")
        );
        assert_eq!(
            evidence.message,
            "http endpoint URL is syntactically valid."
        );
    }

    #[test]
    fn build_probe_evidence_rejects_invalid_sse_scheme() {
        let evidence = build_probe_evidence("sse", "ftp://example.com/mcp", |_| true);

        assert_eq!(evidence.status, McpProbeStatus::Error);
        assert_eq!(evidence.url_valid, Some(false));
        assert!(!evidence.endpoint_is_valid());
        assert_eq!(evidence.endpoint_scheme.as_deref(), Some("ftp"));
        assert_eq!(evidence.endpoint_host.as_deref(), Some("example.com"));
        assert_eq!(
            evidence.endpoint_detail.as_deref(),
            Some("Unsupported URL scheme `ftp`; expected http or https.")
        );
        assert_eq!(evidence.message, "sse endpoint URL is invalid.");
    }

    #[test]
    fn build_probe_evidence_warns_for_unknown_transport() {
        let evidence = build_probe_evidence("websocket", "ws://localhost:3000/mcp", |_| true);

        assert_eq!(evidence.status, McpProbeStatus::Warning);
        assert_eq!(evidence.command_available, None);
        assert_eq!(evidence.url_valid, None);
        assert_eq!(
            evidence.endpoint_detail.as_deref(),
            Some("No static probe evidence is available for transport `websocket`.")
        );
        assert_eq!(
            evidence.message,
            "Unsupported transport websocket for probe."
        );
    }
}
