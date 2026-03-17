//! nix-wire-stats: Aggregate per-operation statistics from `.nixwire` recordings.
//!
//! Parses the bidirectional protocol stream using the shared async protocol
//! library and collects per-op timing, byte volumes, and error counts.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use serde::Serialize;

use nix_wire::handshake::ProtocolVersion;
use nix_wire::ops::Op;
use nix_wire::protocol;
use nix_wire::stderr::StderrCode;
use nix_wire::wire_async::{AsyncWireReader, MemReader};
use nix_wire_recording::{Direction, RecordingReader};

#[derive(Parser)]
#[command(
    name = "nix-wire-stats",
    about = "Aggregate per-operation statistics from Nix daemon wire protocol recordings"
)]
struct Args {
    /// Path to the .nixwire recording file
    #[arg(long)]
    recording: PathBuf,

    /// Output format
    #[arg(long, default_value = "text")]
    format: OutputFormat,
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Text,
    Json,
}

/// Byte offset -> timestamp mapping for one direction.
type TimestampIndex = Vec<(usize, u64)>;

/// Data split by direction, with timestamp indexes.
struct SplitRecords {
    client_bytes: Vec<u8>,
    daemon_bytes: Vec<u8>,
    client_ts: TimestampIndex,
    daemon_ts: TimestampIndex,
}

/// Split records into separate client and daemon byte streams.
fn split_records(records: &[nix_wire_recording::Record]) -> SplitRecords {
    let mut result = SplitRecords {
        client_bytes: Vec::new(),
        daemon_bytes: Vec::new(),
        client_ts: Vec::new(),
        daemon_ts: Vec::new(),
    };

    for rec in records {
        match rec.direction {
            Direction::ClientToDaemon => {
                result
                    .client_ts
                    .push((result.client_bytes.len(), rec.offset_ns));
                result.client_bytes.extend_from_slice(&rec.data);
            }
            Direction::DaemonToClient => {
                result
                    .daemon_ts
                    .push((result.daemon_bytes.len(), rec.offset_ns));
                result.daemon_bytes.extend_from_slice(&rec.data);
            }
        }
    }

    result
}

/// Look up the timestamp for a byte offset using a timestamp index.
fn lookup_timestamp(ts_index: &[(usize, u64)], byte_offset: usize) -> u64 {
    if ts_index.is_empty() {
        return 0;
    }
    match ts_index.binary_search_by_key(&byte_offset, |&(off, _)| off) {
        Ok(i) => ts_index[i].1,
        Err(0) => ts_index[0].1,
        Err(i) => ts_index[i - 1].1,
    }
}

/// Per-operation record collected during parsing.
struct OpRecord {
    op_name: String,
    duration_ns: u64,
    request_bytes: usize,
    framed_bytes: Option<u64>,
    success: bool,
    arg_info: Option<String>,
}

/// Aggregated statistics for one operation type.
#[derive(Serialize)]
struct OpStats {
    op_name: String,
    count: usize,
    error_count: usize,
    total_duration_ms: f64,
    min_duration_ms: f64,
    max_duration_ms: f64,
    avg_duration_ms: f64,
    p50_duration_ms: f64,
    total_request_bytes: u64,
    total_framed_bytes: u64,
}

/// Session-level summary.
#[derive(Serialize)]
struct SessionSummary {
    epoch_ns: u64,
    total_ops: usize,
    total_errors: usize,
    total_duration_ms: f64,
    client_to_daemon_bytes: usize,
    daemon_to_client_bytes: usize,
}

/// Top individual operation by duration.
#[derive(Serialize)]
struct SlowestOp {
    duration_ms: f64,
    op_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    arg_info: Option<String>,
}

/// Full JSON output structure.
#[derive(Serialize)]
struct StatsOutput {
    session: SessionSummary,
    op_stats: Vec<OpStats>,
    slowest_ops: Vec<SlowestOp>,
    top_paths: Vec<PathCount>,
}

#[derive(Serialize)]
struct PathCount {
    path: String,
    count: usize,
}

fn ns_to_ms(ns: u64) -> f64 {
    ns as f64 / 1_000_000.0
}

/// Compute percentile from a sorted slice (nearest-rank).
fn percentile(sorted: &[u64], pct: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((pct / 100.0) * sorted.len() as f64).ceil() as usize;
    let idx = rank.saturating_sub(1).min(sorted.len() - 1);
    ns_to_ms(sorted[idx])
}

fn aggregate(op_records: &[OpRecord]) -> (Vec<OpStats>, Vec<SlowestOp>, Vec<PathCount>) {
    // Group by op_name
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, rec) in op_records.iter().enumerate() {
        groups.entry(rec.op_name.clone()).or_default().push(i);
    }

    let mut stats: Vec<OpStats> = groups
        .iter()
        .map(|(name, indices)| {
            let mut durations: Vec<u64> =
                indices.iter().map(|&i| op_records[i].duration_ns).collect();
            durations.sort_unstable();

            let count = indices.len();
            let error_count = indices.iter().filter(|&&i| !op_records[i].success).count();
            let total_ns: u64 = durations.iter().sum();
            let min_ns = durations[0];
            let max_ns = durations[durations.len() - 1];
            let total_request_bytes: u64 = indices
                .iter()
                .map(|&i| op_records[i].request_bytes as u64)
                .sum();
            let total_framed_bytes: u64 = indices
                .iter()
                .map(|&i| op_records[i].framed_bytes.unwrap_or(0))
                .sum();

            OpStats {
                op_name: name.clone(),
                count,
                error_count,
                total_duration_ms: ns_to_ms(total_ns),
                min_duration_ms: ns_to_ms(min_ns),
                max_duration_ms: ns_to_ms(max_ns),
                avg_duration_ms: ns_to_ms(total_ns) / count as f64,
                p50_duration_ms: percentile(&durations, 50.0),
                total_request_bytes,
                total_framed_bytes,
            }
        })
        .collect();

    // Sort by total duration descending
    stats.sort_by(|a, b| {
        b.total_duration_ms
            .partial_cmp(&a.total_duration_ms)
            .unwrap()
    });

    // Top 5 slowest individual operations
    let mut by_duration: Vec<(usize, u64)> = op_records
        .iter()
        .enumerate()
        .map(|(i, r)| (i, r.duration_ns))
        .collect();
    by_duration.sort_by(|a, b| b.1.cmp(&a.1));
    let slowest: Vec<SlowestOp> = by_duration
        .iter()
        .take(5)
        .map(|&(i, _)| {
            let rec = &op_records[i];
            SlowestOp {
                duration_ms: ns_to_ms(rec.duration_ns),
                op_name: rec.op_name.clone(),
                arg_info: rec.arg_info.clone().filter(|s| !s.is_empty()),
            }
        })
        .collect();

    // Top queried paths
    let mut path_counts: HashMap<String, usize> = HashMap::new();
    for rec in op_records {
        if let Some(ref info) = rec.arg_info {
            if !info.is_empty() && info.starts_with('/') {
                *path_counts.entry(info.clone()).or_default() += 1;
            }
        }
    }
    let mut top_paths: Vec<PathCount> = path_counts
        .into_iter()
        .map(|(path, count)| PathCount { path, count })
        .collect();
    top_paths.sort_by(|a, b| b.count.cmp(&a.count));
    top_paths.truncate(10);

    (stats, slowest, top_paths)
}

async fn collect_ops(split: &SplitRecords) -> Result<(protocol::HandshakeInfo, Vec<OpRecord>)> {
    let mut client = AsyncWireReader::new(MemReader::new(split.client_bytes.clone()));
    let mut daemon = AsyncWireReader::new(MemReader::new(split.daemon_bytes.clone()));

    let info = protocol::parse_handshake(&mut client, &mut daemon).await?;
    let negotiated_version = info.negotiated_version;
    let uses_framed = negotiated_version >= ProtocolVersion::new(1, 23);

    let mut op_records = Vec::new();

    loop {
        let consumed_before = client.inner_ref().position();

        let op_code = match client.peek_u64().await? {
            Some(v) => v,
            None => break,
        };
        client.consume(8);

        let op_start_ns = lookup_timestamp(&split.client_ts, consumed_before);

        // Skip clearly invalid op codes
        if op_code == 0 || op_code > 64 {
            break;
        }

        let op = Op::from_u64(op_code);
        let op_name = op
            .map(|o| o.name().to_string())
            .unwrap_or_else(|| format!("Unknown({})", op_code));

        // Skip fixed arguments
        let arg_info = match op {
            Some(o) => match protocol::skip_op_args(o, negotiated_version, &mut client).await {
                Ok(info) => info,
                Err(_) => break,
            },
            None => break,
        };

        // Skip framed data if applicable
        let mut framed_bytes = None;
        if uses_framed {
            if let Some(o) = op {
                if protocol::op_has_client_framed_data(o) {
                    match client.skip_framed().await {
                        Ok(total) => framed_bytes = Some(total),
                        Err(_) => break,
                    }
                }
            }
        }

        let request_bytes = client.inner_ref().position() - consumed_before;

        // Daemon position before stderr for timing
        let daemon_pos_before = daemon.inner_ref().position();

        let stderr_result = match protocol::read_stderr_loop(&mut daemon).await {
            Ok(r) => r,
            Err(_) => break,
        };

        let success = stderr_result.terminal == Some(StderrCode::Last);

        // Skip daemon result after STDERR_LAST
        if success {
            if let Some(o) = op {
                if protocol::skip_daemon_result(o, negotiated_version, &mut daemon)
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }

        let daemon_pos_after = daemon.inner_ref().position();
        let op_end_ns = if daemon_pos_after > daemon_pos_before {
            lookup_timestamp(&split.daemon_ts, daemon_pos_after.saturating_sub(1))
        } else {
            op_start_ns
        };
        let duration_ns = op_end_ns.saturating_sub(op_start_ns);

        op_records.push(OpRecord {
            op_name,
            duration_ns,
            request_bytes,
            framed_bytes,
            success,
            arg_info,
        });
    }

    Ok((info, op_records))
}

fn format_timestamp(epoch_ns: u64) -> String {
    let secs = epoch_ns / 1_000_000_000;
    let nanos = epoch_ns % 1_000_000_000;
    format!("{secs}.{nanos:09}")
}

fn print_text(
    header: &nix_wire_recording::Header,
    records: &[nix_wire_recording::Record],
    info: &protocol::HandshakeInfo,
    op_records: &[OpRecord],
) {
    let total_c2d: usize = records
        .iter()
        .filter(|r| r.direction == Direction::ClientToDaemon)
        .map(|r| r.data.len())
        .sum();
    let total_d2c: usize = records
        .iter()
        .filter(|r| r.direction == Direction::DaemonToClient)
        .map(|r| r.data.len())
        .sum();
    let total_duration = records
        .first()
        .zip(records.last())
        .map(|(f, l)| Duration::from_nanos(l.offset_ns - f.offset_ns));

    println!("=== Nix Wire Protocol Statistics ===");
    println!(
        "Session: {}  Protocol: {}  Ops: {}",
        format_timestamp(header.epoch_ns),
        info.negotiated_version,
        op_records.len(),
    );
    if let Some(dur) = total_duration {
        println!("Duration: {:.3}ms", dur.as_secs_f64() * 1000.0);
    }
    println!();

    let (stats, slowest, top_paths) = aggregate(op_records);

    // Op type table
    println!(
        "{:<30} {:>5} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "Op Type", "Count", "Total ms", "Avg ms", "Min ms", "Max ms", "Req Bytes"
    );
    println!("{}", "-".repeat(95));
    for s in &stats {
        println!(
            "{:<30} {:>5} {:>10.3} {:>10.3} {:>10.3} {:>10.3} {:>10}",
            s.op_name,
            s.count,
            s.total_duration_ms,
            s.avg_duration_ms,
            s.min_duration_ms,
            s.max_duration_ms,
            s.total_request_bytes,
        );
    }
    println!();

    // Slowest individual operations
    if !slowest.is_empty() {
        println!("Top {} slowest operations:", slowest.len());
        for (i, s) in slowest.iter().enumerate() {
            let extra = s
                .arg_info
                .as_deref()
                .map(|s| format!("  {s}"))
                .unwrap_or_default();
            println!(
                "  {}. [{:>10.3}ms] {}{}",
                i + 1,
                s.duration_ms,
                s.op_name,
                extra
            );
        }
        println!();
    }

    // Top queried paths
    if !top_paths.is_empty() {
        println!("Top queried paths:");
        for p in &top_paths {
            println!("  {:>4}x  {}", p.count, p.path);
        }
        println!();
    }

    // Totals
    let total_errors = op_records.iter().filter(|r| !r.success).count();
    println!("Totals:");
    println!("  Client -> Daemon: {} bytes", total_c2d);
    println!("  Daemon -> Client: {} bytes", total_d2c);
    if total_errors > 0 {
        println!("  Errors: {}", total_errors);
    }
}

fn print_json(
    header: &nix_wire_recording::Header,
    records: &[nix_wire_recording::Record],
    op_records: &[OpRecord],
) -> Result<()> {
    let total_c2d: usize = records
        .iter()
        .filter(|r| r.direction == Direction::ClientToDaemon)
        .map(|r| r.data.len())
        .sum();
    let total_d2c: usize = records
        .iter()
        .filter(|r| r.direction == Direction::DaemonToClient)
        .map(|r| r.data.len())
        .sum();
    let total_duration_ms = records
        .first()
        .zip(records.last())
        .map(|(f, l)| ns_to_ms(l.offset_ns - f.offset_ns))
        .unwrap_or(0.0);

    let total_errors = op_records.iter().filter(|r| !r.success).count();

    let (op_stats, slowest_ops, top_paths) = aggregate(op_records);

    let output = StatsOutput {
        session: SessionSummary {
            epoch_ns: header.epoch_ns,
            total_ops: op_records.len(),
            total_errors,
            total_duration_ms,
            client_to_daemon_bytes: total_c2d,
            daemon_to_client_bytes: total_d2c,
        },
        op_stats,
        slowest_ops,
        top_paths,
    };

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let file = std::fs::File::open(&args.recording)?;
    let mut reader = RecordingReader::new(std::io::BufReader::new(file))?;

    let header = *reader.header();
    let records = reader.read_all()?;

    let split = split_records(&records);
    let (info, op_records) = collect_ops(&split).await?;

    match args.format {
        OutputFormat::Text => {
            print_text(&header, &records, &info, &op_records);
            Ok(())
        }
        OutputFormat::Json => print_json(&header, &records, &op_records),
    }
}
