# nix-wire

A collection of tools for the Nix daemon wire protocol.

Record, replay, decode, and script Nix daemon wire protocol sessions.
`nix-wire` interposes on the Nix daemon Unix socket to capture the full
bidirectional byte stream with nanosecond timestamps. Recordings can then be
decoded into human-readable operation traces, replayed against a daemon, or
decompiled into editable `.nwscript` text files with expect assertions.

## Quick start

```
nix build
sudo ./result/bin/nix-wire-record          # record (Ctrl-C to stop)
./result/bin/nix-wire-decode --recording /nix/var/nix/nix-wire/0000.nixwire
./result/bin/nix-wire-script unpack --recording /nix/var/nix/nix-wire/0000.nixwire
./result/bin/nix-wire-script run --script examples/path-validation.nwscript
```

See [Getting started](docs/getting-started.md) for a full walkthrough.

## Tools

**nix-wire-record** -- Proxy that sits between Nix clients and the daemon
socket, recording every session to a `.nixwire` file.

**nix-wire-decode** -- Parses a recording and prints the protocol handshake,
each operation with timing/size, and a session summary.

**nix-wire-replay** -- Sends the client side of a recording to the daemon and
reads back responses.

**nix-wire-stats** -- Aggregates per-operation statistics (counts, timing
distributions, byte volumes, top queried paths) from a recording.

**nix-wire-script** -- Human-readable protocol scripting:
- `unpack` -- unpack a `.nixwire` recording to a directory with `.nwscript` + data files
- `pack` -- pack a `.nwscript` file into a `.nixwire` recording
- `run` -- compile + send to a daemon + evaluate expect assertions

## Examples

The `examples/` directory contains hand-written `.nwscript` files that can be
run against any Nix daemon:

- [`path-validation.nwscript`](examples/path-validation.nwscript) -- validate
  store paths and query metadata with expect assertions
- [`store-query.nwscript`](examples/store-query.nwscript) -- miscellaneous
  store queries including error handling
- [`nix-develop-session.nwscript`](examples/nix-develop-session.nwscript) --
  decompiled `nix develop` session showing real protocol traffic

## Library crates

**nix-wire** -- Core protocol library with async wire protocol parsing
(handshake, operations, stderr loop, wire primitives).

**nix-wire-recording** -- Read/write `.nixwire` recording files with
nanosecond timestamps.

## Documentation

- [Getting started](docs/getting-started.md) -- tutorial walkthrough
- [nwscript format](docs/nwscript-format.md) -- `.nwscript` text format reference
- [Recording sessions](docs/recording-sessions.md) -- remote recording, output management
- [Wire protocol reference](docs/wire-protocol.md) -- Nix daemon wire protocol spec
- [Recording format](docs/recording-format.md) -- `.nixwire` binary format spec

## Building

```
nix build
```

Or enter the dev shell and use cargo directly:

```
nix develop
cargo build
cargo test
```

## License

MIT
