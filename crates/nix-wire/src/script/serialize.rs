//! Serialize a Script AST's OpCalls into wire protocol bytes.

use std::io::Write;
use std::path::Path;

use anyhow::Result;

use crate::handshake::{ProtocolVersion, WORKER_MAGIC_1};
use crate::wire;

use super::{FramedData, OpCall, Script};

/// Serialize the client side of the handshake into wire bytes.
pub fn serialize_client_handshake(
    version: ProtocolVersion,
    features: &[String],
) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    wire::write_u64(&mut buf, WORKER_MAGIC_1)?;
    wire::write_u64(&mut buf, version.to_wire())?;
    if version.has_features() {
        wire::write_string_set(&mut buf, features)?;
    }
    if version.has_cpu_affinity() {
        wire::write_u64(&mut buf, 0)?;
    }
    if version.has_reserve_space() {
        wire::write_u64(&mut buf, 0)?;
    }
    Ok(buf)
}

/// Serialize a Script into client-side wire bytes.
///
/// Produces only client->daemon records, suitable for use with
/// the recording format or for direct replay.
///
/// `base_dir` is used to resolve `@file:` references relative to the
/// script's location. Pass `script_path.parent()` when loading from a file.
pub fn serialize_script(script: &Script, base_dir: Option<&Path>) -> Result<Vec<u8>> {
    let version = script.preamble.protocol_version;
    let mut buf = serialize_client_handshake(version, &script.preamble.client_features)?;

    for entry in &script.entries {
        serialize_op_call(&mut buf, &entry.op_call, version, base_dir)?;
    }

    Ok(buf)
}

/// Serialize a single OpCall into wire bytes (op code + args).
pub fn serialize_op_call(
    buf: &mut Vec<u8>,
    op_call: &OpCall,
    version: ProtocolVersion,
    base_dir: Option<&Path>,
) -> Result<()> {
    let op = op_call.op();
    wire::write_u64(buf, op as u64)?;
    serialize_op_args(buf, op_call, version, base_dir)
}

/// Serialize just the arguments of an OpCall (no op code prefix).
pub fn serialize_op_args(
    buf: &mut Vec<u8>,
    op_call: &OpCall,
    version: ProtocolVersion,
    base_dir: Option<&Path>,
) -> Result<()> {
    match op_call {
        OpCall::SinglePath { path, .. } | OpCall::SingleString { value: path, .. } => {
            wire::write_string(buf, path)?;
        }

        OpCall::PathSet { paths, .. } => {
            wire::write_string_set(buf, paths)?;
        }

        OpCall::PathSetFlag {
            paths, substitute, ..
        } => {
            wire::write_string_set(buf, paths)?;
            if version >= ProtocolVersion::new(1, 27) {
                wire::write_u64(buf, *substitute as u64)?;
            }
        }

        OpCall::PathSetMode { paths, mode, .. } => {
            wire::write_string_set(buf, paths)?;
            let mode_val: u64 = match mode.as_str() {
                "normal" => 0,
                "repair" => 1,
                "check" => 2,
                _ => mode.parse().unwrap_or(0),
            };
            wire::write_u64(buf, mode_val)?;
        }

        OpCall::NoArgs { .. } => {
            // No args to write
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
            wire::write_u64(buf, *keep_failed)?;
            wire::write_u64(buf, *keep_going)?;
            wire::write_u64(buf, *try_fallback)?;
            wire::write_u64(buf, *verbosity)?;
            wire::write_u64(buf, *max_build_jobs)?;
            wire::write_u64(buf, *max_silent_time)?;
            wire::write_u64(buf, *use_build_hook)?;
            wire::write_u64(buf, *verbose_build)?;
            wire::write_u64(buf, *log_type)?;
            wire::write_u64(buf, *print_build_trace)?;
            if version >= ProtocolVersion::new(1, 10) {
                wire::write_u64(buf, *build_cores)?;
            }
            if version >= ProtocolVersion::new(1, 12) {
                wire::write_u64(buf, *use_substitutes)?;
            }
            if version >= ProtocolVersion::new(1, 12) {
                wire::write_u64(buf, overrides.len() as u64)?;
                for (name, value) in overrides {
                    wire::write_string(buf, name)?;
                    wire::write_string(buf, value)?;
                }
            }
        }

        OpCall::CollectGarbage {
            action,
            paths,
            ignore_liveness,
            max_freed,
        } => {
            wire::write_u64(buf, *action)?;
            wire::write_string_set(buf, paths)?;
            wire::write_u64(buf, *ignore_liveness)?;
            wire::write_u64(buf, *max_freed)?;
            // 3 obsolete u64s
            wire::write_u64(buf, 0)?;
            wire::write_u64(buf, 0)?;
            wire::write_u64(buf, 0)?;
        }

        OpCall::VerifyStore {
            check_contents,
            repair,
        } => {
            wire::write_u64(buf, *check_contents as u64)?;
            wire::write_u64(buf, *repair as u64)?;
        }

        OpCall::AddPermRoot { path, gc_root } => {
            wire::write_string(buf, path)?;
            wire::write_string(buf, gc_root)?;
        }

        OpCall::AddSignatures { path, sigs } => {
            wire::write_string(buf, path)?;
            wire::write_string_set(buf, sigs)?;
        }

        OpCall::AddTextToStore { suffix, text, refs } => {
            wire::write_string(buf, suffix)?;
            wire::write_string(buf, text)?;
            wire::write_string_set(buf, refs)?;
        }

        OpCall::AddToStore {
            name,
            cam_str,
            refs,
            repair,
            data,
        } => {
            wire::write_string(buf, name)?;
            wire::write_string(buf, cam_str)?;
            wire::write_string_set(buf, refs)?;
            wire::write_u64(buf, *repair as u64)?;
            write_framed_data(buf, data, base_dir)?;
        }

        OpCall::AddMultipleToStore {
            repair,
            dont_check_sigs,
            data,
        } => {
            wire::write_u64(buf, *repair as u64)?;
            wire::write_u64(buf, *dont_check_sigs as u64)?;
            write_framed_data(buf, data, base_dir)?;
        }

        OpCall::AddBuildLog { path, data } => {
            wire::write_string(buf, path)?;
            write_framed_data(buf, data, base_dir)?;
        }

        OpCall::RegisterDrvOutput { value } | OpCall::QueryRealisation { value } => {
            wire::write_string(buf, value)?;
        }

        OpCall::QuerySubstitutablePathInfo { path } => {
            wire::write_string(buf, path)?;
        }

        OpCall::QuerySubstitutablePathInfos { paths } => {
            wire::write_string_set(buf, paths)?;
        }

        OpCall::RawBytes { data, .. } => {
            buf.write_all(data)?;
        }
    }

    Ok(())
}

fn write_framed_data(buf: &mut Vec<u8>, data: &FramedData, base_dir: Option<&Path>) -> Result<()> {
    match data {
        FramedData::Inline(bytes) => {
            wire::write_framed(buf, bytes)?;
        }
        FramedData::FileRef(path) => {
            let resolved = match base_dir {
                Some(dir) => {
                    let p = std::path::PathBuf::from(path);
                    if p.is_relative() {
                        dir.join(&p)
                    } else {
                        p
                    }
                }
                None => std::path::PathBuf::from(path),
            };
            let file_data = std::fs::read(&resolved).map_err(|e| {
                anyhow::anyhow!(
                    "failed to read framed data file {}: {}",
                    resolved.display(),
                    e
                )
            })?;
            wire::write_framed(buf, &file_data)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::Op;
    use crate::script::{Entry, Preamble};
    use std::io::Cursor;

    #[test]
    fn serialize_simple_script() {
        let script = Script {
            preamble: Preamble {
                protocol_version: ProtocolVersion::new(1, 38),
                client_features: Vec::new(),
                expects: Vec::new(),
                daemon_version: None,
                trust: None,
                server_features: None,
            },
            entries: vec![
                Entry {
                    timestamp_ms: None,
                    op_call: OpCall::NoArgs { op: Op::SyncWithGC },
                    response: None,
                    expects: Vec::new(),
                },
                Entry {
                    timestamp_ms: None,
                    op_call: OpCall::SinglePath {
                        op: Op::IsValidPath,
                        path: "/nix/store/abc-hello".to_string(),
                    },
                    response: None,
                    expects: Vec::new(),
                },
            ],
        };

        let bytes = serialize_script(&script, None).unwrap();

        // Verify handshake
        let mut cursor = Cursor::new(&bytes);
        let magic = wire::read_u64(&mut cursor).unwrap();
        assert_eq!(magic, WORKER_MAGIC_1);
        let ver = wire::read_u64(&mut cursor).unwrap();
        assert_eq!(ver, ProtocolVersion::new(1, 38).to_wire());

        // Features count=0
        let features = wire::read_u64(&mut cursor).unwrap();
        assert_eq!(features, 0);

        // CPU affinity flag=0
        let affinity = wire::read_u64(&mut cursor).unwrap();
        assert_eq!(affinity, 0);

        // reserveSpace=0
        let reserve = wire::read_u64(&mut cursor).unwrap();
        assert_eq!(reserve, 0);

        // Op 1: SyncWithGC (13)
        let op_code = wire::read_u64(&mut cursor).unwrap();
        assert_eq!(op_code, Op::SyncWithGC as u64);

        // Op 2: IsValidPath (1) + path
        let op_code = wire::read_u64(&mut cursor).unwrap();
        assert_eq!(op_code, Op::IsValidPath as u64);
        let path = wire::read_string(&mut cursor).unwrap();
        assert_eq!(path, "/nix/store/abc-hello");
    }

    #[test]
    fn serialize_set_options() {
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
                response: None,
                expects: Vec::new(),
            }],
        };

        let bytes = serialize_script(&script, None).unwrap();
        assert!(!bytes.is_empty());
    }
}
