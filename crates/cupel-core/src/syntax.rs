// ABOUTME: Payloads of the syntactic facts that language producers emit from the syntax tree.
// ABOUTME: Each payload covers a whole scope; symbol-level entries join on `SymbolKey` (ADR-014).
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::fact::{Fact, FactEntry, FactKind};
use crate::hash::{SiteHash, SymbolHash};
use crate::id::LangId;

/// Implements `Fact` for a payload, at version 1 unless a version is given, and submits its
/// syntactic catalog entry under `name` (ADR-123).
macro_rules! fact {
    ($payload:ident, $kind:ident, $name:literal) => {
        fact!($payload, $kind, $name, 1);
    };
    ($payload:ident, $kind:ident, $name:literal, $version:literal) => {
        impl Fact for $payload {
            const KIND: FactKind = FactKind::$kind;
            const VERSION: u32 = $version;
        }
        inventory::submit!(FactEntry::syntactic::<$payload>($name));
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
fact!(Symbols, Symbols, "symbols", 2);

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
fact!(Decisions, Decisions, "decisions", 2);

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
    /// Where the decision's construct is in the file; absent when the producer does not say
    /// (ADR-133).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-cupel-optional" = true))]
    pub span: Option<Span>,
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
fact!(NormTree, NormTree, "norm-tree");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NormalizedSymbol {
    pub symbol: SymbolKey,
    pub hash: SymbolHash,
    pub root: NormNode,
}

/// A node of a normalized tree. Labels are language-independent so trees in one family compare.
/// Its shape is its label and its children's shapes; the span and text say where it came from
/// and are not part of the shape (ADR-133).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct NormNode {
    pub label: String,
    pub children: Vec<NormNode>,
    /// Where the syntax node this node was normalized from is in the file; absent when the
    /// producer does not say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-cupel-optional" = true))]
    pub span: Option<Span>,
    /// The source text of an identifier or literal that normalized to a marker (`$id`, `$lit`);
    /// absent for every other node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(extend("x-cupel-optional" = true))]
    pub text: Option<String>,
}

/// A node's shape as it serializes: its label and its children's shapes.
#[derive(Serialize)]
struct Shape<'a> {
    label: &'a str,
    children: Vec<Shape<'a>>,
}

impl<'a> Shape<'a> {
    fn of(node: &'a NormNode) -> Self {
        Self {
            label: &node.label,
            children: node.children.iter().map(Shape::of).collect(),
        }
    }
}

impl NormNode {
    /// A node labelled `label` over `children`.
    pub fn new(label: &str, children: Vec<NormNode>) -> Self {
        Self {
            label: label.to_owned(),
            children,
            span: None,
            text: None,
        }
    }

    /// A node labelled `label` without children.
    pub fn leaf(label: &str) -> Self {
        Self::new(label, Vec::new())
    }

    /// This node at `span`.
    #[must_use]
    pub fn at(self, span: Span) -> Self {
        Self {
            span: Some(span),
            ..self
        }
    }

    /// Whether `other` has this node's shape: the same labels with the same children, whatever
    /// their spans and texts.
    pub fn same_shape(&self, other: &NormNode) -> bool {
        self.label == other.label
            && self.children.len() == other.children.len()
            && self
                .children
                .iter()
                .zip(&other.children)
                .all(|(a, b)| a.same_shape(b))
    }

    /// The shape as JSON, `{"label":…,"children":[…]}`: what the structural hash is taken over
    /// (ADR-025), unchanged by spans and texts (ADR-133).
    pub fn shape_json(&self) -> Vec<u8> {
        serde_json::to_vec(&Shape::of(self)).unwrap_or_default()
    }
}

/// Places a mutation operator can apply, per symbol.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MutationSites {
    pub symbols: Vec<SymbolSites>,
}
fact!(MutationSites, MutationSites, "mutation-sites");

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
fact!(TestFrames, TestFrames, "test-frames", 3);

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
    /// The property-based framework that generates the case's inputs, such as `proptest` or
    /// `hypothesis`; `None` for a case with fixed inputs (ADR-095).
    pub property: Option<String>,
    /// The case's doc comment or docstring as written, comment markers and quotes included;
    /// `None` without one (ADR-134).
    pub doc: Option<String>,
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
fact!(ModuleGraph, ModuleGraph, "module-graph", 2);

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
    /// The names the statement imports from its target, as written (not a local alias); empty
    /// when it imports the module whole or the language's import names nothing (ADR-133).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[schemars(extend("x-cupel-optional" = true))]
    pub names: Vec<String>,
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
fact!(ApiSurface, ApiSurface, "api-surface");

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
fact!(Roots, Roots, "roots");

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
fact!(TestConventions, TestConventions, "test-conventions");

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
fact!(ProjectInfo, ProjectInfo, "project-info", 2);

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

/// Where each symbol is named, in every file of the language in the repository (not only the
/// scope, so callers outside a narrowed scope count), and where a name is computed at run time
/// (ADR-135).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct References {
    pub references: Vec<Reference>,
    pub dynamic: Vec<DynamicAccess>,
}
fact!(References, References, "references");

/// One place that names a symbol, or may.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Reference {
    pub file: String,
    pub span: Span,
    /// The name as written; a string's last segment; an attribute's name.
    pub name: String,
    pub kind: ReferenceKind,
    /// The innermost symbol containing the reference; `None` at module level.
    pub from: Option<SymbolKey>,
    pub target: ReferenceTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReferenceKind {
    /// A plain identifier, resolved within its file or through its file's imports.
    Name,
    /// A name reached through a value, type, module, or namespace (`x.f`, `T::f`, `m/f`).
    Member,
    /// A string literal whose text is a name or a path of names.
    Text,
    /// An attribute, decorator, or annotation that hands the definition it is on to whatever
    /// processes it.
    Attribute,
    /// A method the runtime calls by a name the language fixes (a JavaScript `constructor`).
    Implicit,
}

/// What a reference resolves to; it is never guessed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ReferenceTarget {
    /// Definitions in the reference's own file.
    Local { symbols: Vec<SymbolKey> },
    /// Definitions in the file an import of the reference's file binds the name from.
    Imported {
        file: String,
        symbols: Vec<SymbolKey>,
    },
    /// Any symbol of the language with the reference's name.
    Unresolved,
}

impl ReferenceTarget {
    /// The definitions the reference resolves to; empty when unresolved.
    pub fn symbols(&self) -> &[SymbolKey] {
        match self {
            Self::Local { symbols } | Self::Imported { symbols, .. } => symbols,
            Self::Unresolved => &[],
        }
    }
}

/// A place where a name is decided at run time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DynamicAccess {
    pub file: String,
    pub span: Span,
    /// The innermost symbol containing the access; `None` at module level.
    pub from: Option<SymbolKey>,
    pub reach: DynamicReach,
    /// The accessor as written, such as `getattr` or `rates[kind]`, at most 80 characters.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum DynamicReach {
    /// A computed member access: reaches methods.
    Members,
    /// A lookup by a run-time name in a module or namespace, or code evaluated from text:
    /// reaches any symbol.
    Names,
}
