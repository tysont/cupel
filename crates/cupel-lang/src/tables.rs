// ABOUTME: The typed form of a language's `tables.toml`: the data the generic producers read
// ABOUTME: alongside the queries. Unknown keys are errors, so a typo fails the spec test.
use std::collections::BTreeMap;

use cupel_core::syntax::Visibility;
use serde::Deserialize;

/// Everything a language's `tables.toml` may contain. Every section is optional; a section a
/// language omits takes the defaults documented on its type.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Tables {
    pub namespace: NamespaceRules,
    pub visibility: VisibilityRules,
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
