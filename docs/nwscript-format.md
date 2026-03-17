# .nwscript format reference

The `.nwscript` format is a human-readable text representation of Nix daemon
protocol sessions. It can be generated from recordings (`unpack`), hand-authored,
and executed against a daemon (`run`).

## Preamble

Every script starts with a preamble section terminated by `---`:

```
# nwscript v1
protocol: 1.38
features: { ca-derivations }
# daemon: 2.33.3
# trust: trusted
# server_features: { }
expect trust: trusted
expect daemon: /^2\./
---
```

- `# nwscript v1` -- format identifier (magic comment)
- `protocol:` (required) -- client protocol version as `major.minor`
- `features:` (optional) -- client features to advertise during handshake
- `# daemon:` (informational) -- daemon's Nix version string (comment, from `unpack`)
- `# trust:` (informational) -- trust status (comment, from `unpack`)
- `# server_features:` (informational) -- server features (comment, from `unpack`)
- `expect` lines -- assertions on daemon handshake response (for `run`)
- `---` -- delimiter ending the preamble

The `---` delimiter is required.

## Operations

Each operation is one or more lines. Simple ops are one-liners; complex ops
use indented keyword arguments.

### Timestamps

Timestamps are optional, prefixed with `@`:

```
@3.209ms SetOptions
@0.000ms IsValidPath /nix/store/abc...-hello
IsValidPath /nix/store/xyz...-world
```

If omitted, timestamps are auto-assigned with 1ms spacing during `pack`.

### Op categories

**No arguments:**

```
SyncWithGC
FindRoots
QueryAllValidPaths
OptimiseStore
```

**Single path:**

```
IsValidPath /nix/store/abc...-hello
QueryPathInfo /nix/store/abc...-hello
AddTempRoot /nix/store/abc...-hello
EnsurePath /nix/store/abc...-hello
NarFromPath /nix/store/abc...-hello
QueryReferrers /nix/store/abc...-hello
QueryDerivationOutputMap /nix/store/abc...-hello.drv
```

**Single string:**

```
AddIndirectRoot /home/user/project/result
QueryPathFromHashPart abc123def456
```

**Path set:**

```
QuerySubstitutablePaths { /nix/store/aaa, /nix/store/bbb }
QueryMissing { /nix/store/abc.drv!out, /nix/store/xyz.drv!out }
```

**Path set + flag:**

```
QueryValidPaths { /nix/store/abc, /nix/store/xyz } substitute
```

**Path set + build mode:**

```
BuildPaths { /nix/store/abc.drv!out } normal
BuildPathsWithResults { /nix/store/abc.drv!out } normal
```

Build modes: `normal`, `repair`, `check`.

**SetOptions (keyword args):**

```
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
```

**CollectGarbage (keyword args):**

```
CollectGarbage
  action: 0
  paths: { }
  ignore_liveness: 0
  max_freed: 18446744073709551615
```

**VerifyStore:**

```
VerifyStore
  check_contents: 1
  repair: 0
```

**AddToStore (keyword args + framed data):**

```
AddToStore
  name: "hello.txt"
  cam_str: "text:sha256"
  refs: { }
  repair: 0
  data: x"0d0000000000000068656c6c6f"
```

**Framed data** is specified as:
- `x"hex..."` -- inline hex-encoded bytes
- `@file:path/to/file` -- reference to an external file

## Comments

Lines starting with `#` are comments (ignored by the parser). The `unpack`
command emits daemon responses as comments:

```
@3.346ms IsValidPath /nix/store/abc...-hello
  # response: Last
  # result: 1

@3.474ms QueryPathInfo /nix/store/abc...-hello
  # response: Last
  # result.valid: 1
  # result.narHash: fcb5e4ac...
  # result.references: { /nix/store/xyz...-glibc }
  # result.narSize: 226552
```

## Expect assertions

Expects are used with the `run` command to assert on daemon responses. They
are indented lines starting with `expect`:

```
IsValidPath /nix/store/abc...-hello
  expect: Last
  expect result: 1

QueryPathInfo /nix/store/abc...-hello
  expect: Last
  expect result.valid: 1
  expect result.narSize: > 0

BuildPaths { /nix/store/bad.drv!out } normal
  expect: Error
  expect error: /not valid/

QueryMissing { /nix/store/abc.drv!out }
  expect: Last
  expect stderr.count: >= 0
```

### Expect types

**Op-level expects** (indented under an op):

- `expect: Last` / `expect: Error` -- assert on the terminal stderr code
- `expect result: VALUE` -- assert on the result value
- `expect result.FIELD: VALUE` -- assert on a result field (e.g., `valid`,
  `narSize`, `narHash`, `deriver`, `references`)
- `expect error: VALUE` -- assert on the error message
- `expect stderr.count: VALUE` -- assert on the number of stderr messages

**Handshake expects** (in the preamble, before `---`):

- `expect daemon: VALUE` -- assert on the daemon's Nix version
- `expect trust: VALUE` -- assert on the trust status
- `expect server_features: VALUE` -- assert on server features

### Matchers

- `1` or `hello` -- exact string match
- `/pattern/` -- regex match
- `> 0` -- greater than
- `< 100` -- less than
- `>= 1` -- greater or equal
- `<= 99` -- less or equal

## Workflow

A typical workflow:

1. Record a session: `nix-wire-record`
2. Unpack to script: `nix-wire-script unpack --recording 0000.nixwire --output ./session`
3. Edit: convert `# response:` / `# result:` comments into `expect:` assertions
4. Run: `nix-wire-script run --script session.nwscript`
5. Iterate: adjust expects, add new ops, re-run

Or write scripts from scratch using the op reference above.
