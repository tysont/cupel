// ABOUTME: Root of cupel-lang, the language layer: grammars, language specs, the parse cache, the
// ABOUTME: file walk, and the generic query-driven producers of syntactic facts (design §8.4).
//! Cupel's language layer.
//!
//! A language is a grammar, a declarative [`LanguageSpec`] — tree-sitter queries under
//! `src/langs/<id>/queries/*.scm` and data in `src/langs/<id>/tables.toml` — and the producers
//! it registers. Generic producers interpret the spec, so adding a language is mostly writing
//! queries and tables; hand-written producers exist only where a query cannot express a fact.
//!
//! # Layout of a language
//!
//! ```text
//! src/langs/<dir>/
//! ├── mod.rs        # the `Language` impl: identity, grammars, `register()`, hand-written producers
//! ├── project.rs    # its build ecosystem for `ProjectInfo` (see `project`)
//! ├── queries/      # symbols.scm decisions.scm mutations.scm tests.scm imports.scm api.scm
//! └── tables.toml   # data the generic producers read (see `Tables`)
//! ```
//!
//! `javascript` and `typescript` share `langs/ecma/`; each places the files under
//! `langs/ecma/queries/<id>/` before the shared queries of the same name (ADR-023). Every
//! `.scm` file must be listed in its language's `include_str!` table; a test fails otherwise.
//!
//! A generic producer is registered for a language exactly when the language has the query
//! file that drives it, so `describe fact <kind>` lists a language once its query exists.
//!
//! # The capture-name contract
//!
//! Capture names are the interface between a query and its generic producer. The names a
//! query file must use (at least one of each group), may use, and may use with any suffix are
//! [`contract`]`(file)`; the names themselves are constants in [`capture`]. A test compiles
//! every query against every grammar of its language and checks it against the contract.
//! Captures starting with `_` are free for predicates (`(#eq? @_macro "assert")`).
//!
//! | File | Required | Optional |
//! |---|---|---|
//! | `symbols.scm` | `@symbol.function` / `@symbol.method` / `@symbol.closure`; `@symbol.name`; `@symbol.parameter` | `@symbol.body` `@symbol.visibility` `@visibility.public` `@visibility.internal` `@visibility.private` `@export.name` `@scope` `@scope.name` `@namespace.name` |
//! | `decisions.scm` | one of `@decision.branch` `.loop` `.boolean-op` `.case` `.catch` `.early-return` `.ternary` | |
//! | `mutations.scm` | one of `@mut.relational` `.equality` `.arith-add-sub` `.arith-mul-div` `.boolean-op` `.boolean-literal` `.zero-one` `.statement-deletion` `.return-value` `.constant` `.boundary` `.negation` | |
//! | `tests.scm` | `@test.case`; `@test.name` | `@test.assertion` `@assertion.name` `@assertion.idiom.<key>` `@test.mock` `@mock.target` `@test.skip` `@skip.reason` `@test.setup` `@framework.<name>` |
//! | `imports.scm` | `@import`; `@import.specifier` | `@import.name` `@import.module` `@scope` `@scope.name` |
//! | `api.scm` | one of `@api.function` `.method` `.type` `.interface` `.constant` `.variable` `.module`; `@api.name` | `@api.signature` `@api.local` `@root.entry-point` `@root.hook` `@scope` `@scope.name` `@namespace.name` |
//!
//! The meaning of each capture and the `tables.toml` keys each producer reads are documented
//! on the producer that reads them.

pub mod api;
mod contract;
mod conventions;
pub mod decisions;
mod generic;
pub mod imports;
mod langs;
mod language;
pub mod mutations;
mod namespace;
pub mod normalize;
mod parse;
mod producer;
pub mod project;
mod query;
pub mod roots;
mod spec;
pub mod symbols;
mod tables;
pub mod testframes;
mod walk;

pub use contract::{Contract, capture, contract};
pub use conventions::GlobTestConventions;
pub use language::{Language, LanguageRegistry, lang, owns};
pub use parse::{CacheStats, ParseCache, ParseProblem, Parsed, Syntax};
pub use producer::{Extract, Merge, QueryProducer, Source, parse_scope, producer_id};
pub use query::{Match, matches, span};
pub use spec::{LanguageSpec, QueryFile, SpecError};
pub use tables::{DecisionRules, NameRule, NamespaceRules, Tables, VisibilityRules, Weight};
pub use walk::walk;
