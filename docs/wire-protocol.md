# Nix daemon wire protocol

The Nix daemon communicates with clients over a Unix socket (or stdio) using
a custom binary protocol. All integers are u64 little-endian unless noted
otherwise.

This document consolidates the protocol knowledge encoded in the `nix-wire`
crate source files.

## Wire primitives

All data is built from a small set of primitive types:

**u64** -- 8-byte unsigned integer, little-endian. Used for magic numbers,
version fields, operation codes, stderr codes, counts, flags, and sizes.

**String** -- Length-prefixed, padded to 8-byte alignment:

```
[u64 length][bytes][zero-padding to 8-byte boundary]
```

The padding is `(8 - (length % 8)) % 8` zero bytes. An empty string is
just a u64 zero (8 bytes total, no padding needed).

**StringSet** -- A counted array of strings:

```
[u64 count][string_1][string_2]...[string_count]
```

**Framed data** -- Used for streaming large payloads (NAR data, build logs).
A sequence of length-prefixed chunks terminated by a zero-length chunk:

```
[u64 chunk_len][chunk_bytes]   (repeated)
[u64 0]                        (terminator)
```

Framed data has no 8-byte padding per chunk -- the length is exact.

## Handshake sequence

The handshake establishes the protocol version and exchanges capabilities.

```
Client                              Daemon
  |                                   |
  |-- WORKER_MAGIC_1 (u64) --------->|    0x6e697863 ("nixc" as LE u32)
  |-- client_version (u64) --------->|    (major << 8) | minor
  |                                   |
  |<--------- WORKER_MAGIC_2 (u64) --|    0x6478696f ("dxio" as LE u32)
  |<--------- server_version (u64) --|
  |                                   |
```

The **negotiated version** is `min(client_version, server_version)`. All
subsequent version-gated behavior uses this negotiated version.

**Feature exchange** (negotiated >= 1.38):

```
  |-- client_features (StringSet) -->|
  |<-- server_features (StringSet) --|
```

**Post-handshake obsolete fields** (client to daemon):

- CPU affinity (>= 1.14): u64 flag, if nonzero followed by u64 affinity value
- reserveSpace (>= 1.11): u64

**Server sends ClientHandshakeInfo**:

- Daemon Nix version string (>= 1.33)
- Trust status u64 (>= 1.35)
- STDERR_LAST to signal handshake completion

After the handshake, the connection enters the operation loop.

## Operation codes

Each operation is sent as a u64 by the client. The daemon processes the
request, sends stderr messages, and then returns the result.

| Code | Name | Notes |
|------|------|-------|
| 1 | IsValidPath | |
| 6 | QueryReferrers | |
| 7 | AddToStore | |
| 8 | AddTextToStore | obsolete since 1.25 |
| 9 | BuildPaths | |
| 10 | EnsurePath | |
| 11 | AddTempRoot | |
| 12 | AddIndirectRoot | |
| 13 | SyncWithGC | |
| 14 | FindRoots | |
| 18 | QueryDeriver | obsolete |
| 19 | SetOptions | |
| 20 | CollectGarbage | |
| 21 | QuerySubstitutablePathInfo | |
| 22 | QueryDerivationOutputs | obsolete |
| 23 | QueryAllValidPaths | |
| 26 | QueryPathInfo | |
| 28 | QueryDerivationOutputNames | obsolete |
| 29 | QueryPathFromHashPart | |
| 30 | QuerySubstitutablePathInfos | |
| 31 | QueryValidPaths | |
| 32 | QuerySubstitutablePaths | |
| 33 | QueryValidDerivers | |
| 34 | OptimiseStore | |
| 35 | VerifyStore | |
| 36 | BuildDerivation | |
| 37 | AddSignatures | |
| 38 | NarFromPath | |
| 39 | AddToStoreNar | |
| 40 | QueryMissing | |
| 41 | QueryDerivationOutputMap | |
| 42 | RegisterDrvOutput | |
| 43 | QueryRealisation | |
| 44 | AddMultipleToStore | |
| 45 | AddBuildLog | |
| 46 | BuildPathsWithResults | |
| 47 | AddPermRoot | |

Gaps in the numbering (2-5, 15-17, 24-25, 27) correspond to removed
operations.

## Stderr message loop

After the client sends an operation and its arguments, the daemon enters a
**stderr message loop**. The daemon sends zero or more non-terminal messages
(logs, activity traces, data transfers), then exactly one terminal message
(STDERR_LAST on success, STDERR_ERROR on failure).

| Code | Name | Wire value | Payload |
|------|------|------------|---------|
| STDERR_NEXT | Log line | `0x6f6c6d67` | string (log message) |
| STDERR_READ | Data request | `0x64617461` | u64 (requested byte count) |
| STDERR_WRITE | Data send | `0x64617416` | string (data) |
| STDERR_LAST | Success | `0x616c7473` | *(none -- result data follows)* |
| STDERR_ERROR | Failure | `0x63787470` | error type (string), level (u64), name (string), message (string), have-pos (u64), optional position |
| STDERR_START_ACTIVITY | Start activity | `0x53545254` | act (u64), lvl (u64), type (u64), message (string), fields (counted typed array), parent (u64) |
| STDERR_STOP_ACTIVITY | Stop activity | `0x53544f50` | act (u64) |
| STDERR_RESULT | Activity result | `0x52534c54` | act (u64), type (u64), fields (counted typed array) |

**Terminal codes**: STDERR_LAST and STDERR_ERROR end the loop. After
STDERR_LAST, the operation's result data follows. After STDERR_ERROR, the
client can send the next operation or close the connection.

**Fields** in START_ACTIVITY and RESULT are encoded as:

```
[u64 count][field_1]...[field_count]

Each field:
  [u64 type]  0 = u64 value, 1 = string value
  [value]
```

## Framed data operations

Starting at protocol version >= 1.23, certain operations send framed data
from the client to the daemon (after the fixed arguments, before the stderr
loop):

- **AddToStore** -- NAR data
- **AddToStoreNar** -- NAR data
- **AddMultipleToStore** -- multiple NARs
- **AddBuildLog** -- build log content

The daemon may also send framed data back for **NarFromPath** (>= 1.23).

## Protocol version feature gates

Behavior changes are gated on the negotiated protocol version:

| Version | Feature |
|---------|---------|
| >= 1.10 | SetOptions includes buildCores |
| >= 1.11 | Post-handshake reserveSpace field |
| >= 1.12 | SetOptions includes useSubstitutes and setting overrides |
| >= 1.14 | Post-handshake CPU affinity field |
| >= 1.16 | QueryPathInfo result includes ultimate, sigs, ca |
| >= 1.23 | Framed data for AddToStore, AddToStoreNar, AddMultipleToStore, AddBuildLog; NarFromPath returns framed |
| >= 1.25 | AddToStore uses new argument format (name, camStr, refs, repair) |
| >= 1.27 | QueryValidPaths includes substitute flag |
| >= 1.28 | BuildResult includes DrvOutputs map |
| >= 1.29 | BuildResult includes timesBuilt, isNonDeterministic, startTime, stopTime |
| >= 1.33 | Post-handshake daemon version string; flush before reading |
| >= 1.35 | Post-handshake trust status |
| >= 1.37 | BuildResult includes cpuUser/cpuSystem (optional durations) |
| >= 1.38 | Feature negotiation StringSets in handshake |

## Upstream references

The wire protocol is defined in the Nix C++ source:

- `src/libstore/include/nix/store/worker-protocol.hh` -- operation codes, magic numbers
- `src/libstore/worker-protocol-connection.cc` -- handshake, framing
- `src/libstore/daemon.cc` -- daemon-side operation dispatch
- `src/libstore/remote-store.cc` -- client-side operation implementation
