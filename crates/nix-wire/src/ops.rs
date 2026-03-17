//! Nix worker protocol operation codes.
//!
//! From `src/libstore/include/nix/store/worker-protocol.hh`.

/// Worker protocol operation codes.
///
/// Each operation is sent as a u64 on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u64)]
pub enum Op {
    IsValidPath = 1,
    // HasSubstitutes = 3,       // removed
    // QueryPathHash = 4,        // removed
    // QueryReferences = 5,      // removed
    QueryReferrers = 6,
    AddToStore = 7,
    AddTextToStore = 8, // obsolete since 1.25
    BuildPaths = 9,
    EnsurePath = 10,
    AddTempRoot = 11,
    AddIndirectRoot = 12,
    SyncWithGC = 13,
    FindRoots = 14,
    // ExportPath = 16,          // removed
    QueryDeriver = 18, // obsolete
    SetOptions = 19,
    CollectGarbage = 20,
    QuerySubstitutablePathInfo = 21,
    QueryDerivationOutputs = 22, // obsolete
    QueryAllValidPaths = 23,
    // QueryFailedPaths = 24,    // removed
    // ClearFailedPaths = 25,    // removed
    QueryPathInfo = 26,
    // ImportPaths = 27,         // removed
    QueryDerivationOutputNames = 28, // obsolete
    QueryPathFromHashPart = 29,
    QuerySubstitutablePathInfos = 30,
    QueryValidPaths = 31,
    QuerySubstitutablePaths = 32,
    QueryValidDerivers = 33,
    OptimiseStore = 34,
    VerifyStore = 35,
    BuildDerivation = 36,
    AddSignatures = 37,
    NarFromPath = 38,
    AddToStoreNar = 39,
    QueryMissing = 40,
    QueryDerivationOutputMap = 41,
    RegisterDrvOutput = 42,
    QueryRealisation = 43,
    AddMultipleToStore = 44,
    AddBuildLog = 45,
    BuildPathsWithResults = 46,
    AddPermRoot = 47,
}

impl Op {
    /// Try to parse a u64 into a known Op.
    pub fn from_u64(v: u64) -> Option<Self> {
        match v {
            1 => Some(Self::IsValidPath),
            6 => Some(Self::QueryReferrers),
            7 => Some(Self::AddToStore),
            8 => Some(Self::AddTextToStore),
            9 => Some(Self::BuildPaths),
            10 => Some(Self::EnsurePath),
            11 => Some(Self::AddTempRoot),
            12 => Some(Self::AddIndirectRoot),
            13 => Some(Self::SyncWithGC),
            14 => Some(Self::FindRoots),
            18 => Some(Self::QueryDeriver),
            19 => Some(Self::SetOptions),
            20 => Some(Self::CollectGarbage),
            21 => Some(Self::QuerySubstitutablePathInfo),
            22 => Some(Self::QueryDerivationOutputs),
            23 => Some(Self::QueryAllValidPaths),
            26 => Some(Self::QueryPathInfo),
            28 => Some(Self::QueryDerivationOutputNames),
            29 => Some(Self::QueryPathFromHashPart),
            30 => Some(Self::QuerySubstitutablePathInfos),
            31 => Some(Self::QueryValidPaths),
            32 => Some(Self::QuerySubstitutablePaths),
            33 => Some(Self::QueryValidDerivers),
            34 => Some(Self::OptimiseStore),
            35 => Some(Self::VerifyStore),
            36 => Some(Self::BuildDerivation),
            37 => Some(Self::AddSignatures),
            38 => Some(Self::NarFromPath),
            39 => Some(Self::AddToStoreNar),
            40 => Some(Self::QueryMissing),
            41 => Some(Self::QueryDerivationOutputMap),
            42 => Some(Self::RegisterDrvOutput),
            43 => Some(Self::QueryRealisation),
            44 => Some(Self::AddMultipleToStore),
            45 => Some(Self::AddBuildLog),
            46 => Some(Self::BuildPathsWithResults),
            47 => Some(Self::AddPermRoot),
            _ => None,
        }
    }

    /// Human-readable name for the operation.
    pub fn name(self) -> &'static str {
        match self {
            Self::IsValidPath => "IsValidPath",
            Self::QueryReferrers => "QueryReferrers",
            Self::AddToStore => "AddToStore",
            Self::AddTextToStore => "AddTextToStore",
            Self::BuildPaths => "BuildPaths",
            Self::EnsurePath => "EnsurePath",
            Self::AddTempRoot => "AddTempRoot",
            Self::AddIndirectRoot => "AddIndirectRoot",
            Self::SyncWithGC => "SyncWithGC",
            Self::FindRoots => "FindRoots",
            Self::QueryDeriver => "QueryDeriver",
            Self::SetOptions => "SetOptions",
            Self::CollectGarbage => "CollectGarbage",
            Self::QuerySubstitutablePathInfo => "QuerySubstitutablePathInfo",
            Self::QueryDerivationOutputs => "QueryDerivationOutputs",
            Self::QueryAllValidPaths => "QueryAllValidPaths",
            Self::QueryPathInfo => "QueryPathInfo",
            Self::QueryDerivationOutputNames => "QueryDerivationOutputNames",
            Self::QueryPathFromHashPart => "QueryPathFromHashPart",
            Self::QuerySubstitutablePathInfos => "QuerySubstitutablePathInfos",
            Self::QueryValidPaths => "QueryValidPaths",
            Self::QuerySubstitutablePaths => "QuerySubstitutablePaths",
            Self::QueryValidDerivers => "QueryValidDerivers",
            Self::OptimiseStore => "OptimiseStore",
            Self::VerifyStore => "VerifyStore",
            Self::BuildDerivation => "BuildDerivation",
            Self::AddSignatures => "AddSignatures",
            Self::NarFromPath => "NarFromPath",
            Self::AddToStoreNar => "AddToStoreNar",
            Self::QueryMissing => "QueryMissing",
            Self::QueryDerivationOutputMap => "QueryDerivationOutputMap",
            Self::RegisterDrvOutput => "RegisterDrvOutput",
            Self::QueryRealisation => "QueryRealisation",
            Self::AddMultipleToStore => "AddMultipleToStore",
            Self::AddBuildLog => "AddBuildLog",
            Self::BuildPathsWithResults => "BuildPathsWithResults",
            Self::AddPermRoot => "AddPermRoot",
        }
    }

    /// Parse a PascalCase name into an Op.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "IsValidPath" => Some(Self::IsValidPath),
            "QueryReferrers" => Some(Self::QueryReferrers),
            "AddToStore" => Some(Self::AddToStore),
            "AddTextToStore" => Some(Self::AddTextToStore),
            "BuildPaths" => Some(Self::BuildPaths),
            "EnsurePath" => Some(Self::EnsurePath),
            "AddTempRoot" => Some(Self::AddTempRoot),
            "AddIndirectRoot" => Some(Self::AddIndirectRoot),
            "SyncWithGC" => Some(Self::SyncWithGC),
            "FindRoots" => Some(Self::FindRoots),
            "QueryDeriver" => Some(Self::QueryDeriver),
            "SetOptions" => Some(Self::SetOptions),
            "CollectGarbage" => Some(Self::CollectGarbage),
            "QuerySubstitutablePathInfo" => Some(Self::QuerySubstitutablePathInfo),
            "QueryDerivationOutputs" => Some(Self::QueryDerivationOutputs),
            "QueryAllValidPaths" => Some(Self::QueryAllValidPaths),
            "QueryPathInfo" => Some(Self::QueryPathInfo),
            "QueryDerivationOutputNames" => Some(Self::QueryDerivationOutputNames),
            "QueryPathFromHashPart" => Some(Self::QueryPathFromHashPart),
            "QuerySubstitutablePathInfos" => Some(Self::QuerySubstitutablePathInfos),
            "QueryValidPaths" => Some(Self::QueryValidPaths),
            "QuerySubstitutablePaths" => Some(Self::QuerySubstitutablePaths),
            "QueryValidDerivers" => Some(Self::QueryValidDerivers),
            "OptimiseStore" => Some(Self::OptimiseStore),
            "VerifyStore" => Some(Self::VerifyStore),
            "BuildDerivation" => Some(Self::BuildDerivation),
            "AddSignatures" => Some(Self::AddSignatures),
            "NarFromPath" => Some(Self::NarFromPath),
            "AddToStoreNar" => Some(Self::AddToStoreNar),
            "QueryMissing" => Some(Self::QueryMissing),
            "QueryDerivationOutputMap" => Some(Self::QueryDerivationOutputMap),
            "RegisterDrvOutput" => Some(Self::RegisterDrvOutput),
            "QueryRealisation" => Some(Self::QueryRealisation),
            "AddMultipleToStore" => Some(Self::AddMultipleToStore),
            "AddBuildLog" => Some(Self::AddBuildLog),
            "BuildPathsWithResults" => Some(Self::BuildPathsWithResults),
            "AddPermRoot" => Some(Self::AddPermRoot),
            _ => None,
        }
    }
}

impl std::fmt::Display for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_ops_roundtrip() {
        let ops = [
            Op::IsValidPath,
            Op::QueryReferrers,
            Op::AddToStore,
            Op::AddTextToStore,
            Op::BuildPaths,
            Op::EnsurePath,
            Op::AddTempRoot,
            Op::AddIndirectRoot,
            Op::SyncWithGC,
            Op::FindRoots,
            Op::QueryDeriver,
            Op::SetOptions,
            Op::CollectGarbage,
            Op::QuerySubstitutablePathInfo,
            Op::QueryDerivationOutputs,
            Op::QueryAllValidPaths,
            Op::QueryPathInfo,
            Op::QueryDerivationOutputNames,
            Op::QueryPathFromHashPart,
            Op::QuerySubstitutablePathInfos,
            Op::QueryValidPaths,
            Op::QuerySubstitutablePaths,
            Op::QueryValidDerivers,
            Op::OptimiseStore,
            Op::VerifyStore,
            Op::BuildDerivation,
            Op::AddSignatures,
            Op::NarFromPath,
            Op::AddToStoreNar,
            Op::QueryMissing,
            Op::QueryDerivationOutputMap,
            Op::RegisterDrvOutput,
            Op::QueryRealisation,
            Op::AddMultipleToStore,
            Op::AddBuildLog,
            Op::BuildPathsWithResults,
            Op::AddPermRoot,
        ];
        for op in ops {
            let val = op as u64;
            assert_eq!(Op::from_u64(val), Some(op), "roundtrip failed for {:?}", op);
        }
    }

    #[test]
    fn unknown_op() {
        assert_eq!(Op::from_u64(0), None);
        assert_eq!(Op::from_u64(2), None);
        assert_eq!(Op::from_u64(999), None);
    }

    #[test]
    fn from_name_roundtrip() {
        let ops = [
            Op::IsValidPath,
            Op::QueryReferrers,
            Op::AddToStore,
            Op::SetOptions,
            Op::BuildPaths,
            Op::QueryPathInfo,
            Op::AddPermRoot,
        ];
        for op in ops {
            let name = op.name();
            assert_eq!(
                Op::from_name(name),
                Some(op),
                "from_name failed for {}",
                name
            );
        }
    }

    #[test]
    fn from_name_unknown() {
        assert_eq!(Op::from_name("NotAnOp"), None);
        assert_eq!(Op::from_name(""), None);
    }

    #[test]
    fn op_count() {
        // Count all defined operation codes (active + obsolete, excluding removed)
        let mut count = 0;
        for i in 0..=100 {
            if Op::from_u64(i).is_some() {
                count += 1;
            }
        }
        assert_eq!(count, 37);
    }
}
