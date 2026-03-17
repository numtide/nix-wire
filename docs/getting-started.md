# Getting started

This tutorial walks through recording, decoding, and replaying a Nix daemon
wire protocol session.

## Prerequisites

- Nix installed and running (the `nix-daemon` socket at `/nix/var/nix/daemon-socket/socket`)
- The nix-wire tools built:

```
nix build
```

The binaries are in `./result/bin/`.

## Step 1: Record a session

The recorder interposes on the Nix daemon Unix socket. It renames the real
socket, binds its own socket in the same location, and proxies traffic while
writing every byte to a `.nixwire` file.

Start the recorder:

```
sudo ./result/bin/nix-wire-record
```

The recorder takes over the daemon socket and begins listening. In another
terminal, trigger a Nix operation:

```
nix path-info nixpkgs#hello
```

Stop the recorder with Ctrl-C. It restores the original socket on exit.

Recordings are written to `/nix/var/nix/nix-wire/` by default.

### Command mode

Instead of interposing on the daemon socket, you can wrap a command directly:

```
nix-wire-record -- nix-daemon --stdio
```

In command mode, the recorder spawns the given command with stdio connected
to the recorder. This is useful for recording `ssh-ng://` remote sessions
(see [Recording sessions](recording-sessions.md)).

## Step 2: Decode the recording

Parse the recording to see what happened at the protocol level:

```
./result/bin/nix-wire-decode --recording /nix/var/nix/nix-wire/0000.nixwire
```

The decoder prints three sections:

**Handshake** -- Protocol versions, feature exchange, daemon Nix version,
and trust status:

```
Handshake: client=1.38, server=1.38, negotiated=1.38
  daemon nix version: 2.24.0
  trust: trusted (1)
```

**Operations** -- One line per operation with timing and size:

```
[     3.963ms] SetOptions                          req=   104B  stderr=1       0.029ms  STDERR_LAST  0 overrides
[     4.018ms] AddTempRoot                         req=    72B  stderr=1       0.086ms  STDERR_LAST  /nix/store/...
```

Each line shows:

- Timestamp offset from session start
- Operation name
- Request size in bytes
- Number of stderr messages
- Stderr loop duration
- Terminal code (STDERR_LAST = success, STDERR_ERROR = failure)
- Summary of the operation arguments

**Summary** -- Total operations, bytes transferred, session duration.

### JSON output

For programmatic consumption:

```
nix-wire-decode --recording 0000.nixwire --format json
```

### Sync warnings

If the decoder encounters data that does not match expected protocol
structure, it emits `SYNC WARNING` messages to stderr. These are diagnostic
-- the decoder continues best-effort.

## Step 3: Aggregate statistics

Get a high-level summary of what happened in a recording:

```
./result/bin/nix-wire-stats --recording /nix/var/nix/nix-wire/0000.nixwire
```

This shows per-operation-type counts and timing (total, avg, min, max), the
slowest individual operations, and the most frequently queried store paths.
JSON output is also available with `--format json`.

## Step 4: Replay the recording

Send the client side of a recording to a daemon and read back responses:

```
./result/bin/nix-wire-replay --recording /nix/var/nix/nix-wire/0000.nixwire
```

This replays against the local daemon by default. You can also replay against
a remote daemon or a command:

```
nix-wire-replay --recording 0000.nixwire -- nix-daemon --stdio
nix-wire-replay --recording 0000.nixwire -- ssh user@host nix-daemon --stdio
```

## Step 5: Unpack to a script directory

Unpack a binary recording into a directory with a human-readable `.nwscript`
file and any associated data files:

```
./result/bin/nix-wire-script unpack --recording /nix/var/nix/nix-wire/0000.nixwire --output ./unpacked
```

This creates a self-contained directory:

```
unpacked/
  script.nwscript
  0000.bin
  0001.bin
  ...
```

Data files are referenced as `@file:0000.bin` (filename only), so the directory
is portable. Data under 64 bytes is inlined as hex. Adjust with
`--inline-threshold`.

Without `--output`, the script is printed to stdout with all data inlined
(useful for quick inspection):

```
./result/bin/nix-wire-script unpack --recording /nix/var/nix/nix-wire/0000.nixwire
```

The output looks like this:

```
# nwscript v1
protocol: 1.38
features: { }
# daemon: 2.33.3
# trust: trusted
# server_features: { }
---

@3.209ms SetOptions
  keep_failed: 0
  keep_going: 0
  ...
  # response: Last

@3.255ms AddTempRoot /nix/store/abc...-source
  # response: Last
  # result: 1

@3.346ms IsValidPath /nix/store/abc...-source
  # response: Last
  # result: 1

@3.474ms QueryPathInfo /nix/store/abc...-source
  # response: Last
  # result.valid: 1
  # result.narHash: fcb5e4ac...
  # result.references: { }
  # result.narSize: 121584
```

Daemon responses are emitted as `# response:` / `# result:` comments. You can
edit these into `expect:` assertions for testing.

## Step 6: Write and run scripts

Create a `.nwscript` file with expect assertions:

```
# nwscript v1
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
  overrides: 0
  expect: Last

IsValidPath /nix/store/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-does-not-exist
  expect: Last
  expect result: 0

SyncWithGC
  expect: Last
```

Run it against the local daemon:

```
./result/bin/nix-wire-script run --script my-test.nwscript
```

Output:

```
connected: daemon 2.33.3 (protocol 1.38)
  op 0: SetOptions PASS: terminal = Last
  op 1: IsValidPath PASS: terminal = Last
  op 1: IsValidPath PASS: result = 0
  op 2: SyncWithGC PASS: terminal = Last

4/4 expects passed, 0 failed
```

Use `--fail-fast` to stop on the first failure.

See the [nwscript format reference](nwscript-format.md) for the full format
specification and `examples/` for more scripts.

## Step 7: Pack scripts into recordings

Pack a `.nwscript` back into a `.nixwire` recording (client side only):

```
./result/bin/nix-wire-script pack --script my-test.nwscript --output my-test.nixwire
```

`@file:` references are resolved relative to the script file's directory, so
unpacked directories work directly with `pack`.

This produces a `.nixwire` file with only client-to-daemon records, which can
be used with `nix-wire-replay`.

## Next steps

- [nwscript format](nwscript-format.md) -- `.nwscript` text format reference
- [Recording sessions](recording-sessions.md) -- remote recording, output management
- [Wire protocol reference](wire-protocol.md) -- protocol internals
- [Recording format](recording-format.md) -- `.nixwire` binary format
