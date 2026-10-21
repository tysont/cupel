// ABOUTME: A language's declarative spec: its embedded tree-sitter queries, one per query file, and
// ABOUTME: its `tables.toml`, parsed once; queries compile lazily, once per grammar.
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

use tree_sitter::Query;

use crate::tables::Tables;

/// The query files a language may provide, one per generic producer family (design §8.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum QueryFile {
    Symbols,
    Decisions,
    Mutations,
    Tests,
    Imports,
    Api,
    References,
}

impl QueryFile {
    pub const ALL: [QueryFile; 7] = [
        Self::Symbols,
        Self::Decisions,
        Self::Mutations,
        Self::Tests,
        Self::Imports,
        Self::Api,
        Self::References,
    ];

    /// The file's name under `langs/<id>/queries/`.
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Symbols => "symbols.scm",
            Self::Decisions => "decisions.scm",
            Self::Mutations => "mutations.scm",
            Self::Tests => "tests.scm",
            Self::Imports => "imports.scm",
            Self::Api => "api.scm",
            Self::References => "references.scm",
        }
    }

    pub fn from_file_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|file| file.file_name() == name)
    }
}

impl fmt::Display for QueryFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.file_name())
    }
}

/// Why a query could not be had for a grammar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecError {
    /// The language has no such query file.
    Missing(QueryFile),
    /// The query does not compile against the grammar.
    Invalid { file: QueryFile, message: String },
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(file) => write!(f, "no {file}"),
            Self::Invalid { file, message } => write!(f, "{file} does not compile: {message}"),
        }
    }
}

/// Queries and tables that the generic producers interpret for one language.
pub struct LanguageSpec {
    sources: BTreeMap<QueryFile, String>,
    tables: Tables,
    compiled: Mutex<HashMap<(QueryFile, tree_sitter::Language), Arc<Query>>>,
}

impl LanguageSpec {
    /// A spec from embedded query sources and `tables.toml` text. Several sources for one query
    /// file are concatenated in order, which is how TypeScript extends the shared ECMAScript
    /// queries.
    ///
    /// # Panics
    ///
    /// When `tables` is not a valid `tables.toml`; the text is embedded at build time, and a
    /// test parses every language's tables.
    pub fn new(lang: &str, queries: &[(QueryFile, &str)], tables: &str) -> Self {
        let mut sources: BTreeMap<QueryFile, String> = BTreeMap::new();
        for (file, source) in queries {
            let text = sources.entry(*file).or_default();
            text.push_str(source);
            text.push('\n');
        }
        let tables = Tables::parse(tables)
            .unwrap_or_else(|error| panic!("langs/{lang}/tables.toml is invalid: {error}"));
        Self {
            sources,
            tables,
            compiled: Mutex::new(HashMap::new()),
        }
    }

    /// The query text for `file`, when the language has one.
    pub fn source(&self, file: QueryFile) -> Option<&str> {
        self.sources.get(&file).map(String::as_str)
    }

    pub fn has(&self, file: QueryFile) -> bool {
        self.sources.contains_key(&file)
    }

    pub fn tables(&self) -> &Tables {
        &self.tables
    }

    /// `file` compiled against `grammar`, compiled on first use and shared afterwards.
    pub fn query(
        &self,
        file: QueryFile,
        grammar: &tree_sitter::Language,
    ) -> Result<Arc<Query>, SpecError> {
        let source = self.source(file).ok_or(SpecError::Missing(file))?;
        let key = (file, grammar.clone());
        if let Some(query) = self.compiled().get(&key) {
            return Ok(Arc::clone(query));
        }
        let query = Arc::new(
            Query::new(grammar, source).map_err(|error| SpecError::Invalid {
                file,
                message: error.to_string(),
            })?,
        );
        Ok(Arc::clone(self.compiled().entry(key).or_insert(query)))
    }

    fn compiled(
        &self,
    ) -> std::sync::MutexGuard<'_, HashMap<(QueryFile, tree_sitter::Language), Arc<Query>>> {
        self.compiled.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
