//! Decompile a `.nixwire` recording into a Script AST.

use anyhow::Result;

use crate::handshake::ProtocolVersion;
use crate::ops::Op;
use crate::protocol;
use crate::stderr::StderrCode;
use crate::wire_async::{AsyncWireReader, MemReader};

use super::{terminal_name, DaemonResponse, Entry, OpCall, Preamble, Script};

/// Decompile from two pre-split byte streams (client and daemon).
///
/// The caller should split the recording records by direction into
/// client_bytes and daemon_bytes, and provide timestamp info.
pub async fn decompile_streams(
    client: &mut AsyncWireReader<MemReader>,
    daemon: &mut AsyncWireReader<MemReader>,
    timestamps: &[(usize, f64)],
) -> Result<Script> {
    // Parse handshake
    let info = protocol::parse_handshake(client, daemon).await?;
    let version = info.negotiated_version;

    let trust = info.trust_status.map(|t| match t {
        1 => "trusted".to_string(),
        2 => "not-trusted".to_string(),
        _ => "unknown".to_string(),
    });

    let preamble = Preamble {
        protocol_version: version,
        client_features: info.client_features.clone(),
        expects: Vec::new(),
        daemon_version: info.daemon_nix_version,
        trust,
        server_features: Some(info.server_features.clone()),
    };

    let mut entries = Vec::new();

    loop {
        // Try to read next op code
        let op_code = match client.peek_u64().await? {
            Some(v) => v,
            None => break,
        };

        // Look up timestamp from current position in the MemReader
        let client_pos = client.inner_ref().position();
        let timestamp_ms = lookup_timestamp_ms(timestamps, client_pos);

        client.consume(8);

        let op = Op::from_u64(op_code);

        // Read args
        let op_call = if let Some(o) = op {
            match protocol::read_op_args(o, version, client).await? {
                Some(call) => call,
                None => {
                    // Complex op - need raw bytes. Skip args first.
                    let _ = protocol::skip_op_args(o, version, client).await;
                    if version >= ProtocolVersion::new(1, 23)
                        && protocol::op_has_client_framed_data(o)
                    {
                        client.skip_framed().await?;
                    }
                    OpCall::RawBytes {
                        op: o,
                        data: Vec::new(),
                    }
                }
            }
        } else {
            // Unknown op - can't parse
            OpCall::RawBytes {
                op: Op::IsValidPath, // placeholder
                data: Vec::new(),
            }
        };

        // Read daemon response
        let (stderr_result, error_info) = protocol::read_stderr_with_error(daemon).await?;

        let result = if stderr_result.terminal == Some(StderrCode::Last) {
            if let Some(o) = op {
                protocol::read_daemon_result(o, version, daemon).await.ok()
            } else {
                None
            }
        } else {
            None
        };

        let response = Some(DaemonResponse {
            terminal: terminal_name(stderr_result.terminal),
            stderr_count: stderr_result.count,
            result,
            error: error_info,
        });

        entries.push(Entry {
            timestamp_ms,
            op_call,
            response,
            expects: Vec::new(),
        });
    }

    Ok(Script { preamble, entries })
}

/// Look up timestamp in ms from the timestamp index.
fn lookup_timestamp_ms(timestamps: &[(usize, f64)], byte_offset: usize) -> Option<f64> {
    if timestamps.is_empty() {
        return None;
    }
    match timestamps.binary_search_by_key(&byte_offset, |&(off, _)| off) {
        Ok(i) => Some(timestamps[i].1),
        Err(0) => Some(timestamps[0].1),
        Err(i) => Some(timestamps[i - 1].1),
    }
}
