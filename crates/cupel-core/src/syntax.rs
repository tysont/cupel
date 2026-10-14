// ABOUTME: Payloads of the syntactic facts that language producers emit from the syntax tree.
// ABOUTME: Each payload covers a whole scope; symbol-level entries join on `SymbolKey` (ADR-014).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactKind};
use crate::hash::{SiteHash, SymbolHash};
use crate::id::LangId;

/// Implements `Fact` for a payload, at version 1 unless a version is given.
macro_rules! fact {
    ($payload:ident, $kind:ident) => {
        fact!($payload, $kind, 1);
    };
    ($payload:ident, $kind:ident, $version:literal) => {
        impl Fact for $payload {
            const KIND: FactKind = FactKind::$kind;
            const VERSION: u32 = $version;
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
fact!(Symbols, Symbols, 2);

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
    /// Declared parameters, not counting a receiver (`self`, `cls`, `this`, a Go receiver).
    pub parameters: u32,
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
fact!(Decisions, Decisions, 2);

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
    /// A loop over a table of cases written in place: a literal array, vector, slice, list, or
    /// tuple, iterated directly or through a local bound to one in the same body (ADR-069).
    /// Always false for other kinds.
    pub table: bool,
    /// A `case` arm after the first of its match or switch: the same multiway decision, which
    /// modified cyclomatic complexity counts once (ADR-072). Always false for other kinds.
    pub continues: bool,
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

impl NormNode {
    /// A node labelled `label` over `children`.
    pub fn new(label: &str, children: Vec<NormNode>) -> Self {
        Self {
            label: label.to_owned(),
            children,
        }
    }

    /// A node labelled `label` without children.
    pub fn leaf(label: &str) -> Self {
        Self::new(label, Vec::new())
    }
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
fact!(ModuleGraph, ModuleGraph, 2);

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
    /// What the statement does with the module (ADR-068).
    pub edge_kind: EdgeKind,
}

/// What an import statement does with the module it names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    /// Uses the module (`use`, `import`, `require`).
    Import,
    /// Declares the module as a child of the importing one (Rust `mod a;`): structure, not use.
    Declaration,
    /// Imports names in order to export them again (Rust `pub use`, `export … from`, a Python
    /// import of names listed in `__all__`).
    ReExport,
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

impl TestConventions {
    /// Whether `path` is a test file: some glob in `test_globs` matches it. A glob that does not
    /// parse matches nothing.
    pub fn is_test(&self, path: &str) -> bool {
        self.test_globs.iter().any(|glob| {
            globset::GlobBuilder::new(glob)
                .literal_separator(true)
                .build()
                .is_ok_and(|glob| glob.compile_matcher().is_match(path))
        })
    }

    /// The test files that test `source`, empty when it has none.
    pub fn test_files_for(&self, source: &str) -> &[String] {
        self.pairs
            .iter()
            .find(|pair| pair.source == source)
            .map_or(&[], |pair| pair.tests.as_slice())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TestPairing {
    pub source: String,
    pub tests: Vec<String>,
}

/// The project roots detected in the repository: what each language's ecosystem files say,
/// not which tools to run. `cupel init` and `doctor` join a root with the toolchain table to
/// choose commands (ADR-037).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectInfo {
    pub roots: Vec<ProjectRoot>,
}
fact!(ProjectInfo, ProjectInfo, 2);

/// A directory with a build manifest of one language's ecosystem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ProjectRoot {
    /// Relative to the repository root, `/`-separated; `.` for the repository root itself.
    pub path: String,
    pub lang: LangId,
    /// Manifests of the root, then of its members, relative to the root.
    pub build_files: Vec<String>,
    /// Workspace member directories below the root, relative to it.
    pub members: Vec<String>,
    /// Package patterns the root's commands cover (`./...`); empty when commands take none.
    pub packages: Vec<String>,
    /// Directory names the ecosystem generates, skipped below the root.
    pub skip_dirs: Vec<String>,
}
