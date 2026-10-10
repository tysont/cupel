// ABOUTME: Payloads of the syntactic facts that language producers emit from the syntax tree.
// ABOUTME: Each payload covers a whole scope; symbol-level entries join on `SymbolKey` (ADR-014).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::hash::{SiteHash, SymbolHash};
use crate::id::LangId;

/// Implements `Fact` for a payload at version 1.
macro_rules! fact {
    ($payload:ident, $kind:ident) => {
        impl Fact for $payload {
            const KIND: FactKind = FactKind::$kind;
            const VERSION: u32 = 1;
        }
    };
}

/// A 1-based range within one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Span {
    pub line: u32,
    pub col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Identifies a symbol within one run by where it starts. Not stable across edits; anything
/// persisted across runs uses `SymbolHash` (ADR-014).
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
pub struct SymbolKey {
    pub file: String,
    pub line: u32,
    pub col: u32,
}

/// Functions and methods in scope.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Symbols {
    pub symbols: Vec<SymbolDef>,
}
fact!(Symbols, Symbols);

/// One function, method, or named closure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SymbolDef {
    pub name: String,
    /// Enclosing namespace, such as `module::Type`; empty at top level.
    pub namespace: String,
    /// `namespace::name`.
    pub display: String,
    pub kind: SymbolKind,
    pub visibility: Visibility,
    pub lang: LangId,
    pub file: String,
    pub span: Span,
    /// The body, when the symbol has one separate from its signature.
    pub body: Option<Span>,
}

impl SymbolDef {
    pub fn key(&self) -> SymbolKey {
        SymbolKey {
            file: self.file.clone(),
            line: self.span.line,
            col: self.span.col,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum SymbolKind {
    Function,
    Method,
    Closure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Visibility {
    Public,
    /// Visible within a package, crate, or module tree but not exported.
    Internal,
    Private,
}

/// Decision points per symbol, with the weights the language's tables assign.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Decisions {
    pub symbols: Vec<SymbolDecisions>,
}
fact!(Decisions, Decisions);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SymbolDecisions {
    pub symbol: SymbolKey,
    pub decisions: Vec<Decision>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Decision {
    pub kind: DecisionKind,
    /// Depth of enclosing decisions, 0 at the top of the body.
    pub nesting: u32,
    /// Weight toward cyclomatic complexity.
    pub cyclomatic: u32,
    /// Base weight toward cognitive complexity, before nesting.
    pub cognitive: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionKind {
    Branch,
    Loop,
    BooleanOp,
    Case,
    Catch,
    EarlyReturn,
    Ternary,
}

/// Normalized syntax trees per symbol, and the structural hash each yields.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NormTree {
    pub symbols: Vec<NormalizedSymbol>,
}
fact!(NormTree, NormTree);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NormalizedSymbol {
    pub symbol: SymbolKey,
    pub hash: SymbolHash,
    pub root: NormNode,
}

/// A node of a normalized tree. Labels are language-independent so trees in one family compare.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NormNode {
    pub label: String,
    pub children: Vec<NormNode>,
}

/// Places a mutation operator can apply, per symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MutationSites {
    pub symbols: Vec<SymbolSites>,
}
fact!(MutationSites, MutationSites);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SymbolSites {
    pub symbol: SymbolKey,
    pub sites: Vec<Site>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Site {
    pub operator: MutationOperator,
    pub set: OperatorSet,
    /// The source text being replaced.
    pub from: String,
    /// Each replacement the operator produces.
    pub to: Vec<String>,
    /// Structural position within the symbol.
    pub site_hash: SiteHash,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MutationOperator {
    Relational,
    Equality,
    ArithAddSub,
    ArithMulDiv,
    BooleanOp,
    BooleanLiteral,
    ZeroOne,
    StatementDeletion,
    ReturnValue,
    Constant,
    Boundary,
    Negation,
}

/// Operator sets: `default` keeps the equivalent-mutant rate low; `extended` is opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum OperatorSet {
    Default,
    Extended,
}

/// Test cases and what they assert, mock, skip, and set up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestFrames {
    pub files: Vec<TestFile>,
}
fact!(TestFrames, TestFrames);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestFile {
    pub file: String,
    /// The test framework, such as `pytest` or `vitest`.
    pub framework: String,
    /// Setup and fixture blocks shared by the file's cases.
    pub setup: Vec<Span>,
    pub cases: Vec<TestCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestCase {
    pub name: String,
    pub span: Span,
    pub assertions: Vec<Assertion>,
    pub mocks: Vec<Mock>,
    pub skip: Option<SkipMarker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Assertion {
    pub kind: AssertionKind,
    pub strength: AssertionStrength,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AssertionKind {
    Equality,
    Truthiness,
    NotNull,
    Throws,
    Snapshot,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum AssertionStrength {
    Strong,
    Weak,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Mock {
    /// What is replaced, as written.
    pub target: String,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SkipMarker {
    pub reason: Option<String>,
    pub span: Span,
}

/// Resolved import edges per project root.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ModuleGraph {
    pub roots: Vec<RootImports>,
}
fact!(ModuleGraph, ModuleGraph);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RootImports {
    pub root: String,
    pub edges: Vec<ImportEdge>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ImportEdge {
    /// The importing file.
    pub from: String,
    /// The import as written.
    pub specifier: String,
    pub target: ImportTarget,
    /// True for path-relative imports such as `./b`.
    pub relative: bool,
    pub line: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ImportTarget {
    File { path: String },
    External { package: String },
    Unresolved,
}

/// Public items and their normalized signatures.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApiSurface {
    pub items: Vec<ApiItem>,
}
fact!(ApiSurface, ApiSurface);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApiItem {
    /// Fully qualified path of the item.
    pub path: String,
    pub kind: ApiItemKind,
    /// Signature with local names and formatting normalized away.
    pub signature: String,
    pub file: String,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ApiItemKind {
    Function,
    Method,
    Type,
    /// Traits, interfaces, and protocols.
    Interface,
    Constant,
    Variable,
    Module,
}

/// Symbols that are live by convention even with no caller in scope.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Roots {
    pub roots: Vec<LiveRoot>,
}
fact!(Roots, Roots);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LiveRoot {
    pub symbol: SymbolKey,
    pub reason: RootReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RootReason {
    EntryPoint,
    Export,
    Test,
    FrameworkHook,
}

/// How test files are recognized and which tests belong to which source file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestConventions {
    /// Globs that match test files; a file is a test exactly when one matches.
    pub test_globs: Vec<String>,
    pub pairs: Vec<TestPairing>,
}
fact!(TestConventions, TestConventions);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestPairing {
    pub source: String,
    pub tests: Vec<String>,
}

/// Project roots, their build files, default commands, and directories to skip.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectInfo {
    pub roots: Vec<ProjectRoot>,
}
fact!(ProjectInfo, ProjectInfo);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectRoot {
    pub path: String,
    pub lang: LangId,
    pub build_files: Vec<String>,
    pub commands: DefaultCommands,
    pub skip_dirs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DefaultCommands {
    pub test: Option<String>,
    pub coverage: Option<String>,
    pub bench: Option<String>,
}
