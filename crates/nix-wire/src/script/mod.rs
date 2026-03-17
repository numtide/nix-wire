//! Human-readable `.nwscript` format for Nix daemon protocol sessions.
//!
//! Provides AST types for representing protocol sessions as text,
//! plus modules for decompiling, formatting, parsing, serializing,
//! and evaluating expect assertions.

pub mod decompile;
pub mod expect;
pub mod format;
pub mod parse;
pub mod serialize;

use crate::handshake::ProtocolVersion;
use crate::ops::Op;
use crate::stderr::StderrCode;

/// A complete script representing a Nix daemon protocol session.
#[derive(Debug, Clone)]
pub struct Script {
    pub preamble: Preamble,
    pub entries: Vec<Entry>,
}

/// Header metadata for the script.
#[derive(Debug, Clone)]
pub struct Preamble {
    /// Protocol version (e.g., 1.38).
    pub protocol_version: ProtocolVersion,
    /// Client features to advertise during handshake (e.g., "ca-derivations").
    pub client_features: Vec<String>,
    /// Handshake-level expect assertions (for `run`).
    pub expects: Vec<Expect>,
    /// Daemon nix version string (e.g., "2.33.3") -- informational comment.
    pub daemon_version: Option<String>,
    /// Trust status: "trusted", "not-trusted", or "unknown" -- informational comment.
    pub trust: Option<String>,
    /// Server features received during handshake -- informational comment.
    pub server_features: Option<Vec<String>>,
}

/// A single operation entry in the script.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Timestamp in milliseconds from session start (optional).
    pub timestamp_ms: Option<f64>,
    /// The operation call.
    pub op_call: OpCall,
    /// Daemon response (from `show`, used to generate comments/expects).
    pub response: Option<DaemonResponse>,
    /// User-defined expect assertions (for `run`).
    pub expects: Vec<Expect>,
}

/// An operation call with its arguments.
#[derive(Debug, Clone)]
pub enum OpCall {
    /// Single path argument (IsValidPath, QueryPathInfo, etc.).
    SinglePath {
        op: Op,
        path: String,
    },

    /// Single string argument (AddIndirectRoot, QueryPathFromHashPart, etc.).
    SingleString {
        op: Op,
        value: String,
    },

    /// Path set argument (QuerySubstitutablePaths, QueryMissing, etc.).
    PathSet {
        op: Op,
        paths: Vec<String>,
    },

    /// Path set + substitute flag (QueryValidPaths).
    PathSetFlag {
        op: Op,
        paths: Vec<String>,
        substitute: bool,
    },

    /// Path set + build mode (BuildPaths, BuildPathsWithResults).
    PathSetMode {
        op: Op,
        paths: Vec<String>,
        mode: String,
    },

    /// No arguments (SyncWithGC, FindRoots, QueryAllValidPaths, OptimiseStore).
    NoArgs {
        op: Op,
    },

    /// SetOptions with keyword arguments.
    SetOptions {
        keep_failed: u64,
        keep_going: u64,
        try_fallback: u64,
        verbosity: u64,
        max_build_jobs: u64,
        max_silent_time: u64,
        use_build_hook: u64,
        verbose_build: u64,
        log_type: u64,
        print_build_trace: u64,
        build_cores: u64,
        use_substitutes: u64,
        overrides: Vec<(String, String)>,
    },

    /// CollectGarbage with keyword arguments.
    CollectGarbage {
        action: u64,
        paths: Vec<String>,
        ignore_liveness: u64,
        max_freed: u64,
    },

    /// VerifyStore with two bools.
    VerifyStore {
        check_contents: bool,
        repair: bool,
    },

    /// AddPermRoot: path + gcRoot.
    AddPermRoot {
        path: String,
        gc_root: String,
    },

    /// AddSignatures: path + signatures.
    AddSignatures {
        path: String,
        sigs: Vec<String>,
    },

    /// AddTextToStore: suffix + text + refs.
    AddTextToStore {
        suffix: String,
        text: String,
        refs: Vec<String>,
    },

    /// AddToStore (>= 1.25): name + cam_str + refs + repair + framed NAR data.
    AddToStore {
        name: String,
        cam_str: String,
        refs: Vec<String>,
        repair: bool,
        data: FramedData,
    },

    /// AddMultipleToStore: repair + dont_check_sigs + framed data.
    AddMultipleToStore {
        repair: bool,
        dont_check_sigs: bool,
        data: FramedData,
    },

    /// AddBuildLog: path + framed log data.
    AddBuildLog {
        path: String,
        data: FramedData,
    },

    /// RegisterDrvOutput / QueryRealisation: single string.
    RegisterDrvOutput {
        value: String,
    },
    QueryRealisation {
        value: String,
    },

    /// QuerySubstitutablePathInfo: single path.
    QuerySubstitutablePathInfo {
        path: String,
    },

    /// QuerySubstitutablePathInfos: path set.
    QuerySubstitutablePathInfos {
        paths: Vec<String>,
    },

    /// Raw bytes fallback for ops too complex to parse (BuildDerivation, AddToStoreNar).
    RawBytes {
        op: Op,
        data: Vec<u8>,
    },
}

/// Framed data representation.
#[derive(Debug, Clone)]
pub enum FramedData {
    /// Inline bytes.
    Inline(Vec<u8>),
    /// Reference to an external file.
    FileRef(String),
}

/// Daemon response captured during decompile.
#[derive(Debug, Clone)]
pub struct DaemonResponse {
    /// Terminal stderr code ("Last" or "Error").
    pub terminal: String,
    /// Stderr message count.
    pub stderr_count: u64,
    /// Parsed result data (after Last).
    pub result: Option<ResultData>,
    /// Error info (after Error).
    pub error: Option<ErrorInfo>,
}

/// Error information from STDERR_ERROR.
#[derive(Debug, Clone)]
pub struct ErrorInfo {
    pub error_type: String,
    pub level: u64,
    pub name: String,
    pub message: String,
}

/// Parsed result data from the daemon response.
#[derive(Debug, Clone)]
pub enum ResultData {
    /// Single u64 value.
    U64(u64),
    /// Single string value.
    Str(String),
    /// PathInfo result.
    PathInfo(PathInfoResult),
    /// StringSet result.
    StringSet(Vec<String>),
    /// Map of string -> string.
    StringMap(Vec<(String, String)>),
    /// No result data.
    None,
    /// CollectGarbage result.
    CollectGarbage { bytes_freed: u64 },
    /// QuerySubstitutablePathInfo result.
    SubstitutablePathInfo {
        valid: bool,
        deriver: Option<String>,
        refs: Option<Vec<String>>,
        download_size: Option<u64>,
        nar_size: Option<u64>,
    },
    /// QueryMissing result.
    Missing {
        will_build: Vec<String>,
        will_substitute: Vec<String>,
        unknown: Vec<String>,
        download_size: u64,
        nar_size: u64,
    },
    /// Framed data result (NarFromPath).
    Framed(Vec<u8>),
    /// Raw bytes we couldn't parse.
    Raw(Vec<u8>),
}

/// QueryPathInfo result.
#[derive(Debug, Clone)]
pub struct PathInfoResult {
    pub valid: bool,
    pub deriver: Option<String>,
    pub nar_hash: Option<String>,
    pub references: Option<Vec<String>>,
    pub registration_time: Option<u64>,
    pub nar_size: Option<u64>,
    pub ultimate: Option<bool>,
    pub sigs: Option<Vec<String>>,
    pub ca: Option<String>,
}

/// An expect assertion for the `run` command.
#[derive(Debug, Clone)]
pub enum Expect {
    /// Expect a specific terminal code: `expect: Last` or `expect: Error`.
    Terminal(String),
    /// Expect a result field value: `expect result: 1` or `expect result.valid: 1`.
    Result {
        field: Option<String>,
        matcher: Matcher,
    },
    /// Expect error message: `expect error: /pattern/`.
    Error { matcher: Matcher },
    /// Expect stderr count: `expect stderr.count: 0`.
    StderrCount { matcher: Matcher },
    /// Expect daemon version (handshake): `expect daemon: /^2\./`.
    Daemon { matcher: Matcher },
    /// Expect trust status (handshake): `expect trust: trusted`.
    Trust { matcher: Matcher },
    /// Expect server features (handshake): `expect server_features: /ca-derivations/`.
    ServerFeatures { matcher: Matcher },
}

/// A matcher for expect assertions.
#[derive(Debug, Clone)]
pub enum Matcher {
    /// Exact string match.
    Exact(String),
    /// Regex pattern match (delimited by //).
    Regex(String),
    /// Numeric comparison.
    GreaterThan(i64),
    LessThan(i64),
    GreaterOrEqual(i64),
    LessOrEqual(i64),
}

impl std::fmt::Display for Matcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact(s) => write!(f, "{s}"),
            Self::Regex(r) => write!(f, "/{r}/"),
            Self::GreaterThan(n) => write!(f, "> {n}"),
            Self::LessThan(n) => write!(f, "< {n}"),
            Self::GreaterOrEqual(n) => write!(f, ">= {n}"),
            Self::LessOrEqual(n) => write!(f, "<= {n}"),
        }
    }
}

/// Format a terminal stderr code as a string for script output.
pub fn terminal_name(code: Option<StderrCode>) -> String {
    match code {
        Some(StderrCode::Last) => "Last".to_string(),
        Some(StderrCode::Error) => "Error".to_string(),
        Some(c) => c.name().to_string(),
        None => "none".to_string(),
    }
}

impl OpCall {
    /// Get the Op for this call.
    pub fn op(&self) -> Op {
        match self {
            Self::SinglePath { op, .. } => *op,
            Self::SingleString { op, .. } => *op,
            Self::PathSet { op, .. } => *op,
            Self::PathSetFlag { op, .. } => *op,
            Self::PathSetMode { op, .. } => *op,
            Self::NoArgs { op } => *op,
            Self::SetOptions { .. } => Op::SetOptions,
            Self::CollectGarbage { .. } => Op::CollectGarbage,
            Self::VerifyStore { .. } => Op::VerifyStore,
            Self::AddPermRoot { .. } => Op::AddPermRoot,
            Self::AddSignatures { .. } => Op::AddSignatures,
            Self::AddTextToStore { .. } => Op::AddTextToStore,
            Self::AddToStore { .. } => Op::AddToStore,
            Self::AddMultipleToStore { .. } => Op::AddMultipleToStore,
            Self::AddBuildLog { .. } => Op::AddBuildLog,
            Self::RegisterDrvOutput { .. } => Op::RegisterDrvOutput,
            Self::QueryRealisation { .. } => Op::QueryRealisation,
            Self::QuerySubstitutablePathInfo { .. } => Op::QuerySubstitutablePathInfo,
            Self::QuerySubstitutablePathInfos { .. } => Op::QuerySubstitutablePathInfos,
            Self::RawBytes { op, .. } => *op,
        }
    }
}
