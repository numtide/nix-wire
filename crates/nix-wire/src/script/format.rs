//! Format a Script AST to `.nwscript` text output.

use std::fmt::Write;
use std::path::PathBuf;

use super::{
    DaemonResponse, Entry, Expect, FramedData, OpCall, PathInfoResult, ResultData, Script,
};

/// Options for formatting a script.
pub struct FormatOptions {
    /// Directory to write large data files into.
    /// When set, framed data exceeding `inline_threshold` bytes is written
    /// to a file in this directory and referenced as `@file:path`.
    pub data_dir: Option<PathBuf>,
    /// Maximum inline data size in bytes. Data larger than this is written
    /// to a file when `data_dir` is set. Default: 64.
    pub inline_threshold: usize,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            data_dir: None,
            inline_threshold: 64,
        }
    }
}

/// Format a Script to a .nwscript text string with options.
///
/// When `opts.data_dir` is set, large framed data is written to files in
/// that directory and referenced as `@file:path` in the output.
pub fn format_script(script: &Script, opts: &FormatOptions) -> String {
    let mut out = String::new();
    let mut file_counter = 0u32;

    // Preamble
    writeln!(out, "# nwscript v1").unwrap();
    writeln!(out, "protocol: {}", script.preamble.protocol_version).unwrap();
    writeln!(
        out,
        "features: {}",
        format_path_set(&script.preamble.client_features)
    )
    .unwrap();
    // Daemon response as informational comments
    if let Some(ref ver) = script.preamble.daemon_version {
        writeln!(out, "# daemon: {ver}").unwrap();
    }
    if let Some(ref trust) = script.preamble.trust {
        writeln!(out, "# trust: {trust}").unwrap();
    }
    if let Some(ref feats) = script.preamble.server_features {
        writeln!(out, "# server_features: {}", format_path_set(feats)).unwrap();
    }
    // Preamble expects
    for expect in &script.preamble.expects {
        format_preamble_expect(&mut out, expect);
    }
    writeln!(out, "---").unwrap();

    // Entries
    for entry in &script.entries {
        writeln!(out).unwrap();
        format_entry(&mut out, entry, opts, &mut file_counter);
    }

    out
}

fn format_entry(out: &mut String, entry: &Entry, opts: &FormatOptions, file_counter: &mut u32) {
    // Timestamp
    let ts = match entry.timestamp_ms {
        Some(ms) => format!("@{ms:.3}ms "),
        None => String::new(),
    };

    match &entry.op_call {
        OpCall::SinglePath { op, path } => {
            writeln!(out, "{ts}{} {path}", op.name()).unwrap();
        }

        OpCall::SingleString { op, value } => {
            writeln!(out, "{ts}{} {value}", op.name()).unwrap();
        }

        OpCall::PathSet { op, paths } => {
            let set_str = format_path_set(paths);
            writeln!(out, "{ts}{} {set_str}", op.name()).unwrap();
        }

        OpCall::PathSetFlag {
            op,
            paths,
            substitute,
        } => {
            let set_str = format_path_set(paths);
            if *substitute {
                writeln!(out, "{ts}{} {set_str} substitute", op.name()).unwrap();
            } else {
                writeln!(out, "{ts}{} {set_str}", op.name()).unwrap();
            }
        }

        OpCall::PathSetMode { op, paths, mode } => {
            let set_str = format_path_set(paths);
            writeln!(out, "{ts}{} {set_str} {mode}", op.name()).unwrap();
        }

        OpCall::NoArgs { op } => {
            writeln!(out, "{ts}{}", op.name()).unwrap();
        }

        OpCall::SetOptions {
            keep_failed,
            keep_going,
            try_fallback,
            verbosity,
            max_build_jobs,
            max_silent_time,
            use_build_hook,
            verbose_build,
            log_type,
            print_build_trace,
            build_cores,
            use_substitutes,
            overrides,
        } => {
            writeln!(out, "{ts}SetOptions").unwrap();
            writeln!(out, "  keep_failed: {keep_failed}").unwrap();
            writeln!(out, "  keep_going: {keep_going}").unwrap();
            writeln!(out, "  try_fallback: {try_fallback}").unwrap();
            writeln!(out, "  verbosity: {verbosity}").unwrap();
            writeln!(out, "  max_build_jobs: {max_build_jobs}").unwrap();
            writeln!(out, "  max_silent_time: {max_silent_time}").unwrap();
            writeln!(out, "  use_build_hook: {use_build_hook}").unwrap();
            writeln!(out, "  verbose_build: {verbose_build}").unwrap();
            writeln!(out, "  log_type: {log_type}").unwrap();
            writeln!(out, "  print_build_trace: {print_build_trace}").unwrap();
            writeln!(out, "  build_cores: {build_cores}").unwrap();
            writeln!(out, "  use_substitutes: {use_substitutes}").unwrap();
            writeln!(out, "  overrides: {}", overrides.len()).unwrap();
            for (name, value) in overrides {
                writeln!(out, "    \"{name}\" = \"{value}\"").unwrap();
            }
        }

        OpCall::CollectGarbage {
            action,
            paths,
            ignore_liveness,
            max_freed,
        } => {
            writeln!(out, "{ts}CollectGarbage").unwrap();
            writeln!(out, "  action: {action}").unwrap();
            writeln!(out, "  paths: {}", format_path_set(paths)).unwrap();
            writeln!(out, "  ignore_liveness: {ignore_liveness}").unwrap();
            writeln!(out, "  max_freed: {max_freed}").unwrap();
        }

        OpCall::VerifyStore {
            check_contents,
            repair,
        } => {
            writeln!(out, "{ts}VerifyStore").unwrap();
            writeln!(out, "  check_contents: {}", *check_contents as u64).unwrap();
            writeln!(out, "  repair: {}", *repair as u64).unwrap();
        }

        OpCall::AddPermRoot { path, gc_root } => {
            writeln!(out, "{ts}AddPermRoot {path} {gc_root}").unwrap();
        }

        OpCall::AddSignatures { path, sigs } => {
            let set_str = format_path_set(sigs);
            writeln!(out, "{ts}AddSignatures {path} {set_str}").unwrap();
        }

        OpCall::AddTextToStore { suffix, text, refs } => {
            writeln!(out, "{ts}AddTextToStore").unwrap();
            writeln!(out, "  suffix: \"{suffix}\"").unwrap();
            writeln!(out, "  text: \"{}\"", escape_string(text)).unwrap();
            writeln!(out, "  refs: {}", format_path_set(refs)).unwrap();
        }

        OpCall::AddToStore {
            name,
            cam_str,
            refs,
            repair,
            data,
        } => {
            writeln!(out, "{ts}AddToStore").unwrap();
            writeln!(out, "  name: \"{name}\"").unwrap();
            writeln!(out, "  cam_str: \"{cam_str}\"").unwrap();
            writeln!(out, "  refs: {}", format_path_set(refs)).unwrap();
            writeln!(out, "  repair: {}", *repair as u64).unwrap();
            format_framed_data(out, data, opts, file_counter);
        }

        OpCall::AddMultipleToStore {
            repair,
            dont_check_sigs,
            data,
        } => {
            writeln!(out, "{ts}AddMultipleToStore").unwrap();
            writeln!(out, "  repair: {}", *repair as u64).unwrap();
            writeln!(out, "  dont_check_sigs: {}", *dont_check_sigs as u64).unwrap();
            format_framed_data(out, data, opts, file_counter);
        }

        OpCall::AddBuildLog { path, data } => {
            writeln!(out, "{ts}AddBuildLog {path}").unwrap();
            format_framed_data(out, data, opts, file_counter);
        }

        OpCall::RegisterDrvOutput { value } => {
            writeln!(out, "{ts}RegisterDrvOutput {value}").unwrap();
        }

        OpCall::QueryRealisation { value } => {
            writeln!(out, "{ts}QueryRealisation {value}").unwrap();
        }

        OpCall::QuerySubstitutablePathInfo { path } => {
            writeln!(out, "{ts}QuerySubstitutablePathInfo {path}").unwrap();
        }

        OpCall::QuerySubstitutablePathInfos { paths } => {
            let set_str = format_path_set(paths);
            writeln!(out, "{ts}QuerySubstitutablePathInfos {set_str}").unwrap();
        }

        OpCall::RawBytes { op, data } => {
            writeln!(out, "{ts}{}", op.name()).unwrap();
            if !data.is_empty() {
                writeln!(out, "  raw x\"{}\"", hex_encode(data)).unwrap();
            }
        }
    }

    // Expects
    for expect in &entry.expects {
        format_expect(out, expect);
    }

    // Response comments
    if let Some(ref resp) = entry.response {
        format_response_comment(out, resp);
    }
}

fn format_expect(out: &mut String, expect: &Expect) {
    match expect {
        Expect::Terminal(code) => {
            writeln!(out, "  expect: {code}").unwrap();
        }
        Expect::Result { field, matcher } => {
            let field_str = match field {
                Some(f) => format!("result.{f}"),
                None => "result".to_string(),
            };
            writeln!(out, "  expect {field_str}: {matcher}").unwrap();
        }
        Expect::Error { matcher } => {
            writeln!(out, "  expect error: {matcher}").unwrap();
        }
        Expect::StderrCount { matcher } => {
            writeln!(out, "  expect stderr.count: {matcher}").unwrap();
        }
        Expect::Daemon { matcher } => {
            writeln!(out, "  expect daemon: {matcher}").unwrap();
        }
        Expect::Trust { matcher } => {
            writeln!(out, "  expect trust: {matcher}").unwrap();
        }
        Expect::ServerFeatures { matcher } => {
            writeln!(out, "  expect server_features: {matcher}").unwrap();
        }
    }
}

fn format_preamble_expect(out: &mut String, expect: &Expect) {
    match expect {
        Expect::Daemon { matcher } => {
            writeln!(out, "expect daemon: {matcher}").unwrap();
        }
        Expect::Trust { matcher } => {
            writeln!(out, "expect trust: {matcher}").unwrap();
        }
        Expect::ServerFeatures { matcher } => {
            writeln!(out, "expect server_features: {matcher}").unwrap();
        }
        // Other expect types in preamble get generic formatting
        other => {
            let mut tmp = String::new();
            format_expect(&mut tmp, other);
            // Remove leading indent for preamble-level expects
            write!(out, "{}", tmp.trim_start()).unwrap();
        }
    }
}

fn format_response_comment(out: &mut String, resp: &DaemonResponse) {
    writeln!(out, "  # response: {}", resp.terminal).unwrap();
    if resp.stderr_count > 1 {
        writeln!(out, "  # stderr.count: {}", resp.stderr_count).unwrap();
    }
    if let Some(ref err) = resp.error {
        writeln!(out, "  # error: {}", err.message).unwrap();
    }
    if let Some(ref result) = resp.result {
        format_result_comment(out, result);
    }
}

fn format_result_comment(out: &mut String, result: &ResultData) {
    match result {
        ResultData::U64(v) => {
            writeln!(out, "  # result: {v}").unwrap();
        }
        ResultData::Str(s) => {
            writeln!(out, "  # result: {s}").unwrap();
        }
        ResultData::PathInfo(info) => {
            format_path_info_comment(out, info);
        }
        ResultData::StringSet(set) => {
            writeln!(out, "  # result: {}", format_path_set(set)).unwrap();
        }
        ResultData::StringMap(pairs) => {
            writeln!(out, "  # result.count: {}", pairs.len()).unwrap();
            for (k, v) in pairs.iter().take(5) {
                writeln!(out, "  # result: \"{k}\" = \"{v}\"").unwrap();
            }
            if pairs.len() > 5 {
                writeln!(out, "  # ... and {} more", pairs.len() - 5).unwrap();
            }
        }
        ResultData::None => {}
        ResultData::CollectGarbage { bytes_freed } => {
            writeln!(out, "  # result.bytes_freed: {bytes_freed}").unwrap();
        }
        ResultData::SubstitutablePathInfo {
            valid,
            deriver,
            download_size,
            nar_size,
            ..
        } => {
            writeln!(out, "  # result.valid: {}", *valid as u64).unwrap();
            if let Some(ref d) = deriver {
                writeln!(out, "  # result.deriver: {d}").unwrap();
            }
            if let Some(ds) = download_size {
                writeln!(out, "  # result.downloadSize: {ds}").unwrap();
            }
            if let Some(ns) = nar_size {
                writeln!(out, "  # result.narSize: {ns}").unwrap();
            }
        }
        ResultData::Missing {
            will_build,
            will_substitute,
            unknown,
            download_size,
            nar_size,
        } => {
            writeln!(out, "  # result.willBuild: {}", format_path_set(will_build)).unwrap();
            writeln!(
                out,
                "  # result.willSubstitute: {}",
                format_path_set(will_substitute)
            )
            .unwrap();
            writeln!(out, "  # result.unknown: {}", format_path_set(unknown)).unwrap();
            writeln!(out, "  # result.downloadSize: {download_size}").unwrap();
            writeln!(out, "  # result.narSize: {nar_size}").unwrap();
        }
        ResultData::Framed(data) => {
            writeln!(out, "  # result: framed {} bytes", data.len()).unwrap();
        }
        ResultData::Raw(data) => {
            writeln!(out, "  # result: raw {} bytes", data.len()).unwrap();
        }
    }
}

fn format_path_info_comment(out: &mut String, info: &PathInfoResult) {
    writeln!(out, "  # result.valid: {}", info.valid as u64).unwrap();
    if let Some(ref d) = info.deriver {
        if !d.is_empty() {
            writeln!(out, "  # result.deriver: {d}").unwrap();
        }
    }
    if let Some(ref h) = info.nar_hash {
        writeln!(out, "  # result.narHash: {h}").unwrap();
    }
    if let Some(ref refs) = info.references {
        writeln!(out, "  # result.references: {}", format_path_set(refs)).unwrap();
    }
    if let Some(ns) = info.nar_size {
        writeln!(out, "  # result.narSize: {ns}").unwrap();
    }
}

fn format_path_set(paths: &[String]) -> String {
    if paths.is_empty() {
        return "{ }".to_string();
    }
    let items: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
    format!("{{ {} }}", items.join(", "))
}

fn format_framed_data(
    out: &mut String,
    data: &FramedData,
    opts: &FormatOptions,
    file_counter: &mut u32,
) {
    match data {
        FramedData::Inline(bytes) => {
            if bytes.len() > opts.inline_threshold {
                if let Some(ref dir) = opts.data_dir {
                    let filename = format!("{file_counter:04}.bin");
                    let path = dir.join(&filename);
                    if let Err(e) = std::fs::write(&path, bytes) {
                        // Fall back to inline if write fails
                        eprintln!("warning: failed to write {}: {e}", path.display());
                        writeln!(out, "  data: x\"{}\"", hex_encode(bytes)).unwrap();
                        return;
                    }
                    *file_counter += 1;
                    writeln!(out, "  data: @file:{filename}").unwrap();
                    return;
                }
            }
            writeln!(out, "  data: x\"{}\"", hex_encode(bytes)).unwrap();
        }
        FramedData::FileRef(path) => {
            writeln!(out, "  data: @file:{path}").unwrap();
        }
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handshake::ProtocolVersion;
    use crate::ops::Op;
    use crate::script::Preamble;

    #[test]
    fn format_simple_script() {
        let script = Script {
            preamble: Preamble {
                protocol_version: ProtocolVersion::new(1, 38),
                client_features: Vec::new(),
                expects: Vec::new(),
                daemon_version: Some("2.33.3".to_string()),
                trust: Some("trusted".to_string()),
                server_features: None,
            },
            entries: vec![
                Entry {
                    timestamp_ms: Some(0.0),
                    op_call: OpCall::NoArgs { op: Op::SyncWithGC },
                    response: Some(DaemonResponse {
                        terminal: "Last".to_string(),
                        stderr_count: 1,
                        result: Some(ResultData::U64(1)),
                        error: None,
                    }),
                    expects: Vec::new(),
                },
                Entry {
                    timestamp_ms: Some(1.234),
                    op_call: OpCall::SinglePath {
                        op: Op::IsValidPath,
                        path: "/nix/store/abc-hello".to_string(),
                    },
                    response: Some(DaemonResponse {
                        terminal: "Last".to_string(),
                        stderr_count: 1,
                        result: Some(ResultData::U64(1)),
                        error: None,
                    }),
                    expects: Vec::new(),
                },
            ],
        };

        let text = format_script(&script, &FormatOptions::default());
        assert!(text.contains("# nwscript v1"));
        assert!(text.contains("protocol: 1.38"));
        assert!(
            !text.contains("# protocol:"),
            "protocol should not be a comment"
        );
        assert!(text.contains("features: { }"));
        assert!(text.contains("# daemon: 2.33.3"));
        assert!(text.contains("---"));
        assert!(text.contains("@0.000ms SyncWithGC"));
        assert!(text.contains("@1.234ms IsValidPath /nix/store/abc-hello"));
        assert!(text.contains("# response: Last"));
        assert!(text.contains("# result: 1"));
    }

    #[test]
    fn format_set_options() {
        let script = Script {
            preamble: Preamble {
                protocol_version: ProtocolVersion::new(1, 38),
                client_features: Vec::new(),
                expects: Vec::new(),
                daemon_version: None,
                trust: None,
                server_features: None,
            },
            entries: vec![Entry {
                timestamp_ms: None,
                op_call: OpCall::SetOptions {
                    keep_failed: 0,
                    keep_going: 0,
                    try_fallback: 0,
                    verbosity: 1,
                    max_build_jobs: 16,
                    max_silent_time: 0,
                    use_build_hook: 1,
                    verbose_build: 0,
                    log_type: 0,
                    print_build_trace: 0,
                    build_cores: 0,
                    use_substitutes: 1,
                    overrides: vec![("substitute".to_string(), "true".to_string())],
                },
                response: Some(DaemonResponse {
                    terminal: "Last".to_string(),
                    stderr_count: 1,
                    result: Some(ResultData::None),
                    error: None,
                }),
                expects: Vec::new(),
            }],
        };

        let text = format_script(&script, &FormatOptions::default());
        assert!(text.contains("SetOptions"));
        assert!(text.contains("  keep_failed: 0"));
        assert!(text.contains("  verbosity: 1"));
        assert!(text.contains("  overrides: 1"));
        assert!(text.contains("    \"substitute\" = \"true\""));
    }

    #[test]
    fn format_path_set_display() {
        assert_eq!(format_path_set(&[]), "{ }");
        assert_eq!(
            format_path_set(&["a".to_string(), "b".to_string()]),
            "{ a, b }"
        );
    }

    #[test]
    fn roundtrip_format_parse() {
        use crate::script::parse::parse_script;

        let script = Script {
            preamble: Preamble {
                protocol_version: ProtocolVersion::new(1, 38),
                client_features: Vec::new(),
                expects: Vec::new(),
                daemon_version: Some("2.33.3".to_string()),
                trust: Some("trusted".to_string()),
                server_features: None,
            },
            entries: vec![
                Entry {
                    timestamp_ms: Some(0.0),
                    op_call: OpCall::NoArgs { op: Op::SyncWithGC },
                    response: None,
                    expects: Vec::new(),
                },
                Entry {
                    timestamp_ms: Some(1.234),
                    op_call: OpCall::SinglePath {
                        op: Op::IsValidPath,
                        path: "/nix/store/abc-hello".to_string(),
                    },
                    response: None,
                    expects: Vec::new(),
                },
                Entry {
                    timestamp_ms: Some(2.0),
                    op_call: OpCall::PathSet {
                        op: Op::QueryMissing,
                        paths: vec![
                            "/nix/store/aaa.drv!out".to_string(),
                            "/nix/store/bbb.drv!out".to_string(),
                        ],
                    },
                    response: None,
                    expects: Vec::new(),
                },
                Entry {
                    timestamp_ms: None,
                    op_call: OpCall::SetOptions {
                        keep_failed: 0,
                        keep_going: 0,
                        try_fallback: 0,
                        verbosity: 1,
                        max_build_jobs: 16,
                        max_silent_time: 0,
                        use_build_hook: 1,
                        verbose_build: 0,
                        log_type: 0,
                        print_build_trace: 0,
                        build_cores: 0,
                        use_substitutes: 1,
                        overrides: vec![
                            ("substitute".to_string(), "true".to_string()),
                            ("system".to_string(), "x86_64-linux".to_string()),
                        ],
                    },
                    response: None,
                    expects: Vec::new(),
                },
            ],
        };

        // Format to text
        let text = format_script(&script, &FormatOptions::default());

        // Parse back
        let parsed = parse_script(&text).expect("roundtrip parse failed");

        // Verify preamble
        assert_eq!(
            parsed.preamble.protocol_version,
            script.preamble.protocol_version
        );
        assert_eq!(
            parsed.preamble.daemon_version,
            script.preamble.daemon_version
        );
        assert_eq!(parsed.preamble.trust, script.preamble.trust);

        // Verify entry count
        assert_eq!(parsed.entries.len(), script.entries.len());

        // Verify ops
        assert_eq!(parsed.entries[0].op_call.op(), Op::SyncWithGC);
        assert_eq!(parsed.entries[0].timestamp_ms, Some(0.0));

        assert_eq!(parsed.entries[1].op_call.op(), Op::IsValidPath);
        assert_eq!(parsed.entries[1].timestamp_ms, Some(1.234));
        if let OpCall::SinglePath { path, .. } = &parsed.entries[1].op_call {
            assert_eq!(path, "/nix/store/abc-hello");
        } else {
            panic!("expected SinglePath");
        }

        assert_eq!(parsed.entries[2].op_call.op(), Op::QueryMissing);
        if let OpCall::PathSet { paths, .. } = &parsed.entries[2].op_call {
            assert_eq!(paths.len(), 2);
            assert_eq!(paths[0], "/nix/store/aaa.drv!out");
        } else {
            panic!("expected PathSet");
        }

        assert_eq!(parsed.entries[3].op_call.op(), Op::SetOptions);
        if let OpCall::SetOptions {
            verbosity,
            max_build_jobs,
            overrides,
            ..
        } = &parsed.entries[3].op_call
        {
            assert_eq!(*verbosity, 1);
            assert_eq!(*max_build_jobs, 16);
            assert_eq!(overrides.len(), 2);
            assert_eq!(overrides[0].0, "substitute");
        } else {
            panic!("expected SetOptions");
        }

        // Format again and compare text output
        let text2 = format_script(&parsed, &FormatOptions::default());
        assert_eq!(text, text2, "double roundtrip text mismatch");
    }
}
