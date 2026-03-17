# nix-wire toolbox ideas

Utility ideas brainstormed for the nix-wire project, organized by priority.

## Tier 1: High value, builds directly on existing library

### 1. `nix-wire-stats` -- Protocol statistics analyzer

- Aggregate op counts, timing distributions, byte volumes per op type
- Identify slow ops, hot store paths, repeated queries
- Text and JSON output formats
- Complexity: small -- mostly formatting on top of existing `protocol.rs` parsing
- File: `src/bin/stats.rs`

### 2. `nix-wire-diff` -- Recording comparator

- Compare two recordings at the protocol level (not raw bytes)
- Show differences in op sequence, timing deltas, result codes
- Useful for: "did this nix upgrade change the protocol conversation?"
- Complexity: medium -- needs op-level alignment algorithm
- File: `src/bin/diff.rs`

### 3. `nix-wire-paths` -- Store path extractor

- Extract all store paths mentioned in a recording
- Group by op type (queried, built, substituted, added)
- Output as plain list, JSON, or derivation graph
- Complexity: medium -- needs to parse op arguments instead of skipping them
- File: `src/bin/paths.rs`

## Tier 2: More specialized

### 4. `nix-wire-fuzz` -- Protocol fuzzer

- Generate malformed wire protocol inputs to test daemon robustness
- Mutation-based: take real recordings, mutate fields
- Generation-based: synthesize random valid/invalid protocol streams
- Complexity: medium-large
- File: `src/bin/fuzz.rs`

### 5. `nix-wire-explain` -- Build conversation explainer

- Narrative output: "Client asked if path X is valid. Daemon said yes. Client then queried path info for X..."
- Useful for understanding what a `nix build` actually does at the protocol level
- Complexity: medium -- needs semantic understanding of op relationships
- File: `src/bin/explain.rs`

### 6. `nix-wire-bench` -- Protocol benchmarker

- Replay a recording N times, measure throughput and latency
- Compare performance across daemon versions
- Warm/cold cache scenarios
- Complexity: small-medium -- extends replay with measurement loop
- File: `src/bin/bench.rs`

## Tier 3: Advanced / future

### 7. `nix-wire-mock` -- Mock Nix daemon

- Serve recorded responses to a real Nix client
- Useful for testing Nix clients without a real store
- Complexity: large -- needs to handle the full daemon side

### 8. `nix-wire-rewrite` -- Recording mutator

- Rewrite store paths, versions, or op arguments in a recording
- Useful for generating test variants from real sessions
- Complexity: medium-large -- needs to parse and re-serialize

### 9. `nix-wire-export` -- Format converter

- Export recordings to PCAP, Wireshark dissector, or other formats
- Complexity: medium

## Recommendation

Start with `nix-wire-stats` -- it has the highest value-to-effort ratio. It
reuses all existing parsing, just adds aggregation and formatting. A single
file addition with no library changes needed.
