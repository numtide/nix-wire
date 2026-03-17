//! Parse `.nwscript` text into a Script AST.

use anyhow::{bail, Context, Result};

use crate::handshake::ProtocolVersion;
use crate::ops::Op;

use super::{Entry, Expect, FramedData, Matcher, OpCall, Preamble, Script};

/// Parse a .nwscript text string into a Script AST.
pub fn parse_script(text: &str) -> Result<Script> {
    let lines: Vec<&str> = text.lines().collect();
    let mut pos = 0;

    // Parse preamble
    let preamble = parse_preamble(&lines, &mut pos)?;

    // Parse entries
    let mut entries = Vec::new();
    while pos < lines.len() {
        // Skip blank lines and comment-only lines at top level
        let line = lines[pos].trim();
        if line.is_empty() || (line.starts_with('#') && !line.starts_with("# nwscript")) {
            pos += 1;
            continue;
        }

        // Should be an op line (possibly with @timestamp)
        let entry = parse_entry(&lines, &mut pos)?;
        entries.push(entry);
    }

    Ok(Script { preamble, entries })
}

fn parse_preamble(lines: &[&str], pos: &mut usize) -> Result<Preamble> {
    let mut protocol_version = ProtocolVersion::new(1, 38);
    let mut client_features = Vec::new();
    let mut expects = Vec::new();
    let mut daemon_version = None;
    let mut trust = None;
    let mut server_features = None;

    while *pos < lines.len() {
        let line = lines[*pos].trim();
        if line.is_empty() {
            *pos += 1;
            continue;
        }

        // --- delimiter ends the preamble
        if line == "---" {
            *pos += 1;
            break;
        }

        // Comment lines (informational)
        if line.starts_with('#') {
            let content = line.trim_start_matches('#').trim();

            if let Some(rest) = content.strip_prefix("daemon:") {
                daemon_version = Some(rest.trim().to_string());
            } else if let Some(rest) = content.strip_prefix("trust:") {
                trust = Some(rest.trim().to_string());
            } else if let Some(rest) = content.strip_prefix("server_features:") {
                server_features = Some(parse_feature_set(rest.trim()));
            }
            // else: skip other comment lines (nwscript v1 header, etc.)
            *pos += 1;
            continue;
        }

        // `protocol:` line
        if let Some(rest) = line.strip_prefix("protocol:") {
            let ver_str = rest.trim();
            if let Some(v) = parse_version_str(ver_str) {
                protocol_version = v;
            }
            *pos += 1;
            continue;
        }

        // `features:` line
        if let Some(rest) = line.strip_prefix("features:") {
            client_features = parse_feature_set(rest.trim());
            *pos += 1;
            continue;
        }

        // `expect` lines in preamble (handshake expects)
        if line.starts_with("expect ") || line.starts_with("expect:") {
            expects.push(parse_expect(line)?);
            *pos += 1;
            continue;
        }

        bail!("unexpected line in preamble (missing `---` delimiter?): {line}");
    }

    Ok(Preamble {
        protocol_version,
        client_features,
        expects,
        daemon_version,
        trust,
        server_features,
    })
}

fn parse_version_str(ver_str: &str) -> Option<ProtocolVersion> {
    let parts: Vec<&str> = ver_str.split('.').collect();
    if parts.len() == 2 {
        let major: u16 = parts[0].parse().ok()?;
        let minor: u8 = parts[1].parse().ok()?;
        Some(ProtocolVersion::new(major, minor))
    } else {
        None
    }
}

fn parse_feature_set(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.starts_with('{') && s.ends_with('}') {
        let inner = s[1..s.len() - 1].trim();
        if inner.is_empty() {
            Vec::new()
        } else {
            inner
                .split(',')
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect()
        }
    } else if s.is_empty() {
        Vec::new()
    } else {
        // Single feature or space-separated
        s.split_whitespace().map(|f| f.to_string()).collect()
    }
}

fn parse_entry(lines: &[&str], pos: &mut usize) -> Result<Entry> {
    let line = lines[*pos].trim();
    *pos += 1;

    // Parse optional timestamp
    let (timestamp_ms, rest) = if line.starts_with('@') {
        parse_timestamp(line)?
    } else {
        (None, line)
    };

    // Parse op name and inline args
    let op_call = parse_op_line(rest, lines, pos)?;

    // Parse indented lines (expects and comments)
    let mut expects = Vec::new();
    while *pos < lines.len() {
        let next = lines[*pos];
        if !next.starts_with("  ") && !next.starts_with('\t') {
            break;
        }
        let trimmed = next.trim();
        if trimmed.is_empty() {
            *pos += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            // Comment line (response comments from show) -- skip
            *pos += 1;
            continue;
        }
        if trimmed.starts_with("expect") {
            expects.push(parse_expect(trimmed)?);
            *pos += 1;
            continue;
        }
        // Other indented lines are part of the op (already consumed by parse_op_line)
        break;
    }

    Ok(Entry {
        timestamp_ms,
        op_call,
        response: None,
        expects,
    })
}

fn parse_timestamp(line: &str) -> Result<(Option<f64>, &str)> {
    // @0.000ms OpName ...
    let rest = line.strip_prefix('@').unwrap();
    if let Some(ms_pos) = rest.find("ms") {
        let num_str = &rest[..ms_pos];
        let ms: f64 = num_str.parse().context("bad timestamp")?;
        let after = rest[ms_pos + 2..].trim();
        Ok((Some(ms), after))
    } else {
        bail!("malformed timestamp: {line}");
    }
}

fn parse_op_line(line: &str, lines: &[&str], pos: &mut usize) -> Result<OpCall> {
    let mut parts = line.splitn(2, ' ');
    let op_name = parts.next().unwrap_or("");
    let args = parts.next().unwrap_or("").trim();

    let op = Op::from_name(op_name).with_context(|| format!("unknown op: {op_name}"))?;

    match op {
        // Ops with keyword-style args that span multiple lines
        Op::SetOptions => parse_set_options(lines, pos),
        Op::CollectGarbage => parse_collect_garbage(lines, pos),
        Op::VerifyStore => parse_verify_store(lines, pos),
        Op::AddTextToStore => parse_add_text_to_store(lines, pos),
        Op::AddToStore => parse_add_to_store(lines, pos),
        Op::AddMultipleToStore => parse_add_multiple_to_store(lines, pos),

        // No args
        Op::SyncWithGC | Op::FindRoots | Op::QueryAllValidPaths | Op::OptimiseStore => {
            Ok(OpCall::NoArgs { op })
        }

        // Single path
        Op::IsValidPath
        | Op::QueryReferrers
        | Op::QueryDeriver
        | Op::QueryDerivationOutputs
        | Op::QueryDerivationOutputNames
        | Op::QueryDerivationOutputMap
        | Op::QueryValidDerivers
        | Op::QueryPathInfo
        | Op::EnsurePath
        | Op::AddTempRoot
        | Op::NarFromPath => Ok(OpCall::SinglePath {
            op,
            path: args.to_string(),
        }),

        // Single string
        Op::AddIndirectRoot | Op::QueryPathFromHashPart => Ok(OpCall::SingleString {
            op,
            value: args.to_string(),
        }),

        // PathSet + flag
        Op::QueryValidPaths => {
            let (paths, rest) = parse_inline_path_set(args)?;
            let substitute = rest.trim() == "substitute";
            Ok(OpCall::PathSetFlag {
                op,
                paths,
                substitute,
            })
        }

        // PathSet + mode
        Op::BuildPaths | Op::BuildPathsWithResults => {
            let (paths, rest) = parse_inline_path_set(args)?;
            let mode = if rest.trim().is_empty() {
                "normal".to_string()
            } else {
                rest.trim().to_string()
            };
            Ok(OpCall::PathSetMode { op, paths, mode })
        }

        // PathSet only
        Op::QuerySubstitutablePaths | Op::QueryMissing => {
            let (paths, _) = parse_inline_path_set(args)?;
            Ok(OpCall::PathSet { op, paths })
        }

        // AddPermRoot: path gcRoot
        Op::AddPermRoot => {
            let mut parts = args.splitn(2, ' ');
            let path = parts.next().unwrap_or("").to_string();
            let gc_root = parts.next().unwrap_or("").to_string();
            Ok(OpCall::AddPermRoot { path, gc_root })
        }

        // AddSignatures: path { sigs }
        Op::AddSignatures => {
            let (path, sigs_rest) = args.split_once(' ').unwrap_or((args, ""));
            let (sigs, _) = parse_inline_path_set(sigs_rest)?;
            Ok(OpCall::AddSignatures {
                path: path.to_string(),
                sigs,
            })
        }

        // AddBuildLog: path (then framed data on next lines)
        Op::AddBuildLog => {
            let data = parse_inline_framed_data(lines, pos)?;
            Ok(OpCall::AddBuildLog {
                path: args.to_string(),
                data,
            })
        }

        Op::RegisterDrvOutput => Ok(OpCall::RegisterDrvOutput {
            value: args.to_string(),
        }),
        Op::QueryRealisation => Ok(OpCall::QueryRealisation {
            value: args.to_string(),
        }),

        Op::QuerySubstitutablePathInfo => Ok(OpCall::QuerySubstitutablePathInfo {
            path: args.to_string(),
        }),
        Op::QuerySubstitutablePathInfos => {
            let (paths, _) = parse_inline_path_set(args)?;
            Ok(OpCall::QuerySubstitutablePathInfos { paths })
        }

        // Complex ops -- raw bytes
        Op::BuildDerivation | Op::AddToStoreNar => {
            let data = parse_raw_hex(lines, pos);
            Ok(OpCall::RawBytes { op, data })
        }
    }
}

fn parse_inline_path_set(s: &str) -> Result<(Vec<String>, &str)> {
    let s = s.trim();
    if s.starts_with('{') {
        if let Some(end) = s.find('}') {
            let inner = s[1..end].trim();
            let paths: Vec<String> = if inner.is_empty() {
                Vec::new()
            } else {
                inner.split(',').map(|p| p.trim().to_string()).collect()
            };
            let rest = &s[end + 1..];
            Ok((paths, rest))
        } else {
            bail!("unclosed path set brace");
        }
    } else if s.is_empty() {
        Ok((Vec::new(), ""))
    } else {
        // Single item (no braces)
        Ok((vec![s.to_string()], ""))
    }
}

fn parse_set_options(lines: &[&str], pos: &mut usize) -> Result<OpCall> {
    let mut opts = std::collections::HashMap::new();
    let mut overrides = Vec::new();
    let mut in_overrides = false;

    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.starts_with("  ") && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("expect") {
            break;
        }

        if in_overrides {
            // Parse "name" = "value"
            if trimmed.starts_with('"') {
                if let Some((name, value)) = parse_kv_pair(trimmed) {
                    overrides.push((name, value));
                }
                *pos += 1;
                continue;
            } else {
                in_overrides = false;
                // Fall through to normal kv parsing
            }
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if key == "overrides" {
                in_overrides = true;
            } else {
                opts.insert(key.to_string(), value.to_string());
            }
        }
        *pos += 1;
    }

    let get_u64 = |key: &str| -> u64 { opts.get(key).and_then(|v| v.parse().ok()).unwrap_or(0) };

    Ok(OpCall::SetOptions {
        keep_failed: get_u64("keep_failed"),
        keep_going: get_u64("keep_going"),
        try_fallback: get_u64("try_fallback"),
        verbosity: get_u64("verbosity"),
        max_build_jobs: get_u64("max_build_jobs"),
        max_silent_time: get_u64("max_silent_time"),
        use_build_hook: get_u64("use_build_hook"),
        verbose_build: get_u64("verbose_build"),
        log_type: get_u64("log_type"),
        print_build_trace: get_u64("print_build_trace"),
        build_cores: get_u64("build_cores"),
        use_substitutes: get_u64("use_substitutes"),
        overrides,
    })
}

fn parse_collect_garbage(lines: &[&str], pos: &mut usize) -> Result<OpCall> {
    let mut action = 0u64;
    let mut paths = Vec::new();
    let mut ignore_liveness = 0u64;
    let mut max_freed = 0u64;

    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.starts_with("  ") && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("expect") {
            break;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "action" => action = value.parse().unwrap_or(0),
                "paths" => {
                    let (p, _) = parse_inline_path_set(value)?;
                    paths = p;
                }
                "ignore_liveness" => ignore_liveness = value.parse().unwrap_or(0),
                "max_freed" => max_freed = value.parse().unwrap_or(0),
                _ => {}
            }
        }
        *pos += 1;
    }

    Ok(OpCall::CollectGarbage {
        action,
        paths,
        ignore_liveness,
        max_freed,
    })
}

fn parse_verify_store(lines: &[&str], pos: &mut usize) -> Result<OpCall> {
    let mut check_contents = false;
    let mut repair = false;

    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.starts_with("  ") && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("expect") {
            break;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "check_contents" => check_contents = value != "0",
                "repair" => repair = value != "0",
                _ => {}
            }
        }
        *pos += 1;
    }

    Ok(OpCall::VerifyStore {
        check_contents,
        repair,
    })
}

fn parse_add_text_to_store(lines: &[&str], pos: &mut usize) -> Result<OpCall> {
    let mut suffix = String::new();
    let mut text = String::new();
    let mut refs = Vec::new();

    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.starts_with("  ") && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("expect") {
            break;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "suffix" => suffix = unquote(value),
                "text" => text = unquote(value),
                "refs" => {
                    let (r, _) = parse_inline_path_set(value)?;
                    refs = r;
                }
                _ => {}
            }
        }
        *pos += 1;
    }

    Ok(OpCall::AddTextToStore { suffix, text, refs })
}

fn parse_add_to_store(lines: &[&str], pos: &mut usize) -> Result<OpCall> {
    let mut name = String::new();
    let mut cam_str = String::new();
    let mut refs = Vec::new();
    let mut repair = false;
    let mut data = FramedData::Inline(Vec::new());

    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.starts_with("  ") && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("expect") {
            break;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "name" => name = unquote(value),
                "cam_str" => cam_str = unquote(value),
                "refs" => {
                    let (r, _) = parse_inline_path_set(value)?;
                    refs = r;
                }
                "repair" => repair = value != "0",
                "data" => data = parse_data_value(value)?,
                _ => {}
            }
        }
        *pos += 1;
    }

    Ok(OpCall::AddToStore {
        name,
        cam_str,
        refs,
        repair,
        data,
    })
}

fn parse_add_multiple_to_store(lines: &[&str], pos: &mut usize) -> Result<OpCall> {
    let mut repair = false;
    let mut dont_check_sigs = false;
    let mut data = FramedData::Inline(Vec::new());

    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.starts_with("  ") && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("expect") {
            break;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "repair" => repair = value != "0",
                "dont_check_sigs" => dont_check_sigs = value != "0",
                "data" => data = parse_data_value(value)?,
                _ => {}
            }
        }
        *pos += 1;
    }

    Ok(OpCall::AddMultipleToStore {
        repair,
        dont_check_sigs,
        data,
    })
}

fn parse_data_value(value: &str) -> Result<FramedData> {
    if let Some(rest) = value.strip_prefix("x\"") {
        let hex = rest.trim_end_matches('"');
        Ok(FramedData::Inline(hex_decode(hex)))
    } else if let Some(path) = value.strip_prefix("@file:") {
        Ok(FramedData::FileRef(path.to_string()))
    } else {
        bail!("unsupported data format: {value} (expected x\"...\" or @file:...)")
    }
}

fn parse_inline_framed_data(lines: &[&str], pos: &mut usize) -> Result<FramedData> {
    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.starts_with("  ") && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("expect") {
            break;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            if key.trim() == "data" {
                *pos += 1;
                return parse_data_value(value.trim());
            }
        }
        *pos += 1;
    }
    Ok(FramedData::Inline(Vec::new()))
}

fn parse_raw_hex(lines: &[&str], pos: &mut usize) -> Vec<u8> {
    while *pos < lines.len() {
        let line = lines[*pos];
        if !line.starts_with("  ") && !line.starts_with('\t') {
            break;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("expect") {
            break;
        }

        if let Some(rest) = trimmed.strip_prefix("raw x\"") {
            let hex = rest.trim_end_matches('"');
            *pos += 1;
            return hex_decode(hex);
        }
        *pos += 1;
    }
    Vec::new()
}

fn parse_expect(line: &str) -> Result<Expect> {
    // "expect: Last", "expect result: 1", "expect result.valid: 1",
    // "expect error: /pattern/", "expect stderr.count: 0"
    let rest = line
        .strip_prefix("expect")
        .context("not an expect line")?
        .trim();

    if let Some(value) = rest.strip_prefix(':') {
        // "expect: Last" or "expect: Error"
        return Ok(Expect::Terminal(value.trim().to_string()));
    }

    if let Some(rest) = rest.strip_prefix("stderr.count:") {
        let matcher = parse_matcher(rest.trim())?;
        return Ok(Expect::StderrCount { matcher });
    }

    if let Some(rest) = rest.strip_prefix("error:") {
        let matcher = parse_matcher(rest.trim())?;
        return Ok(Expect::Error { matcher });
    }

    if let Some(rest) = rest.strip_prefix("result") {
        let rest = rest.trim();
        if let Some(value) = rest.strip_prefix(':') {
            let matcher = parse_matcher(value.trim())?;
            return Ok(Expect::Result {
                field: None,
                matcher,
            });
        }
        if let Some(rest) = rest.strip_prefix('.') {
            if let Some((field, value)) = rest.split_once(':') {
                let matcher = parse_matcher(value.trim())?;
                return Ok(Expect::Result {
                    field: Some(field.trim().to_string()),
                    matcher,
                });
            }
        }
    }

    // Handshake expects
    if let Some(rest) = rest.strip_prefix("daemon:") {
        let matcher = parse_matcher(rest.trim())?;
        return Ok(Expect::Daemon { matcher });
    }

    if let Some(rest) = rest.strip_prefix("trust:") {
        let matcher = parse_matcher(rest.trim())?;
        return Ok(Expect::Trust { matcher });
    }

    if let Some(rest) = rest.strip_prefix("server_features:") {
        let matcher = parse_matcher(rest.trim())?;
        return Ok(Expect::ServerFeatures { matcher });
    }

    bail!("unrecognized expect: {line}");
}

fn parse_matcher(s: &str) -> Result<Matcher> {
    if s.starts_with('/') && s.ends_with('/') && s.len() > 1 {
        return Ok(Matcher::Regex(s[1..s.len() - 1].to_string()));
    }
    if let Some(rest) = s.strip_prefix(">=") {
        let n: i64 = rest.trim().parse().context("bad number in >=")?;
        return Ok(Matcher::GreaterOrEqual(n));
    }
    if let Some(rest) = s.strip_prefix("<=") {
        let n: i64 = rest.trim().parse().context("bad number in <=")?;
        return Ok(Matcher::LessOrEqual(n));
    }
    if let Some(rest) = s.strip_prefix('>') {
        let n: i64 = rest.trim().parse().context("bad number in >")?;
        return Ok(Matcher::GreaterThan(n));
    }
    if let Some(rest) = s.strip_prefix('<') {
        let n: i64 = rest.trim().parse().context("bad number in <")?;
        return Ok(Matcher::LessThan(n));
    }
    Ok(Matcher::Exact(s.to_string()))
}

fn parse_kv_pair(s: &str) -> Option<(String, String)> {
    // "name" = "value"
    let s = s.trim();
    if !s.starts_with('"') {
        return None;
    }
    let rest = &s[1..];
    let end_name = rest.find('"')?;
    let name = rest[..end_name].to_string();
    let rest = &rest[end_name + 1..];
    let rest = rest.trim();
    let rest = rest.strip_prefix('=')?;
    let rest = rest.trim();
    let rest = rest.strip_prefix('"')?;
    let end_val = rest.find('"')?;
    let value = rest[..end_val].to_string();
    Some((name, value))
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        inner
            .replace("\\n", "\n")
            .replace("\\r", "\r")
            .replace("\\t", "\t")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\")
    } else {
        s.to_string()
    }
}

fn hex_decode(hex: &str) -> Vec<u8> {
    let hex = hex.replace(' ', "");
    let mut result = Vec::with_capacity(hex.len() / 2);
    let mut chars = hex.chars();
    while let (Some(a), Some(b)) = (chars.next(), chars.next()) {
        if let Ok(byte) = u8::from_str_radix(&format!("{a}{b}"), 16) {
            result.push(byte);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_script() {
        let text = r#"# nwscript v1
protocol: 1.38
features: { }
# daemon: 2.33.3
# trust: trusted
---

@0.000ms SyncWithGC
  # response: Last
  # result: 1

@1.234ms IsValidPath /nix/store/abc-hello
  # response: Last
  # result: 1
"#;

        let script = parse_script(text).unwrap();
        assert_eq!(
            script.preamble.protocol_version,
            ProtocolVersion::new(1, 38)
        );
        assert_eq!(script.preamble.daemon_version.as_deref(), Some("2.33.3"));
        assert_eq!(script.preamble.trust.as_deref(), Some("trusted"));
        assert_eq!(script.entries.len(), 2);

        assert_eq!(script.entries[0].timestamp_ms, Some(0.0));
        assert_eq!(script.entries[0].op_call.op(), Op::SyncWithGC);

        assert_eq!(script.entries[1].timestamp_ms, Some(1.234));
        assert_eq!(script.entries[1].op_call.op(), Op::IsValidPath);
    }

    #[test]
    fn parse_set_options() {
        let text = r#"# nwscript v1
protocol: 1.38
features: { }
---

SetOptions
  keep_failed: 0
  keep_going: 0
  try_fallback: 0
  verbosity: 1
  max_build_jobs: 16
  max_silent_time: 0
  use_build_hook: 1
  verbose_build: 0
  log_type: 0
  print_build_trace: 0
  build_cores: 0
  use_substitutes: 1
  overrides: 2
    "substitute" = "true"
    "system" = "x86_64-linux"
"#;

        let script = parse_script(text).unwrap();
        assert_eq!(script.entries.len(), 1);
        match &script.entries[0].op_call {
            OpCall::SetOptions {
                verbosity,
                max_build_jobs,
                overrides,
                ..
            } => {
                assert_eq!(*verbosity, 1);
                assert_eq!(*max_build_jobs, 16);
                assert_eq!(overrides.len(), 2);
                assert_eq!(overrides[0], ("substitute".to_string(), "true".to_string()));
            }
            _ => panic!("expected SetOptions"),
        }
    }

    #[test]
    fn parse_expects() {
        let text = r#"# nwscript v1
protocol: 1.38
features: { }
---

IsValidPath /nix/store/abc-hello
  expect: Last
  expect result: 1

QueryPathInfo /nix/store/abc-hello
  expect: Last
  expect result.valid: 1
  expect result.narSize: > 0

BuildPaths { /nix/store/bad.drv!out } normal
  expect: Error
  expect error: /not valid/
"#;

        let script = parse_script(text).unwrap();
        assert_eq!(script.entries.len(), 3);

        // First entry: exact expects
        assert_eq!(script.entries[0].expects.len(), 2);
        match &script.entries[0].expects[0] {
            Expect::Terminal(s) => assert_eq!(s, "Last"),
            _ => panic!("expected Terminal"),
        }
        match &script.entries[0].expects[1] {
            Expect::Result { field, matcher } => {
                assert!(field.is_none());
                match matcher {
                    Matcher::Exact(s) => assert_eq!(s, "1"),
                    _ => panic!("expected Exact"),
                }
            }
            _ => panic!("expected Result"),
        }

        // Third entry: error + regex
        match &script.entries[2].expects[1] {
            Expect::Error { matcher } => match matcher {
                Matcher::Regex(r) => assert_eq!(r, "not valid"),
                _ => panic!("expected Regex"),
            },
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn parse_path_set() {
        let (paths, rest) = parse_inline_path_set("{ /a, /b, /c }").unwrap();
        assert_eq!(paths, vec!["/a", "/b", "/c"]);
        assert!(rest.trim().is_empty());
    }

    #[test]
    fn parse_path_set_with_mode() {
        let (paths, rest) = parse_inline_path_set("{ /nix/store/abc.drv!out } normal").unwrap();
        assert_eq!(paths, vec!["/nix/store/abc.drv!out"]);
        assert_eq!(rest.trim(), "normal");
    }

    #[test]
    fn unquote_strings() {
        assert_eq!(unquote("\"hello\""), "hello");
        assert_eq!(unquote("\"he\\\"llo\""), "he\"llo");
        assert_eq!(unquote("\"line1\\nline2\""), "line1\nline2");
    }

    #[test]
    fn parse_new_preamble_with_delimiter() {
        let text = r#"# nwscript v1
protocol: 1.38
features: { ca-derivations }
# daemon: 2.33.3
# trust: trusted
# server_features: { }
---

SyncWithGC
"#;

        let script = parse_script(text).unwrap();
        assert_eq!(
            script.preamble.protocol_version,
            ProtocolVersion::new(1, 38)
        );
        assert_eq!(
            script.preamble.client_features,
            vec!["ca-derivations".to_string()]
        );
        assert_eq!(script.preamble.daemon_version.as_deref(), Some("2.33.3"));
        assert_eq!(script.preamble.trust.as_deref(), Some("trusted"));
        assert_eq!(script.preamble.server_features, Some(Vec::<String>::new()));
        assert!(script.preamble.expects.is_empty());
        assert_eq!(script.entries.len(), 1);
        assert_eq!(script.entries[0].op_call.op(), Op::SyncWithGC);
    }

    #[test]
    fn parse_preamble_with_expects() {
        let text = r#"# nwscript v1
protocol: 1.38
features: { }
expect trust: trusted
expect daemon: /^2\./
---

SyncWithGC
"#;

        let script = parse_script(text).unwrap();
        assert_eq!(script.preamble.expects.len(), 2);
        match &script.preamble.expects[0] {
            Expect::Trust { matcher } => match matcher {
                Matcher::Exact(s) => assert_eq!(s, "trusted"),
                _ => panic!("expected Exact matcher"),
            },
            _ => panic!("expected Trust expect"),
        }
        match &script.preamble.expects[1] {
            Expect::Daemon { matcher } => match matcher {
                Matcher::Regex(r) => assert_eq!(r, "^2\\."),
                _ => panic!("expected Regex matcher"),
            },
            _ => panic!("expected Daemon expect"),
        }
    }

    #[test]
    fn parse_empty_features() {
        let text = r#"# nwscript v1
protocol: 1.38
features: { }
---

SyncWithGC
"#;

        let script = parse_script(text).unwrap();
        assert!(script.preamble.client_features.is_empty());
    }

    #[test]
    fn parse_multiple_features() {
        let text = r#"# nwscript v1
protocol: 1.38
features: { ca-derivations, recursive-nix }
---

SyncWithGC
"#;

        let script = parse_script(text).unwrap();
        assert_eq!(
            script.preamble.client_features,
            vec!["ca-derivations".to_string(), "recursive-nix".to_string()]
        );
    }
}
