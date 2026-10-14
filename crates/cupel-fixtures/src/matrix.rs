// ABOUTME: The matrix test's engine (design §8.3.7, ADR-043): every (check, language) from the registries,
// ABOUTME: a golden per supported pair, a skip per unsupported pair, and checks run on declared facts only.
// ABOUTME: Fixtures run as whole trees, so a check that compares a range's two sides records its skip.
use std::collections::BTreeSet;
use std::fs;

use cupel_core::syntax::{ProjectInfo, SymbolDef, Symbols};
use cupel_core::{
    Cancellation, Check, CheckCtx, CheckId, CheckRegistry, CheckResult, CheckStatus, Ctx, Envelope,
    Facts, LangId, ProduceCtx, ProducerRegistry, Range, RunId, Scope, SkipReason, Support,
    SupportMatrix, Writer,
};
use cupel_lang::{LanguageRegistry, walk};
use serde_json::Value;

use crate::evidence;
use crate::golden::Goldens;
use crate::redact::redact;
use crate::repos::{copy_repo, repo_path};
use crate::run::run_cupel;

/// One cell of the support matrix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    pub check: CheckId,
    pub lang: LangId,
    pub support: Support,
}

impl Pair {
    /// `golden/<check>/<lang>`.
    pub fn golden(&self) -> String {
        format!("{}/{}", self.check, self.lang)
    }
}

/// The registries the binary uses: compiled-in languages, their producers, the native checks.
pub struct Registries {
    pub languages: LanguageRegistry,
    pub producers: ProducerRegistry,
    pub checks: CheckRegistry,
}

impl Registries {
    pub fn builtin() -> Self {
        let languages = LanguageRegistry::builtin();
        let mut producers = ProducerRegistry::default();
        languages.register_producers(&mut producers);
        let mut checks = CheckRegistry::default();
        cupel_checks::register(&mut checks);
        Self {
            languages,
            producers,
            checks,
        }
    }

    /// Every (check, language) with its support, ordered by check, then language.
    pub fn pairs(&self) -> Vec<Pair> {
        let matrix = SupportMatrix::compute(
            &self.checks,
            &self.producers,
            &self.languages.langs(),
            &Ctx {
                root: repo_path("."),
            },
        );
        matrix
            .cells()
            .map(|(check, lang, support)| Pair {
                check: check.clone(),
                lang: lang.clone(),
                support: support.clone(),
            })
            .collect()
    }

    fn check(&self, id: &CheckId) -> &dyn Check {
        self.checks
            .get(id)
            .expect("pairs come from the check registry")
    }
}

/// Every way the matrix fails: a supported pair without a passing golden, an unsupported pair
/// that is not skipped with its reason, invalid evidence, and goldens no supported pair claims.
pub fn verify(goldens: &Goldens) -> Vec<String> {
    let registries = Registries::builtin();
    let pairs = registries.pairs();
    let mut problems = Vec::new();
    for pair in &pairs {
        let check = registries.check(&pair.check);
        let outcome = match pair.support {
            Support::Supported => verify_supported(goldens, check, pair),
            _ => verify_skipped(pair),
        };
        if let Err(problem) = outcome {
            problems.push(format!("{}: {problem}", pair.golden()));
        }
    }
    problems.extend(stale_goldens(goldens, &registries, &pairs));
    problems
}

/// Runs `check --only <check>` on a copy of the language's fixture repository.
fn run_on_fixture(
    pair: &Pair,
) -> Result<(Envelope, std::path::PathBuf, tempfile::TempDir), String> {
    let dir = tempfile::tempdir().map_err(|error| error.to_string())?;
    copy_repo(pair.lang.as_str(), dir.path());
    let outcome = run_cupel(
        &["check", "--only", pair.check.as_str(), "--format", "json"],
        dir.path(),
    );
    let envelope = outcome
        .envelope
        .clone()
        .ok_or_else(|| format!("no envelope (exit {:?}): {}", outcome.code, outcome.stderr))?;
    if outcome.code != Some(0) {
        return Err(format!(
            "exit {:?}: {}",
            outcome.code,
            serde_json::to_string(&envelope.findings).unwrap_or_default()
        ));
    }
    Ok((envelope, outcome.cwd, dir))
}

/// A supported pair runs and matches its golden. A check that reads a range's base side
/// (ADR-056) cannot run on a whole tree, so its golden records the `needs-range` skip.
fn verify_supported(goldens: &Goldens, check: &dyn Check, pair: &Pair) -> Result<(), String> {
    let (envelope, root, _dir) = run_on_fixture(pair)?;
    let needs_range = !check.base_needs().is_empty();
    if let Some(skip) = envelope.checks.iter().find(|result| {
        result.skipped.as_ref().is_some_and(|skipped| {
            !needs_range
                || skipped.reason != SkipReason::NeedsRange
                || skipped.lang.as_ref() != Some(&pair.lang)
        })
    }) {
        return Err(format!("a supported pair was skipped: {:?}", skip.skipped));
    }
    if needs_range
        && !envelope
            .checks
            .iter()
            .any(|result| result.skipped.is_some())
    {
        return Err("a check that reads the base side ran on a whole tree".to_owned());
    }
    for finding in &envelope.findings {
        if finding.check != pair.check {
            return Err(format!(
                "the fixture produced a `{}` finding ({}); fixtures must parse cleanly",
                finding.kind, finding.title
            ));
        }
        let spec = check
            .manifest()
            .findings
            .iter()
            .find(|spec| spec.kind == finding.kind)
            .ok_or_else(|| format!("finding kind `{}` is not in the manifest", finding.kind))?;
        evidence::validate(&spec.evidence, &Value::Object(finding.evidence.0.clone()))
            .map_err(|problem| format!("{} {}: {problem}", finding.kind, finding.id))?;
    }
    goldens
        .check(&pair.golden(), &redact(&envelope, &root))
        .map_err(|error| error.to_string())
}

fn verify_skipped(pair: &Pair) -> Result<(), String> {
    let (envelope, _, _dir) = run_on_fixture(pair)?;
    let skipped = envelope
        .checks
        .iter()
        .filter(|result| result.id == pair.check)
        .find_map(|result| {
            result
                .skipped
                .as_ref()
                .filter(|skipped| skipped.lang.as_ref() == Some(&pair.lang))
        })
        .ok_or_else(|| "no `skipped` entry for the language".to_owned())?;
    let expected = match &pair.support {
        Support::Unsupported(fact) => {
            skipped.reason == SkipReason::Unsupported && skipped.fact == Some(*fact)
        }
        Support::Unavailable(tool) => {
            skipped.reason == SkipReason::Unavailable && skipped.tool.as_ref() == Some(tool)
        }
        Support::Unconfigured(_) => skipped.reason == SkipReason::Unconfigured,
        Support::Supported => true,
    };
    if expected {
        Ok(())
    } else {
        Err(format!(
            "skipped as {skipped:?}, but the matrix says {:?}",
            pair.support
        ))
    }
}

/// Goldens under a registered check's directory that no supported pair claims, and check
/// directories no registered check owns.
fn stale_goldens(goldens: &Goldens, registries: &Registries, pairs: &[Pair]) -> Vec<String> {
    let claimed: BTreeSet<String> = pairs
        .iter()
        .filter(|pair| pair.support == Support::Supported)
        .map(Pair::golden)
        .collect();
    let mut problems = Vec::new();
    let Ok(entries) = fs::read_dir(goldens.dir()) else {
        return problems;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        // Check directories are named by check ID, which always has a dot (ADR-017).
        if !name.contains('.') || !entry.path().is_dir() {
            continue;
        }
        if registries
            .checks
            .get(&CheckId::from(name.as_str()))
            .is_none()
        {
            problems.push(format!("golden/{name}/ belongs to no registered check"));
            continue;
        }
        for golden in fs::read_dir(entry.path()).into_iter().flatten().flatten() {
            let path = golden.path();
            let stem = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let claimed_name = format!("{name}/{stem}");
            if !claimed.contains(&claimed_name) {
                problems.push(format!(
                    "golden/{claimed_name}.json is stale: the pair is not supported"
                ));
            }
        }
    }
    problems
}

/// Runs `pair`'s check on its language's fixture with a `Facts` built from the manifest alone:
/// exactly its `needs`, plus each `want` the language produces. A check that reads anything
/// else gets `UndeclaredFact` and fails here. A check with `base_needs` gets the fixture as
/// both sides of an empty range (ADR-056).
pub fn run_with_declared_facts(pair: &Pair) -> Result<CheckResult, String> {
    let registries = Registries::builtin();
    let check = registries.check(&pair.check);
    let manifest = check.manifest();
    let language = registries
        .languages
        .get(&pair.lang)
        .ok_or_else(|| format!("no language `{}`", pair.lang))?;
    let lang = cupel_lang::lang(language.as_ref());
    let root = repo_path(pair.lang.as_str());
    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let files = walk(&root, &registries.languages, &ProjectInfo::default());
    let produce_ctx = ProduceCtx {
        ctx: Ctx { root: root.clone() },
        config: serde_json::Map::new(),
        writer: Writer::new(scratch.path(), false)
            .map_err(|error| error.to_string())?
            .confined(),
        cancel: Cancellation::new(),
    };
    let scope = Scope::of_files(files.clone());
    let produce = |kind| {
        let producer = registries.producers.producers_for(kind, &lang).next()?;
        let mut events = Vec::new();
        Some(
            producer
                .produce(&produce_ctx, &scope, &mut events)
                .map(cupel_core::Produced::share)
                .map_err(|error| format!("{kind}: {error}")),
        )
    };
    let mut facts = Facts::for_manifest(manifest);
    for kind in &manifest.needs {
        let fact = produce(*kind).ok_or_else(|| format!("no producer of `{kind}`"))??;
        facts
            .insert_shared(&fact)
            .map_err(|error| error.to_string())?;
    }
    for kind in &manifest.wants {
        if let Some(fact) = produce(*kind) {
            facts
                .insert_shared(&fact?)
                .map_err(|error| error.to_string())?;
        }
    }
    for kind in check.base_needs() {
        let fact = produce(*kind).ok_or_else(|| format!("no producer of `{kind}`"))??;
        facts
            .insert_base_shared(&fact)
            .map_err(|error| error.to_string())?;
    }
    if !check.base_needs().is_empty() {
        facts.set_range(Range::default());
    }
    let symbols = produce(cupel_core::FactKind::Symbols)
        .transpose()?
        .and_then(|fact| fact.downcast_ref::<Symbols>().cloned())
        .unwrap_or_default();
    let scope = Scope {
        files,
        symbols: Some(symbols.symbols.iter().map(SymbolDef::key).collect()),
    };
    let ctx = CheckCtx {
        scope: scope.clone(),
        config: serde_json::Map::new(),
        writer: Writer::new(scratch.path(), false)
            .map_err(|error| error.to_string())?
            .scratch(&RunId::generate()),
        cancel: Cancellation::new(),
    };
    let plan = check
        .plan(&ctx, &scope)
        .map_err(|error| error.to_string())?;
    let result = check
        .run(&ctx, &facts, &plan, &mut Vec::new())
        .map_err(|error| error.to_string())?;
    if result.status == CheckStatus::Error {
        return Err(format!("the check reported an error: {result:?}"));
    }
    Ok(result)
}
