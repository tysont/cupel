// ABOUTME: The typed form of a language's `tables.toml`: the data the generic producers read
// ABOUTME: alongside the queries. Unknown keys are errors, so a typo fails the spec test.
use std::collections::{BTreeMap, HashMap};

use cupel_core::syntax::{
    AssertionKind, AssertionStrength, DecisionKind, MutationOperator, Visibility,
};
use serde::Deserialize;

/// Everything a language's `tables.toml` may contain. Every section is optional; a section a
/// language omits takes the defaults documented on its type.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Tables {
    pub namespace: NamespaceRules,
    pub visibility: VisibilityRules,
    pub decisions: DecisionRules,
    pub syntax: SyntaxRules,
    /// Present when the language has a `NormTree` producer.
    pub normalize: Option<NormalizeRules>,
    pub mutations: HashMap<MutationOperator, Replacements>,
    pub tests: TestRules,
    /// Present when the language has a `TestConventions` producer.
    pub conventions: Option<ConventionRules>,
    /// Present when the language has a `Roots` producer.
    pub roots: Option<RootRules>,
    pub api: ApiRules,
    pub references: ReferenceRules,
    /// Present when `check --scaffold` writes property stubs for the language (ADR-096 §4).
    pub property_stubs: Option<crate::property_stubs::StubRules>,
}

impl Tables {
    pub fn parse(text: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(text)
    }
}

/// `[namespace]`: how a file's path becomes the namespace of the symbols in it, unless the
/// file captures `@namespace.name` (Go's `package` clause).
///
/// ```toml
/// [namespace]
/// separator = "::"                    # joins namespace segments; default "."
/// source_roots = ["src"]              # path up to and including the last such directory is dropped
/// index_files = ["lib", "main", "mod"] # a file stem equal to one of these adds no segment
/// ```
///
/// `src/report/format.rs` is `report::format`; `src/lib.rs` is the empty namespace;
/// `tests/ledger.rs`, outside any source root, is `tests::ledger`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct NamespaceRules {
    pub separator: String,
    pub source_roots: Vec<String>,
    pub index_files: Vec<String>,
}

impl Default for NamespaceRules {
    fn default() -> Self {
        Self {
            separator: ".".to_owned(),
            source_roots: Vec::new(),
            index_files: Vec::new(),
        }
    }
}

/// `[visibility]`: a symbol's visibility when its match has no `@visibility.<v>` capture.
/// The `@symbol.visibility` node's text (whitespace removed) is looked up in `modifiers`;
/// an unlisted modifier gets `modifier_default` (or `default`); a symbol without a modifier
/// takes the first matching `[[visibility.names]]` rule, else `default`.
///
/// ```toml
/// [visibility]
/// default = "private"                           # default "private"
/// modifiers = { "pub" = "public", "pub(self)" = "private" }
/// modifier_default = "internal"                 # pub(crate), pub(super), pub(in …)
///
/// [[visibility.names]]                          # Go: exported names are capitalized
/// capitalized = true
/// visibility = "public"
///
/// [[visibility.names]]                          # Python: `_name` is private by convention
/// prefix = "_"
/// visibility = "private"
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct VisibilityRules {
    pub default: Visibility,
    pub modifiers: BTreeMap<String, Visibility>,
    pub modifier_default: Option<Visibility>,
    pub names: Vec<NameRule>,
}

impl Default for VisibilityRules {
    fn default() -> Self {
        Self {
            default: Visibility::Private,
            modifiers: BTreeMap::new(),
            modifier_default: None,
            names: Vec::new(),
        }
    }
}

/// Matches a name when every given condition holds.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameRule {
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub capitalized: Option<bool>,
    pub visibility: Visibility,
}

impl NameRule {
    pub fn matches(&self, name: &str) -> bool {
        self.prefix
            .as_ref()
            .is_none_or(|p| name.starts_with(p.as_str()))
            && self
                .suffix
                .as_ref()
                .is_none_or(|s| name.ends_with(s.as_str()))
            && self
                .capitalized
                .is_none_or(|c| name.chars().next().is_some_and(char::is_uppercase) == c)
    }
}

impl VisibilityRules {
    /// The visibility of a symbol called `name` whose modifier text (if any) is `modifier`.
    pub fn resolve(&self, name: &str, modifier: Option<&str>) -> Visibility {
        if let Some(modifier) = modifier {
            let compact: String = modifier.split_whitespace().collect();
            return self
                .modifiers
                .get(&compact)
                .copied()
                .or(self.modifier_default)
                .unwrap_or(self.default);
        }
        self.names
            .iter()
            .find(|rule| rule.matches(name))
            .map_or(self.default, |rule| rule.visibility)
    }
}

/// `[syntax]`: which syntax errors the grammar reports that the language does not have.
///
/// ```toml
/// [syntax]
/// token_trees = ["token_tree"]  # node kinds whose children may be any token
/// ```
///
/// An `ERROR` node directly inside a `token_trees` node that holds only tokens is a token the
/// grammar does not know there (Rust's `~` in a macro call, which rustc lexes and leaves to the
/// macro), so it is not a `lang.parse-error`. Default: none.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SyntaxRules {
    pub token_trees: Vec<String>,
}

/// `[decisions]`: how decisions captured by decisions.scm weigh and nest.
///
/// ```toml
/// [decisions]
/// chain_parents = ["else_clause"]   # a branch whose parent is one of these continues a chain
/// condition_fields = ["condition"]  # decisions in these fields of a decision do not nest in it
/// functions = ["arrow_function"]    # anonymous functions a final `return` of theirs only ends
///
/// [decisions.weights.early-return]  # overrides one kind's default weight
/// cyclomatic = 0
/// cognitive = 1
/// nests = false
/// ```
///
/// Default weights (`cyclomatic`, `cognitive`, `nests`): `branch`, `loop`, `case`, `catch`,
/// and `ternary` are (1, 1, yes); `boolean-op` is (1, 1, no); `early-return` is (0, 1, no).
/// Only the first `case` among siblings keeps its cognitive weight, so a switch counts once.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct DecisionRules {
    pub chain_parents: Vec<String>,
    pub condition_fields: Vec<String>,
    /// Node kinds of anonymous functions whose decisions stay with the enclosing symbol: a
    /// `return` that is the last statement of one only ends it, so it is not an early return.
    pub functions: Vec<String>,
    pub weights: HashMap<DecisionKind, Weight>,
}

impl Default for DecisionRules {
    fn default() -> Self {
        Self {
            chain_parents: Vec::new(),
            condition_fields: vec!["condition".to_owned()],
            functions: Vec::new(),
            weights: HashMap::new(),
        }
    }
}

impl DecisionRules {
    /// The weight of `kind`: the table's, else the default.
    pub fn weight(&self, kind: DecisionKind) -> Weight {
        self.weights
            .get(&kind)
            .copied()
            .unwrap_or_else(|| Weight::default_for(kind))
    }
}

/// How much one decision adds to cyclomatic and cognitive complexity, and whether decisions
/// inside it are nested one level deeper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Weight {
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub nests: bool,
}

impl Weight {
    pub fn default_for(kind: DecisionKind) -> Self {
        let (cyclomatic, cognitive, nests) = match kind {
            DecisionKind::Branch
            | DecisionKind::Loop
            | DecisionKind::Case
            | DecisionKind::Catch
            | DecisionKind::Ternary => (1, 1, true),
            DecisionKind::BooleanOp => (1, 1, false),
            DecisionKind::EarlyReturn => (0, 1, false),
        };
        Self {
            cyclomatic,
            cognitive,
            nests,
        }
    }
}

/// `[normalize]`: how `TableNormalize` turns a symbol's syntax tree into a `NormTree`
/// (ADR-025). Comments are always dropped, as is the symbol's own name.
///
/// ```toml
/// [normalize]
/// identifiers = ["identifier", "field_identifier"]  # named kinds that become `$id`
/// literals = ["integer_literal", "string_literal"]  # named kinds that become `$lit`
/// erase = ["attribute_item"]                        # kinds dropped with their subtrees
/// flatten = []                                      # kinds replaced by their children
/// operators = []                                    # word tokens kept as operators (`and`)
/// receivers = ["value"]                             # callee fields that hold a receiver
///
/// [[normalize.calls]]                               # identifiers in `callee` keep their text
/// kind = "call_expression"
/// callee = "function"
/// ```
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct NormalizeRules {
    pub identifiers: Vec<String>,
    pub literals: Vec<String>,
    pub erase: Vec<String>,
    pub flatten: Vec<String>,
    pub operators: Vec<String>,
    pub receivers: Vec<String>,
    pub calls: Vec<CallRule>,
}

/// A node kind whose `callee` field names the function it calls.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CallRule {
    pub kind: String,
    pub callee: String,
}

/// `[mutations.<operator>]`: the replacements a site captured as `@mut.<operator>` produces.
/// `replace` maps the site's exact text to its replacements; each of `templates` is a
/// replacement with `{}` standing for the site's text. Replacements equal to the original are
/// dropped, and a site with none left is not reported.
///
/// ```toml
/// [mutations.relational]
/// replace = { "<" = ["<="], "<=" = ["<"], ">" = [">="], ">=" = [">"] }
///
/// [mutations.negation]
/// templates = ["!({})"]
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Replacements {
    pub replace: BTreeMap<String, Vec<String>>,
    pub templates: Vec<String>,
}

impl Replacements {
    /// Every distinct replacement for `from`, in table order, without `from` itself.
    pub fn of(&self, from: &str) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let candidates = self
            .replace
            .get(from)
            .into_iter()
            .flatten()
            .cloned()
            .chain(self.templates.iter().map(|t| t.replace("{}", from)));
        for candidate in candidates {
            if candidate != from && !out.contains(&candidate) {
                out.push(candidate);
            }
        }
        out
    }
}

/// `[tests]`: what `QueryTestFrames` needs besides tests.scm.
///
/// ```toml
/// [tests]
/// framework = "rust-test"     # a file's framework when no `@framework.<name>` capture matched
/// unknown_assertion = { kind = "custom", strength = "strong" }  # the default shown
///
/// [tests.assertions]          # assertion key → semantic kind and strength
/// assert_eq = { kind = "equality", strength = "strong" }
/// is-present = { kind = "not-null", strength = "weak" }   # an `@assertion.idiom.is-present`
/// ```
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TestRules {
    pub framework: String,
    pub unknown_assertion: AssertionClass,
    pub assertions: BTreeMap<String, AssertionClass>,
}

impl Default for TestRules {
    fn default() -> Self {
        Self {
            framework: String::new(),
            unknown_assertion: AssertionClass {
                kind: AssertionKind::Custom,
                strength: AssertionStrength::Strong,
            },
            assertions: BTreeMap::new(),
        }
    }
}

impl TestRules {
    /// The class of the assertion whose key is `key`.
    pub fn classify(&self, key: &str) -> AssertionClass {
        self.assertions
            .get(key)
            .copied()
            .unwrap_or(self.unknown_assertion)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssertionClass {
    pub kind: AssertionKind,
    pub strength: AssertionStrength,
}

/// `[conventions]`: how `GlobTestConventions` recognizes test files and pairs them with sources.
///
/// ```toml
/// [conventions]
/// test_globs = ["**/tests/**/*.rs"]   # a file is a test file exactly when one matches
///
/// [[conventions.tests_for]]           # candidate test files for a source file, in order
/// template = "{base}tests/{stem}.rs"
///
/// [[conventions.tests_for]]
/// template = "{base}tests/*.rs"       # globs match files in scope
/// stems = ["lib"]                     # only for sources with one of these stems
///
/// [[conventions.tests_for]]
/// template = "{path}"
/// contains = "#[cfg(test)]"           # only when the candidate's text contains this
/// ```
///
/// Placeholders: `{path}` the source path, `{dir}` its directory, `{stem}` its name before the
/// first dot, `{ext}` its extension with the dot, `{base}` the path before its last source root
/// directory (`[namespace].source_roots`, with a trailing `/`, or empty), and `{module}` the
/// path below that root without the extension. A candidate must be a file in scope.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ConventionRules {
    pub test_globs: Vec<String>,
    pub tests_for: Vec<TestsFor>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestsFor {
    pub template: String,
    #[serde(default)]
    pub stems: Vec<String>,
    #[serde(default)]
    pub contains: Option<String>,
}

/// `[roots]`: which symbols `QueryRoots` treats as live by name. Exports (public symbols), test
/// cases (tests.scm), and `@root.<reason>` captures in api.scm need no table entry.
///
/// ```toml
/// [roots]
/// entry_points = ["main"]        # functions with these names are entry points
/// hooks = ["__init__", "init"]   # symbols with these names are framework hooks
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RootRules {
    pub entry_points: Vec<String>,
    pub hooks: Vec<String>,
}

/// `[references]`: what the `References` producer reads besides `references.scm` (ADR-135).
///
/// ```toml
/// [references]
/// inert = ["inline", "allow"]   # attributes with these names only instruct the compiler or a
///                               # linter, so they are not references to what they are on
/// implicit = ["constructor"]    # methods with these names are called by the runtime
/// path_imports = true           # an import that lists no names binds its path's last
///                               # segment (Rust `use a::b` binds `b`); default false
/// ```
///
/// An attribute's name is its text up to its first `(`, with a leading `@` or `#[` and any
/// path before its last segment removed: `@functools.cache` is `cache`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ReferenceRules {
    pub inert: Vec<String>,
    pub implicit: Vec<String>,
    pub path_imports: bool,
}

/// `[api]`: how `QueryApi` spaces the tokens of a signature, beyond the shared rules documented
/// in `cupel_lang::api`.
///
/// ```toml
/// [api]
/// tight_after = ["]", "..."]   # no space after these tokens: Go's `[]T`, `map[K]V`, `...T`
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ApiRules {
    pub tight_after: Vec<String>,
}
