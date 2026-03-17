# Recording format (.nixwire)

The `.nixwire` binary format captures a full bidirectional Nix daemon session
with nanosecond timestamps. All integers are little-endian.

## Header (24 bytes)

| Offset | Size | Field | Value |
|--------|------|-------|-------|
| 0 | 8 | magic | `NIXWREC\0` (bytes `4e 49 58 57 52 45 43 00`) |
| 8 | 2 | version | `1` |
| 10 | 2 | flags | `0` (reserved for future use) |
| 12 | 8 | epoch_ns | Unix timestamp in nanoseconds at session start |
| 20 | 4 | reserved | `0` |

The magic bytes and version are validated on read. Unknown versions are
rejected.

## Records (variable length, sequential until EOF)

Each record captures one chunk of data flowing through the socket:

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 8 | offset_ns | Nanoseconds since `epoch_ns` |
| 8 | 1 | direction | `0` = client to daemon, `1` = daemon to client |
| 9 | 4 | length | Byte count of following data |
| 13 | length | data | Raw wire bytes |

Records are written sequentially with no padding between them. A record is
13 + `length` bytes on disk.

## Fragmentation

A single wire protocol message may span multiple records. The recorder writes
data as it arrives from the socket -- it does not attempt to align records to
protocol message boundaries. Decoders must reassemble the byte stream from
records before parsing protocol messages.

## Timestamps

- `epoch_ns` in the header is an absolute Unix timestamp (nanoseconds since
  1970-01-01 00:00:00 UTC).
- `offset_ns` in each record is relative to `epoch_ns`. To get the absolute
  time of a record: `epoch_ns + offset_ns`.

## File naming convention

The recorder writes files named `NNNN.nixwire` (zero-padded, 4+ digits) into
the output directory. The default output directory is
`<store>/var/nix/nix-wire/` (e.g. `/nix/var/nix/nix-wire/`).

The sequence number is determined by scanning existing files in the output
directory and incrementing past the highest existing ID. Concurrent sessions
use an atomic counter to avoid collisions.

Examples: `0000.nixwire`, `0001.nixwire`, `0042.nixwire`.

## Rust API

The `nix-wire-recording` crate provides `RecordingWriter` and
`RecordingReader` for streaming access to this format. See `cargo doc` for
API details.
