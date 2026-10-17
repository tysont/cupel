// ABOUTME: The PIT producer plugin (ADR-110) driven through cupel-plugin's host as the CLI drives it: its manifest,
// ABOUTME: a committed PIT report placed on Cupel's Java mutation sites, a real PIT run on a copy, and its failures.
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::config::PluginEntry;
use cupel_core::syntax::{MutationSites, NormTree, Symbols};
use cupel_core::{
    Cancellation, Ctx, FactKind, Lang, LangFamily, LangId, MutantOutcome, MutationOutcomes,
    ProduceCtx, Produced, ProducerRegistry, Scope, SharedFact, Writer, find_program,
};
use cupel_fixtures::conformance::produce_fact_at;
use cupel_fixtures::{Capture, copy_repo};
use cupel_lang::LanguageRegistry;
use cupel_plugin::{Discovery, discover};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};

/// The plugin as `[plugins]` runs it: `python3` and the script in the repository's `plugins/`.
fn command() -> Vec<String> {
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/pit/cupel_produce_pit.py")
        .canonicalize()
        .unwrap();
    vec!["python3".to_owned(), script.display().to_string()]
}

fn java() -> Lang {
    Lang {
        id: LangId::from("java"),
        family: LangFamily::from("java"),
    }
}

/// Whether `python3` is installed; says the tests were skipped when it is not.
fn python() -> bool {
    let installed = find_program("python3").is_some();
    if !installed {
        eprintln!("SKIPPED: python3 not installed; the PIT plugin did not run");
    }
    installed
}

/// The producers the plugin registers, discovered in `root`.
fn registered(root: &Path) -> ProducerRegistry {
    let entries = [("pit".to_owned(), PluginEntry::Command(command()))].into();
    let plugins = discover(&Discovery {
        root,
        entries: &entries,
        path: None,
        languages: &[LangId::from("java"), LangId::from("python")],
        taken: Vec::new(),
        cancel: &Cancellation::new(),
    })
    .unwrap();
    let mut producers = ProducerRegistry::default();
    plugins.register_producers(&mut producers);
    producers
}

/// A copy of the Java mutation fixture without its link to the plugins.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("tests.mutation/java", dir.path());
    fs::remove_dir_all(dir.path().join("plugins")).unwrap();
    fs::remove_file(dir.path().join("cupel.toml")).unwrap();
    dir
}

/// The Java producers' `kind` fact over the repository at `root`.
fn fact<F: DeserializeOwned + cupel_core::Fact>(root: &Path, kind: FactKind) -> SharedFact {
    let languages = LanguageRegistry::builtin();
    let mut producers = ProducerRegistry::default();
    languages.register_producers(&mut producers);
    let json = produce_fact_at(&producers, &languages, &java(), kind, root).unwrap();
    Produced::new(serde_json::from_value::<F>(json).unwrap()).share()
}

/// The plugin's outcomes in `root` under `config` (the `[lang.java]` table), with the facts it
/// needs produced first, as the scheduler would.
fn produce(root: &Path, config: Map<String, Value>) -> cupel_core::Result<MutationOutcomes> {
    let producers = registered(root);
    let producer = producers
        .producers_for(FactKind::MutationOutcomes, &java())
        .next()
        .unwrap();
    let mut ctx = ProduceCtx::new(
        Ctx::new(root.to_path_buf()),
        Writer::new(root, false).unwrap().confined(),
        Cancellation::new(),
    );
    ctx.config = config;
    ctx.upstream
        .insert(fact::<Symbols>(root, FactKind::Symbols));
    ctx.upstream
        .insert(fact::<NormTree>(root, FactKind::NormTree));
    ctx.upstream
        .insert(fact::<MutationSites>(root, FactKind::MutationSites));
    producer
        .produce(&ctx, &Scope::of_files(Vec::new()), &mut Vec::new())
        .map(|made| made.downcast::<MutationOutcomes>().unwrap())
}

/// Each mutant as `(line, operator, from, to, outcome, killing tests)`.
fn shape(outcomes: &MutationOutcomes) -> Vec<(u32, String, String, String, MutantOutcome, usize)> {
    outcomes
        .mutants
        .iter()
        .map(|mutant| {
            (
                mutant.span.line,
                serde_json::to_value(mutant.operator)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned(),
                mutant.from.clone(),
                mutant.to.clone(),
                mutant.outcome,
                mutant.killing_tests.len(),
            )
        })
        .collect()
}

/// What PIT's report of the fixture is, placed on Cupel's sites.
fn expected() -> Vec<(u32, String, String, String, MutantOutcome, usize)> {
    use MutantOutcome::{Killed, Survived, Uncovered};
    let row = |line: u32, operator: &str, from: &str, to: &str, outcome, killing: usize| {
        (
            line,
            operator.to_owned(),
            from.to_owned(),
            to.to_owned(),
            outcome,
            killing,
        )
    };
    vec![
        row(
            10,
            "negation",
            "total >= 100 && member",
            "negated conditional",
            Killed,
            1,
        ),
        row(10, "relational", ">=", ">", Survived, 0),
        row(10, "relational", ">=", "negated conditional", Killed, 2),
        row(
            11,
            "return-value",
            "return total - 10;",
            "replaced int return with 0",
            Killed,
            1,
        ),
        row(11, "arith-add-sub", "-", "+", Killed, 1),
        row(
            13,
            "return-value",
            "return total * 1;",
            "replaced int return with 0",
            Killed,
            1,
        ),
        row(13, "arith-mul-div", "*", "/", Survived, 0),
        row(
            18,
            "return-value",
            "return total - 5;",
            "replaced int return with 0",
            Uncovered,
            0,
        ),
        row(18, "arith-add-sub", "-", "+", Uncovered, 0),
    ]
}

/// The `[lang.java]` table that points the plugin at a report already in the tree.
fn committed_report(root: &Path) -> Map<String, Value> {
    fs::write(
        root.join("mutations.xml"),
        Capture::of("pit", "discount").file("mutations.xml"),
    )
    .unwrap();
    Map::from_iter([
        ("pit_report".to_owned(), json!("mutations.xml")),
        ("pit_report_ready".to_owned(), json!(true)),
    ])
}

#[test]
fn the_plugin_describes_a_java_mutation_outcomes_producer_the_host_admits() {
    if !python() {
        return;
    }
    let root = fixture();
    let producers = registered(root.path());
    let ids: Vec<String> = producers
        .producers_for(FactKind::MutationOutcomes, &java())
        .map(|producer| producer.id().to_string())
        .collect();
    assert_eq!(ids, ["plugin.pit"]);
    let python = Lang {
        id: LangId::from("python"),
        family: LangFamily::from("python"),
    };
    assert_eq!(
        producers
            .producers_for(FactKind::MutationOutcomes, &python)
            .count(),
        0
    );
}

#[test]
fn a_pit_report_becomes_mutants_on_cupels_sites_with_their_tests() {
    if !python() {
        return;
    }
    let root = fixture();
    let config = committed_report(root.path());
    let outcomes = produce(root.path(), config).unwrap();
    assert_eq!(shape(&outcomes), expected());
    assert_eq!(
        (outcomes.engine.as_str(), outcomes.per_test),
        ("plugin.pit", true)
    );
    let survived = &outcomes.mutants[1];
    assert_eq!(
        (
            survived.symbol.file.as_str(),
            survived.symbol_hash.is_some(),
            survived.tests.clone()
        ),
        (
            "src/main/java/discount/Discount.java",
            true,
            vec![
                "discount.LargeTest#discountsALargeMemberOrder".to_owned(),
                "discount.SmallTest#keepsASmallOrder".to_owned()
            ]
        )
    );
}

#[test]
fn without_a_report_the_plugin_fails_with_its_reason() {
    if !python() {
        return;
    }
    let root = fixture();
    let config = Map::from_iter([
        ("pit_report".to_owned(), json!("missing.xml")),
        ("pit_report_ready".to_owned(), json!(true)),
    ]);
    let error = produce(root.path(), config).unwrap_err().to_string();
    assert!(error.contains("no PIT report at missing.xml"), "{error}");
}

/// Every file below `root`, relative to it, sorted.
fn files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                found.push(path.strip_prefix(root).unwrap().to_path_buf());
            }
        }
    }
    found.sort();
    found
}

#[test]
fn pit_runs_on_a_copy_of_the_project_and_leaves_the_tree_as_it_was() {
    if !python() {
        return;
    }
    if find_program("mvn").is_none() {
        eprintln!("SKIPPED: mvn not installed; PIT was not run");
        return;
    }
    let root = fixture();
    let before = files(root.path());
    let outcomes = produce(root.path(), Map::new()).unwrap();
    assert_eq!(shape(&outcomes), expected());
    let after: Vec<PathBuf> = files(root.path())
        .into_iter()
        .filter(|file| !file.starts_with(".cupel"))
        .collect();
    assert_eq!(after, before);
}
