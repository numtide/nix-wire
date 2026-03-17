//! Evaluate expect assertions against daemon responses.

use regex::Regex;

use crate::protocol::HandshakeInfo;

use super::{DaemonResponse, Expect, Matcher, ResultData};

/// Result of evaluating an expect assertion.
#[derive(Debug, Clone)]
pub struct ExpectResult {
    pub passed: bool,
    pub message: String,
}

/// Evaluate a list of expects against a daemon response.
pub fn evaluate_expects(expects: &[Expect], response: &DaemonResponse) -> Vec<ExpectResult> {
    expects
        .iter()
        .map(|expect| evaluate_expect(expect, response))
        .collect()
}

/// Evaluate handshake-level expects against HandshakeInfo.
pub fn evaluate_handshake_expects(expects: &[Expect], info: &HandshakeInfo) -> Vec<ExpectResult> {
    expects
        .iter()
        .map(|expect| evaluate_handshake_expect(expect, info))
        .collect()
}

fn evaluate_handshake_expect(expect: &Expect, info: &HandshakeInfo) -> ExpectResult {
    match expect {
        Expect::Daemon { matcher } => {
            let value = info.daemon_nix_version.as_deref().unwrap_or("unknown");
            let passed = matches_value(matcher, value);
            ExpectResult {
                passed,
                message: if passed {
                    format!("daemon = {value}")
                } else {
                    format!("daemon: expected {matcher}, got {value}")
                },
            }
        }
        Expect::Trust { matcher } => {
            let trust_str = match info.trust_status {
                Some(1) => "trusted",
                Some(2) => "not-trusted",
                Some(_) => "unknown",
                None => "unknown",
            };
            let passed = matches_value(matcher, trust_str);
            ExpectResult {
                passed,
                message: if passed {
                    format!("trust = {trust_str}")
                } else {
                    format!("trust: expected {matcher}, got {trust_str}")
                },
            }
        }
        Expect::ServerFeatures { matcher } => {
            let feats_str = format!("{{ {} }}", info.server_features.join(", "));
            let passed = matches_value(matcher, &feats_str);
            ExpectResult {
                passed,
                message: if passed {
                    format!("server_features = {feats_str}")
                } else {
                    format!("server_features: expected {matcher}, got {feats_str}")
                },
            }
        }
        _ => ExpectResult {
            passed: false,
            message: format!("non-handshake expect in preamble: {expect:?}"),
        },
    }
}

fn evaluate_expect(expect: &Expect, response: &DaemonResponse) -> ExpectResult {
    match expect {
        Expect::Terminal(expected) => {
            let passed = response.terminal == *expected;
            ExpectResult {
                passed,
                message: if passed {
                    format!("terminal = {expected}")
                } else {
                    format!("expected terminal {expected}, got {}", response.terminal)
                },
            }
        }

        Expect::Result { field, matcher } => {
            let field_str = field
                .as_deref()
                .map(|f| format!("result.{f}"))
                .unwrap_or_else(|| "result".to_string());
            let value = match &response.result {
                Some(result) => extract_result_value(result, field.as_deref()),
                None => None,
            };
            match value {
                Some(v) => {
                    let passed = matches_value(matcher, &v);
                    ExpectResult {
                        passed,
                        message: if passed {
                            format!("{field_str} = {v}")
                        } else {
                            format!("{field_str}: expected {matcher}, got {v}")
                        },
                    }
                }
                None => ExpectResult {
                    passed: false,
                    message: format!("result field {} not found", field_str),
                },
            }
        }

        Expect::Error { matcher } => match &response.error {
            Some(err) => {
                let passed = matches_value(matcher, &err.message);
                ExpectResult {
                    passed,
                    message: if passed {
                        format!("error matched: {}", err.message)
                    } else {
                        format!("error: expected {matcher}, got {}", err.message)
                    },
                }
            }
            None => ExpectResult {
                passed: false,
                message: "expected error but none received".to_string(),
            },
        },

        Expect::StderrCount { matcher } => {
            let count_str = response.stderr_count.to_string();
            let passed = matches_value(matcher, &count_str);
            ExpectResult {
                passed,
                message: if passed {
                    format!("stderr.count = {}", response.stderr_count)
                } else {
                    format!(
                        "stderr.count: expected {matcher}, got {}",
                        response.stderr_count
                    )
                },
            }
        }

        // Handshake expects should not appear in op-level evaluation
        Expect::Daemon { .. } | Expect::Trust { .. } | Expect::ServerFeatures { .. } => {
            ExpectResult {
                passed: false,
                message: "handshake expect used in op context".to_string(),
            }
        }
    }
}

fn extract_result_value(result: &ResultData, field: Option<&str>) -> Option<String> {
    match (result, field) {
        (ResultData::U64(v), None) => Some(v.to_string()),
        (ResultData::Str(s), None) => Some(s.clone()),
        (ResultData::PathInfo(info), Some("valid")) => Some((info.valid as u64).to_string()),
        (ResultData::PathInfo(info), Some("deriver")) => info.deriver.clone(),
        (ResultData::PathInfo(info), Some("narHash")) => info.nar_hash.clone(),
        (ResultData::PathInfo(info), Some("narSize")) => info.nar_size.map(|n| n.to_string()),
        (ResultData::PathInfo(info), Some("references")) => info
            .references
            .as_ref()
            .map(|r| format!("[{}]", r.join(", "))),
        (ResultData::CollectGarbage { bytes_freed }, Some("bytes_freed")) => {
            Some(bytes_freed.to_string())
        }
        (ResultData::SubstitutablePathInfo { valid, .. }, Some("valid")) => {
            Some((*valid as u64).to_string())
        }
        (ResultData::SubstitutablePathInfo { download_size, .. }, Some("downloadSize")) => {
            download_size.map(|n| n.to_string())
        }
        (ResultData::SubstitutablePathInfo { nar_size, .. }, Some("narSize")) => {
            nar_size.map(|n| n.to_string())
        }
        (ResultData::Missing { download_size, .. }, Some("downloadSize")) => {
            Some(download_size.to_string())
        }
        (ResultData::Missing { nar_size, .. }, Some("narSize")) => Some(nar_size.to_string()),
        (ResultData::StringSet(set), None) => Some(format!("[{}]", set.join(", "))),
        _ => None,
    }
}

fn matches_value(matcher: &Matcher, value: &str) -> bool {
    match matcher {
        Matcher::Exact(expected) => value == expected,
        Matcher::Regex(pattern) => Regex::new(pattern)
            .map(|re| re.is_match(value))
            .unwrap_or(false),
        Matcher::GreaterThan(n) => value.parse::<i64>().map(|v| v > *n).unwrap_or(false),
        Matcher::LessThan(n) => value.parse::<i64>().map(|v| v < *n).unwrap_or(false),
        Matcher::GreaterOrEqual(n) => value.parse::<i64>().map(|v| v >= *n).unwrap_or(false),
        Matcher::LessOrEqual(n) => value.parse::<i64>().map(|v| v <= *n).unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::ProtocolVersion;
    use crate::script::{ErrorInfo, PathInfoResult};

    #[test]
    fn terminal_match() {
        let response = DaemonResponse {
            terminal: "Last".to_string(),
            stderr_count: 1,
            result: Some(ResultData::U64(1)),
            error: None,
        };

        let results = evaluate_expects(&[Expect::Terminal("Last".to_string())], &response);
        assert!(results[0].passed);
    }

    #[test]
    fn terminal_mismatch() {
        let response = DaemonResponse {
            terminal: "Error".to_string(),
            stderr_count: 1,
            result: None,
            error: None,
        };

        let results = evaluate_expects(&[Expect::Terminal("Last".to_string())], &response);
        assert!(!results[0].passed);
    }

    #[test]
    fn result_exact_match() {
        let response = DaemonResponse {
            terminal: "Last".to_string(),
            stderr_count: 1,
            result: Some(ResultData::U64(1)),
            error: None,
        };

        let results = evaluate_expects(
            &[Expect::Result {
                field: None,
                matcher: Matcher::Exact("1".to_string()),
            }],
            &response,
        );
        assert!(results[0].passed);
    }

    #[test]
    fn result_numeric_comparison() {
        let response = DaemonResponse {
            terminal: "Last".to_string(),
            stderr_count: 1,
            result: Some(ResultData::PathInfo(PathInfoResult {
                valid: true,
                deriver: None,
                nar_hash: None,
                references: None,
                registration_time: None,
                nar_size: Some(226552),
                ultimate: None,
                sigs: None,
                ca: None,
            })),
            error: None,
        };

        let results = evaluate_expects(
            &[Expect::Result {
                field: Some("narSize".to_string()),
                matcher: Matcher::GreaterThan(0),
            }],
            &response,
        );
        assert!(results[0].passed);
    }

    #[test]
    fn error_regex_match() {
        let response = DaemonResponse {
            terminal: "Error".to_string(),
            stderr_count: 1,
            result: None,
            error: Some(ErrorInfo {
                error_type: "Error".to_string(),
                level: 0,
                name: "".to_string(),
                message: "path /nix/store/abc is not valid".to_string(),
            }),
        };

        let results = evaluate_expects(
            &[Expect::Error {
                matcher: Matcher::Regex("not valid".to_string()),
            }],
            &response,
        );
        assert!(results[0].passed);
    }

    #[test]
    fn stderr_count_match() {
        let response = DaemonResponse {
            terminal: "Last".to_string(),
            stderr_count: 0,
            result: None,
            error: None,
        };

        let results = evaluate_expects(
            &[Expect::StderrCount {
                matcher: Matcher::Exact("0".to_string()),
            }],
            &response,
        );
        assert!(results[0].passed);
    }

    #[test]
    fn handshake_daemon_exact() {
        let info = HandshakeInfo {
            client_version: ProtocolVersion::new(1, 38),
            server_version: ProtocolVersion::new(1, 38),
            negotiated_version: ProtocolVersion::new(1, 38),
            client_features: Vec::new(),
            server_features: Vec::new(),
            daemon_nix_version: Some("2.33.3".to_string()),
            trust_status: Some(1),
        };

        let results = evaluate_handshake_expects(
            &[Expect::Daemon {
                matcher: Matcher::Exact("2.33.3".to_string()),
            }],
            &info,
        );
        assert!(results[0].passed);
    }

    #[test]
    fn handshake_daemon_regex() {
        let info = HandshakeInfo {
            client_version: ProtocolVersion::new(1, 38),
            server_version: ProtocolVersion::new(1, 38),
            negotiated_version: ProtocolVersion::new(1, 38),
            client_features: Vec::new(),
            server_features: Vec::new(),
            daemon_nix_version: Some("2.33.3".to_string()),
            trust_status: Some(1),
        };

        let results = evaluate_handshake_expects(
            &[Expect::Daemon {
                matcher: Matcher::Regex("^2\\.".to_string()),
            }],
            &info,
        );
        assert!(results[0].passed);
    }

    #[test]
    fn handshake_trust_match() {
        let info = HandshakeInfo {
            client_version: ProtocolVersion::new(1, 38),
            server_version: ProtocolVersion::new(1, 38),
            negotiated_version: ProtocolVersion::new(1, 38),
            client_features: Vec::new(),
            server_features: Vec::new(),
            daemon_nix_version: None,
            trust_status: Some(1),
        };

        let results = evaluate_handshake_expects(
            &[Expect::Trust {
                matcher: Matcher::Exact("trusted".to_string()),
            }],
            &info,
        );
        assert!(results[0].passed);
    }

    #[test]
    fn handshake_trust_mismatch() {
        let info = HandshakeInfo {
            client_version: ProtocolVersion::new(1, 38),
            server_version: ProtocolVersion::new(1, 38),
            negotiated_version: ProtocolVersion::new(1, 38),
            client_features: Vec::new(),
            server_features: Vec::new(),
            daemon_nix_version: None,
            trust_status: Some(2),
        };

        let results = evaluate_handshake_expects(
            &[Expect::Trust {
                matcher: Matcher::Exact("trusted".to_string()),
            }],
            &info,
        );
        assert!(!results[0].passed);
    }
}
