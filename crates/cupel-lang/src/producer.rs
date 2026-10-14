// ABOUTME: The generic-producer scaffold: parse the language's files in scope (in parallel, through the
// ABOUTME: cache), extract a partial fact per file, merge in scope order, and report broken files.
use std::sync::Arc;

use cupel_core::syntax::{
    ApiSurface, Decisions, MutationSites, NormTree, Roots, Symbols, TestFrames,
};
use cupel_core::{
    Action, ActionKind, Availability, CheckId, CostClass, Ctx, Event, EventSink, Evidence, Fact,
    FactKind, Finding, FindingId, Langs, Location, ProduceCtx, Producer, ProducerId, Result, Scope,
    Severity, Tier,
};
use rayon::prelude::*;
use serde_json::json;

use crate::language::{Language, owns, parse_one};
use crate::parse::{ParseCache, ParseProblem, Parsed};

/// What an extractor sees: one parsed file and the language it belongs to.
pub struct Source<'a> {
    pub parsed: &'a Parsed,
    pub language: &'a dyn Language,
}

/// Extracts one file's share of a fact.
pub type Extract<F> = fn(&Source<'_>) -> F;

/// A scope-wide fact built from per-file parts (ADR-014).
pub trait Merge: Default {
    /// Appends `other`, a later file's part.
    fn merge(&mut self, other: Self);
}

macro_rules! merge_list {
    ($fact:ty, $field:ident) => {
        impl Merge for $fact {
            fn merge(&mut self, other: Self) {
                self.$field.extend(other.$field);
            }
        }
    };
}

merge_list!(Symbols, symbols);
merge_list!(Decisions, symbols);
merge_list!(NormTree, symbols);
merge_list!(MutationSites, symbols);
merge_list!(TestFrames, files);
merge_list!(ApiSurface, items);
merge_list!(Roots, roots);

/// A producer that runs `extract` over every file of its language in scope. Every generic
/// producer (`QuerySymbols`, `QueryDecisions`, …) is one of these with its own extractor.
pub struct QueryProducer<F> {
    language: Arc<dyn Language>,
    cache: Arc<ParseCache>,
    extract: Extract<F>,
}

impl<F> QueryProducer<F> {
    pub fn new(language: Arc<dyn Language>, cache: Arc<ParseCache>, extract: Extract<F>) -> Self {
        Self {
            language,
            cache,
            extract,
        }
    }
}

impl<F: Fact + Merge> Producer<F> for QueryProducer<F> {
    fn id(&self) -> ProducerId {
        producer_id(self.language.as_ref())
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.language.id()])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> CostClass {
        CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(&self, ctx: &ProduceCtx, scope: &Scope, sink: &mut dyn EventSink) -> Result<F> {
        let parsed = parse_scope(&self.cache, ctx, scope, self.language.as_ref(), sink)?;
        let language = self.language.as_ref();
        let parts: Vec<F> = parsed
            .par_iter()
            .map(|parsed| (self.extract)(&Source { parsed, language }))
            .collect();
        let mut fact = F::default();
        for part in parts {
            fact.merge(part);
        }
        Ok(fact)
    }
}

/// `lang.<id>`, the ID every producer a language registers carries.
pub fn producer_id(language: &dyn Language) -> ProducerId {
    ProducerId::from(format!("lang.{}", language.id()))
}

/// Parses the files in `scope` that `language` owns, in scope order, reporting each file that
/// cannot be parsed, or parses with errors, as a `lang.parse-error` finding. Files with
/// syntax errors are still returned: tree-sitter recovers, and the rest of the file counts.
pub fn parse_scope(
    cache: &ParseCache,
    ctx: &ProduceCtx,
    scope: &Scope,
    language: &dyn Language,
    sink: &mut dyn EventSink,
) -> Result<Vec<Parsed>> {
    ctx.cancel.checkpoint()?;
    let outcomes: Vec<std::result::Result<Parsed, ParseProblem>> = scope
        .files
        .par_iter()
        .filter(|file| owns(language, file))
        .map(|file| parse_one(cache, &ctx.ctx.root, file, language, &ctx.cancel))
        .collect();
    ctx.cancel.checkpoint()?;
    let mut parsed = Vec::with_capacity(outcomes.len());
    for outcome in outcomes {
        match outcome {
            Ok(file) => {
                if let Some(error) = file.first_error() {
                    let start = error.start_position();
                    sink.emit(Event::Finding(Box::new(parse_error(
                        &file.file,
                        &file.lang.to_string(),
                        line_col(start.row, start.column),
                        &format!(
                            "unexpected syntax at line {}, column {}",
                            start.row + 1,
                            start.column + 1
                        ),
                    ))));
                }
                parsed.push(file);
            }
            Err(problem) => sink.emit(Event::Finding(Box::new(parse_error(
                &problem.file,
                problem.lang.as_str(),
                None,
                &problem.reason,
            )))),
        }
    }
    Ok(parsed)
}

/// A 0-based tree-sitter row and byte column as 1-based numbers.
fn line_col(row: usize, column: usize) -> Option<(u32, u32)> {
    Some((
        u32::try_from(row + 1).ok()?,
        u32::try_from(column + 1).ok()?,
    ))
}

/// A `lang.parse-error` finding about `file`; `at` is where the first error is, when known.
fn parse_error(file: &str, lang: &str, at: Option<(u32, u32)>, reason: &str) -> Finding {
    let check = CheckId::from("lang");
    let kind = "lang.parse-error";
    let id = FindingId::for_subject(&check, kind, file);
    let (line, col) = at.map_or((1, None), |(line, col)| (line, Some(col)));
    Finding {
        suppress: format!("cupel suppress {id} --reason '…'"),
        id,
        check,
        kind: kind.to_owned(),
        severity: Severity::Warn,
        confidence: 1.0,
        tier: Tier::try_from(0).expect("tier 0 is valid"),
        symbol: None,
        location: Some(Location {
            file: file.to_owned(),
            line,
            col,
            end_line: line,
            end_col: col,
        }),
        title: format!("`{file}` does not parse as {lang}"),
        detail: Some(reason.to_owned()),
        evidence: Evidence(
            json!({ "file": file, "lang": lang, "reason": reason })
                .as_object()
                .cloned()
                .unwrap_or_default(),
        ),
        action: Action {
            kind: ActionKind::Review,
            priority: 2,
            summary: format!(
                "Fix the syntax error; if the file is valid {lang}, the grammar does not \
                 understand it and its facts may be incomplete."
            ),
            hints: serde_json::Map::new(),
        },
        repro: format!("cupel survey --paths {file}"),
        new_since_baseline: false,
        suppressed: false,
    }
}
