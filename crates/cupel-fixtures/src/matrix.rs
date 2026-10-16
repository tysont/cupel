// ABOUTME: The matrix test's engine (design §8.3.7, ADR-043): every (check, language) from the registries,
// ABOUTME: a golden per supported pair, a skip per unsupported pair, and checks run on declared facts only.
// ABOUTME: Fixtures run as whole trees, so a check that compares a range's two sides records its skip; a fixture's
// ABOUTME: `[plugins]` join its registries, and a plugin check has pairs where it has its own fixture (ADR-090).
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use cupel_core::config::ConfigFile;
use cupel_core::syntax::{ProjectInfo, SymbolDef, Symbols};
use cupel_core::{
    Availability, Cancellation, Check, CheckCtx, CheckId, CheckRegistry, CheckResult, CheckStatus,
    Ctx, Envelope, Facts, LangId, ProduceCtx, ProducerRegistry, Range, RunId, Scope, SkipReason,
    Support, SupportMatrix, Writer,
};
use cupel_lang::{LanguageRegistry, walk};
use cupel_plugin::{Discovery, Plugins};
use serde_json::Value;

use crate::evidence;
use crate::golden::Goldens;
use crate::redact::redact;
use crate::repos::{copy_repo, repo_path};
use crate::run::run_cupel;

/// The `--seed` every matrix run passes, so a check whose producers vary their work by seed
/// (the flake engine's reruns, ADR-078) makes the same golden on every run.
pub const SEED: u64 = 3;

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

    /// The fixture repository the pair runs on: see [`fixture_repo`].
    pub fn fixture(&self) -> String {
        fixture_repo(&repo_path("."), &self.check, &self.lang)
    }
}

/// The fixture repository (a name under `repos/`) that `check` runs on for `lang`: the check's
/// own `<check>/<lang>` when it has one, else the language's shared `<lang>`. Code in a check's
/// own repository reaches no other check's goldens, the fact goldens, or the language tests, so
/// a check added in parallel with others keeps its fixtures there (ADR-074).
pub fn fixture_repo(repos: &Path, check: &CheckId, lang: &LangId) -> String {
    let own = format!("{check}/{lang}");
    if repos.join(&own).is_dir() {
        own
    } else {
        lang.to_string()
    }
}

/// The registries the binary uses: compiled-in languages, their producers, the tool producers,
/// and the native checks.
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
        cupel_adapt::register(&mut producers);
        cupel_mutate::register(&mut producers);
        let mut checks = CheckRegistry::default();
        cupel_checks::register(&mut checks);
        Self {
            languages,
            producers,
            checks,
        }
    }

    /// The registries a run in the fixture repository `fixture` uses when its `cupel.toml`
    /// configures plugins: the binary's plus those plugins (ADR-090 §3); `PATH` is never
    /// searched. `None` when it configures none, or when a program they run is not installed,
    /// which is reported with a `SKIPPED:` line.
    ///
    /// # Panics
    ///
    /// When the fixture's `cupel.toml` does not parse, or a plugin it names cannot be used.
    pub fn of_fixture(fixture: &str) -> Option<Self> {
        let root = repo_path(fixture);
        let config = root.join("cupel.toml");
        let text = fs::read_to_string(&config).ok()?;
        let file = ConfigFile::parse(&text, &config)
            .unwrap_or_else(|error| panic!("fixture {fixture}: {error}"));
        if file.plugins.is_empty() || !programs_installed(&file, fixture) {
            return None;
        }
        let mut registries = Self::builtin();
        let plugins = cupel_plugin::discover(&Discovery {
            root: &root,
            entries: &file.plugins,
            path: None,
            languages: &registries.languages.ids(),
            taken: registries
                .checks
                .iter()
                .map(|check| check.manifest().id.clone())
                .collect(),
            cancel: &Cancellation::new(),
        })
        .unwrap_or_else(|error| panic!("fixture {fixture}: {error}"));
        registries.add(&plugins);
        Some(registries)
    }

    fn add(&mut self, plugins: &Plugins) {
        plugins.register_producers(&mut self.producers);
        plugins.register_checks(&mut self.checks);
    }

    /// The registries, built once and shared by every test of a test binary.
    pub fn shared() -> &'static Self {
        static REGISTRIES: std::sync::OnceLock<Registries> = std::sync::OnceLock::new();
        REGISTRIES.get_or_init(Self::builtin)
    }

    /// The registered language `id`.
    ///
    /// # Panics
    ///
    /// When no language `id` is registered.
    pub fn lang(&self, id: &str) -> cupel_core::Lang {
        self.languages
            .langs()
            .into_iter()
            .find(|lang| lang.id.as_str() == id)
            .unwrap_or_else(|| panic!("{id} is registered"))
    }

    /// Every (check, language) with its support outside any repository, as `describe matrix`
    /// computes it, ordered by check, then language.
    pub fn pairs(&self) -> Vec<Pair> {
        let matrix = SupportMatrix::compute(
            &self.checks,
            &self.producers,
            &self.languages.langs(),
            &Ctx::new(repo_path(".")),
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

    /// Every (check, language) with its support in the fixture repository it runs on, which is
    /// what the matrix test verifies: support is per repository, and a fresh coverage report
    /// committed in a fixture is part of it (ADR-079). Ordered by check, then language.
    /// A plugin check's pairs are those of its own fixtures (`repos/<check>/<lang>/`) whose
    /// `cupel.toml` registers it (ADR-090 §3).
    pub fn fixture_pairs(&self) -> Vec<Pair> {
        let repos = repo_path(".");
        let langs = self.languages.langs();
        let mut matrices: BTreeMap<String, SupportMatrix> = BTreeMap::new();
        let mut pairs = Vec::new();
        for check in self.checks.iter() {
            for lang in &langs {
                let id = &check.manifest().id;
                let fixture = fixture_repo(&repos, id, &lang.id);
                let matrix = matrices
                    .entry(fixture.clone())
                    .or_insert_with(|| self.matrix_in(&fixture));
                pairs.push(Pair {
                    check: id.clone(),
                    lang: lang.id.clone(),
                    support: matrix
                        .get(id, &lang.id)
                        .cloned()
                        .unwrap_or(Support::Supported),
                });
            }
        }
        pairs.extend(self.plugin_pairs(&repos));
        pairs
    }

    /// The support matrix in the fixture repository `fixture`, with its plugins.
    fn matrix_in(&self, fixture: &str) -> SupportMatrix {
        let ctx = Ctx::new(repo_path(fixture));
        let langs = self.languages.langs();
        match Self::of_fixture(fixture) {
            Some(own) => SupportMatrix::compute(&own.checks, &own.producers, &langs, &ctx),
            None => SupportMatrix::compute(&self.checks, &self.producers, &langs, &ctx),
        }
    }

    /// The pairs of plugin checks: each `repos/<check>/<lang>/` whose plugins register `<check>`
    /// and whose `<check>` is not a native check.
    fn plugin_pairs(&self, repos: &Path) -> Vec<Pair> {
        let mut pairs = Vec::new();
        for entry in fs::read_dir(repos).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = CheckId::from(name.as_str());
            if !name.contains('.') || self.checks.get(&id).is_some() {
                continue;
            }
            for lang in self.languages.ids() {
                let fixture = format!("{name}/{lang}");
                let Some(own) = Self::of_fixture(&fixture) else {
                    continue;
                };
                let matrix = own.matrix_in(&fixture);
                if let Some(support) = matrix.get(&id, &lang) {
                    pairs.push(Pair {
                        check: id.clone(),
                        lang,
                        support: support.clone(),
                    });
                }
            }
        }
        pairs.sort_by(|a, b| (&a.check, &a.lang).cmp(&(&b.check, &b.lang)));
        pairs
    }

    fn check(&self, id: &CheckId) -> &dyn Check {
        self.checks
            .get(id)
            .expect("pairs come from the check registry")
    }
}

/// Whether every bare program `file`'s plugins run (such as `python3`) is on `PATH`; says which is
/// not, as runners' tests do.
fn programs_installed(file: &ConfigFile, fixture: &str) -> bool {
    file.plugins.values().all(|entry| {
        let program = entry.program();
        let installed = program.contains('/') || cupel_core::find_program(program).is_some();
        if !installed {
            eprintln!("SKIPPED: {program} not installed; the plugins of {fixture} were not loaded");
        }
        installed
    })
}

/// Every way the matrix fails: a supported pair without a passing golden, an unsupported pair
/// that is not skipped with its reason, invalid evidence, and goldens no supported pair claims.
pub fn verify(goldens: &Goldens) -> Vec<String> {
    let registries = Registries::builtin();
    let pairs = registries.fixture_pairs();
    let mut problems = Vec::new();
    for pair in &pairs {
        let own = Registries::of_fixture(&pair.fixture());
        let check = own.as_ref().unwrap_or(&registries).check(&pair.check);
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
    copy_repo(&pair.fixture(), dir.path());
    let outcome = run_cupel(
        &[
            "check",
            "--only",
            pair.check.as_str(),
            "--format",
            "json",
            "--seed",
            &SEED.to_string(),
        ],
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
        // The matrix resolves tools at the repository root; the fixture's own detection may
        // prefer another runner of the language, which is just as missing.
        Support::Unavailable(tool) => {
            eprintln!(
                "SKIPPED: {tool} not installed; {} ran as `unavailable`",
                pair.golden()
            );
            skipped.reason == SkipReason::Unavailable
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

/// Goldens under a registered check's directory that nothing claims, and check directories no
/// registered check owns. A check's directory holds everything golden about that check alone:
/// `<lang>` per supported pair (the matrix), `describe` (`describe <check>`), and
/// `survey-<lang>` per language (its slice of the survey envelope).
fn stale_goldens(goldens: &Goldens, registries: &Registries, pairs: &[Pair]) -> Vec<String> {
    let mut claimed: BTreeSet<String> = pairs
        .iter()
        // A pair whose tool is missing here is supported where it is installed, so its golden
        // stays (CI installs every runner).
        .filter(|pair| matches!(pair.support, Support::Supported | Support::Unavailable(_)))
        .map(Pair::golden)
        .collect();
    for check in registries.checks.iter() {
        let id = &check.manifest().id;
        claimed.insert(format!("{id}/describe"));
        for lang in registries.languages.ids() {
            claimed.insert(format!("{id}/survey-{lang}"));
        }
    }
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
        let id = CheckId::from(name.as_str());
        if registries.checks.get(&id).is_none() && !pairs.iter().any(|pair| pair.check == id) {
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
                    "golden/{claimed_name}.json is stale: no supported pair, check, or survey claims it"
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
    let registries = Registries::of_fixture(&pair.fixture()).unwrap_or_else(Registries::builtin);
    let check = registries.check(&pair.check);
    let manifest = check.manifest();
    let language = registries
        .languages
        .get(&pair.lang)
        .ok_or_else(|| format!("no language `{}`", pair.lang))?;
    let lang = cupel_lang::lang(language.as_ref());
    let root = repo_path(&pair.fixture());
    let scratch = tempfile::tempdir().map_err(|error| error.to_string())?;
    let files = walk(&root, &registries.languages, &ProjectInfo::default());
    let produce_ctx = ProduceCtx::new(
        Ctx::new(root.clone()),
        Writer::new(scratch.path(), false)
            .map_err(|error| error.to_string())?
            .confined(),
        Cancellation::new(),
    );
    let scope = Scope::of_files(files.clone());
    // The producer the scheduler would choose: the first ready candidate; a want with none is
    // not served, as in a run.
    let produce = |kind| {
        let producer = registries
            .producers
            .candidates(kind, &lang, &produce_ctx.ctx)
            .ok()?
            .into_iter()
            .find(|producer| producer.availability(&produce_ctx.ctx) == Availability::Ready)?;
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
