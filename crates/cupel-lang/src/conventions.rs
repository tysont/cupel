// ABOUTME: `GlobTestConventions`: which files are tests (globs) and which tests belong to which source
// ABOUTME: files (path templates), from `[conventions]`. Reads paths, and file text only for `contains`.
use std::collections::BTreeSet;
use std::fs;
use std::sync::Arc;

use cupel_core::syntax::{TestConventions, TestPairing};
use cupel_core::{
    Availability, Ctx, EventSink, FactKind, Langs, ProduceCtx, Producer, ProducerId, Result, Scope,
};
use globset::GlobBuilder;

use crate::language::{Language, owns, producer_id};
use crate::namespace::file_namespace_root;
use crate::tables::ConventionRules;
use crate::walk::language_files;

inventory::submit!(crate::generic::GenericProducer {
    kind: FactKind::TestConventions,
    drives: |spec| spec.tables().conventions.is_some(),
    attach: |language, _, reg| {
        reg.register::<TestConventions, _>(GlobTestConventions::new(language));
    },
});

/// The `TestConventions` producer of one language. Registered only for languages with a
/// `[conventions]` table.
pub struct GlobTestConventions {
    language: Arc<dyn Language>,
}

impl GlobTestConventions {
    pub fn new(language: Arc<dyn Language>) -> Self {
        Self { language }
    }
}

impl Producer<TestConventions> for GlobTestConventions {
    fn id(&self) -> ProducerId {
        producer_id(self.language.as_ref())
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![self.language.id()])
    }

    fn needs(&self) -> &[FactKind] {
        &[]
    }

    fn cost(&self) -> cupel_core::CostClass {
        cupel_core::CostClass::Quick
    }

    fn availability(&self, _: &Ctx) -> Availability {
        Availability::Ready
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<TestConventions> {
        ctx.cancel.checkpoint()?;
        let tables = self.language.spec().tables();
        let Some(rules) = tables.conventions.as_ref() else {
            return Ok(TestConventions::default());
        };
        let mut conventions = TestConventions {
            test_globs: rules.test_globs.clone(),
            pairs: Vec::new(),
        };
        let files: Vec<&str> = scope
            .files
            .iter()
            .map(String::as_str)
            .filter(|file| owns(self.language.as_ref(), file))
            .collect();
        // A source in scope pairs with its tests wherever they are: a scope narrowed to sources
        // (`--paths src`) leaves its tests out, and they test it all the same. The candidates are
        // every file of the language the walk keeps, as `References` reads them (ADR-135).
        let mut candidates =
            language_files(&ctx.ctx.root, self.language.as_ref(), &ctx.ctx.exclude);
        candidates.extend(files.iter().map(|file| (*file).to_owned()));
        candidates.sort();
        candidates.dedup();
        let candidates: Vec<&str> = candidates.iter().map(String::as_str).collect();
        let read = |file: &str| fs::read_to_string(ctx.ctx.root.join(file)).unwrap_or_default();
        let sources: Vec<&str> = files
            .iter()
            .copied()
            .filter(|file| !conventions.is_test(file))
            .collect();
        for source in sources {
            let roots = &tables.namespace.source_roots;
            let tests = tests_for(rules, source, roots, &candidates, &read);
            if !tests.is_empty() {
                conventions.pairs.push(TestPairing {
                    source: source.to_owned(),
                    tests,
                });
            }
        }
        Ok(conventions)
    }
}

/// The test files in `files` that `rules` pair with `source`, in rule order, without repeats.
fn tests_for(
    rules: &ConventionRules,
    source: &str,
    source_roots: &[String],
    files: &[&str],
    read: &dyn Fn(&str) -> String,
) -> Vec<String> {
    let mut tests: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    for rule in &rules.tests_for {
        let parts = Parts::of(source, source_roots);
        if !rule.stems.is_empty() && !rule.stems.iter().any(|stem| *stem == parts.stem) {
            continue;
        }
        for candidate in candidates(&parts.expand(&rule.template), files) {
            let wanted = rule
                .contains
                .as_ref()
                .is_none_or(|needle| read(candidate).contains(needle.as_str()));
            if wanted && seen.insert(candidate) {
                tests.push(candidate.to_owned());
            }
        }
    }
    tests
}

/// The files in scope a template names: itself, or every file its glob matches.
fn candidates<'a>(expanded: &str, files: &[&'a str]) -> Vec<&'a str> {
    if expanded.contains(['*', '?', '[']) {
        let Ok(glob) = GlobBuilder::new(expanded).literal_separator(true).build() else {
            return Vec::new();
        };
        let matcher = glob.compile_matcher();
        files
            .iter()
            .copied()
            .filter(|f| matcher.is_match(f))
            .collect()
    } else {
        files.iter().copied().filter(|f| *f == expanded).collect()
    }
}

/// The pieces of a source path that templates refer to.
struct Parts<'a> {
    path: &'a str,
    dir: &'a str,
    stem: &'a str,
    ext: &'a str,
    base: String,
    module: String,
}

impl<'a> Parts<'a> {
    fn of(path: &'a str, source_roots: &[String]) -> Self {
        let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
        let stem = name.split_once('.').map_or(name, |(stem, _)| stem);
        let ext = name.rfind('.').map_or("", |dot| &name[dot..]);
        let (base, module) = file_namespace_root(path, source_roots);
        Self {
            path,
            dir,
            stem,
            ext,
            base,
            module,
        }
    }

    fn expand(&self, template: &str) -> String {
        let expanded = template
            .replace("{path}", self.path)
            .replace("{dir}", self.dir)
            .replace("{stem}", self.stem)
            .replace("{ext}", self.ext)
            .replace("{base}", &self.base)
            .replace("{module}", &self.module);
        expanded.trim_start_matches('/').to_owned()
    }
}
