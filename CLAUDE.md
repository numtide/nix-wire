# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

nix-wire: tools for recording, replaying, decoding, and scripting Nix daemon wire protocol sessions. A Rust workspace with three crates, built with Nix (using numtide/blueprint).

## Build & Test

```sh
nix develop                        # enter dev shell (cargo, rustc, rustfmt, clippy, rust-analyzer)
cargo build                        # build all crates
cargo test                         # run all tests
cargo test -p nix-wire             # test one crate
cargo test -p nix-wire-tools parse_handshake  # run a single test by name
cargo clippy                       # lint
cargo fmt --check                  # check formatting
nix build --log-format bar-with-logs  # full Nix build (produces result/bin/nix-wire-{record,decode,replay,stats,script})
```

Recording requires root access to interpose on the daemon socket:
```sh
sudo ./result/bin/nix-wire-record  # Ctrl-C to stop
```

## Architecture

### Workspace layout (`crates/`)

**nix-wire** -- Core protocol library (no IO, no CLI). All wire format parsing lives here.
- `wire.rs` / `wire_async.rs` -- Low-level primitives: read/write u64, u32, length-prefixed padded strings, framed data. Sync (`std::io`) and async (`tokio::io::AsyncBufRead`) variants. `AsyncWireReader` wraps a buffered reader and provides peek support needed by the stderr loop.
- `handshake.rs` -- Magic numbers (`WORKER_MAGIC_1`, `WORKER_MAGIC_2`), `ProtocolVersion` type with version-gated feature checks (`has_features()`, `has_cpu_affinity()`, etc.).
- `ops.rs` -- `Op` enum mapping all 37 active operation codes from `worker-protocol.hh`.
- `stderr.rs` -- `StderrCode` enum (Next, Read, Write, Last, Error, StartActivity, StopActivity, Result). `Last` and `Error` are terminal codes.
- `protocol.rs` -- High-level async protocol parsing: `parse_handshake`, `skip_op_args` (per-op argument consumption), `read_stderr_loop` (peek-based detection), `skip_daemon_result` (per-op result skipping). This is the main module tools build on.

**nix-wire-recording** -- `.nixwire` binary format: 24-byte header (magic `NIXWREC\0`, version, epoch_ns), then sequential records (offset_ns, direction byte, length, data). `RecordingWriter` / `RecordingReader` for file I/O.

**nix-wire-tools** -- CLI binaries using clap.
- `record.rs` -- Proxy that renames daemon socket to `socket.orig`, listens in its place, records bidirectional traffic with nanosecond timestamps. Restores socket on shutdown.
- `decode.rs` -- Loads a recording, splits into client/daemon byte streams using `MemReader`, runs the full protocol parser, prints operation traces. Supports `--format text|json`.
- `replay.rs` -- Pre-parses recording to identify ops, then sends client records to a live daemon while protocol-parsing responses. Supports `--compare` and `--timing`.
- `stats.rs` -- Aggregates per-operation statistics from a recording: counts, timing distributions (min/max/avg/p50), byte volumes, top queried paths, slowest individual ops. Supports `--format text|json`.
- `script.rs` -- Human-readable protocol scripting with three subcommands:
  - `unpack` -- unpack `.nixwire` to a directory with `.nwscript` + data files
  - `pack` -- pack `.nwscript` + data files into `.nixwire` recording
  - `run` -- compile + send to daemon + evaluate expect assertions

### Script module (`nix-wire/src/script/`)

The `script` module in the core library provides the AST and processing pipeline:
- `mod.rs` -- AST types: `Script`, `Preamble`, `Entry`, `OpCall` (enum with ~20 variants), `Expect`, `Matcher`, `DaemonResponse`, `ResultData`
- `decompile.rs` -- `.nixwire` recording -> Script AST (uses `AsyncWireReader<MemReader>` for position tracking)
- `format.rs` -- Script AST -> `.nwscript` text (with response comments)
- `parse.rs` -- `.nwscript` text -> Script AST (line-oriented parser)
- `serialize.rs` -- Script AST -> wire protocol bytes
- `expect.rs` -- evaluate `Expect` assertions against `DaemonResponse`

### Protocol parsing flow

The decoder and replayer share the same pattern:
1. Split recording into two byte streams (client, daemon) by direction
2. `parse_handshake()` reads both streams in interleaved order
3. Loop: read op code from client -> `skip_op_args()` -> `skip_framed()` if applicable -> `read_stderr_loop()` from daemon -> `skip_daemon_result()`

Version-dependent behavior is threaded through `ProtocolVersion` comparisons (features at 1.38, framed data at 1.23, etc.).

### Nix integration

Uses numtide/blueprint with `prefix = "nix"`. Nix package is at `nix/package.nix` (rustPlatform.buildRustPackage), dev shell at `nix/devshell.nix`.
