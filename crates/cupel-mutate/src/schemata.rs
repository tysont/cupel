// ABOUTME: Mutant schemata (ADR-082): a language's rewriter gates every mutant of a file behind `CUPEL_MUTANT`, so the
// ABOUTME: file builds once; the engine asks `rewriter(lang)` and runs file copies where there is none.
use std::collections::{BTreeMap, BTreeSet};

use cupel_core::syntax::Span;

mod rust;

/// The environment variable that selects a schemata mutant at run time; `0` selects none.
pub const MUTANT_VAR: &str = "CUPEL_MUTANT";

/// One mutant to gate: its id, and its site's span, text, and replacement.
#[derive(Debug, Clone, Copy)]
pub struct Gate<'a> {
    /// 1-based; `CUPEL_MUTANT=<id>` selects it.
    pub id: u32,
    pub span: Span,
    pub from: &'a str,
    pub to: &'a str,
}

/// A file rewritten as schemata.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rewritten {
    /// The rewritten file; `None` when no gate could be placed.
    pub source: Option<String>,
    /// The gates left out, by id, with why: they run as file copies.
    pub excluded: BTreeMap<u32, String>,
    /// The line of each placed gate's expression, by id: where the compiler reports it.
    pub lines: BTreeMap<u32, u32>,
}

/// Rewrites one language's files as schemata.
pub trait Rewriter: Sync {
    /// `source` with every gate it can place behind the mutant switch, and the reader appended.
    fn rewrite(&self, source: &str, gates: &[Gate<'_>]) -> Rewritten;

    /// The lines of `file` that the build `output` names in its errors.
    fn culprits(&self, output: &str, file: &str) -> BTreeSet<u32>;
}

/// The schemata rewriter of language `lang`; `None` where its mutants run as file copies.
pub fn rewriter(lang: &str) -> Option<&'static dyn Rewriter> {
    match lang {
        "rust" => Some(&rust::Rust),
        _ => None,
    }
}
