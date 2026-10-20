// ABOUTME: The matrix test (design §8.3.7, plan §2.8): one trial per (check, language) from the registries for its
// ABOUTME: golden or skip and one for its declared-facts run (ADR-043, ADR-074), so a mutant runs only its pairs (ADR-129).
use std::fs;
use std::path::Path;

use cupel_core::{CheckId, LangId, Support};
use cupel_fixtures::matrix::{
    Registries, fixture_repo, run_with_declared_facts, stale_goldens, verify_pair,
};
use cupel_fixtures::{GoldenMode, Goldens};
use libtest_mimic::{Arguments, Failed, Trial};

/// A custom harness (`harness = false`): the trials are named from the registries at run time,
/// one per pair, so nextest runs each in its own process and the per-test map attributes to each
/// pair only the code its run executes. Listing computes no support; each trial computes its own
/// pair's in the fixture repository it runs on.
fn main() {
    let args = Arguments::from_args();
    let mut trials = Vec::new();
    for (check, lang) in Registries::shared().pair_ids() {
        let name = format!("{check}/{lang}");
        let (golden, declared) = ((check.clone(), lang.clone()), (check, lang));
        trials.push(Trial::test(format!("golden::{name}"), move || {
            golden_or_skip(&golden.0, &golden.1)
        }));
        trials.push(Trial::test(format!("declared_facts::{name}"), move || {
            runs_given_only_its_declared_facts(&declared.0, &declared.1)
        }));
    }
    trials.push(Trial::test("no_golden_is_stale", no_golden_is_stale));
    let plain: [(&str, fn()); 3] = [
        (
            "deleting_a_golden_fails_its_pair_and_names_it",
            deleting_a_golden_fails_its_pair_and_names_it,
        ),
        (
            "a_stray_golden_is_stale_and_named",
            a_stray_golden_is_stale_and_named,
        ),
        (
            "a_check_runs_on_its_own_fixture_repository_where_it_has_one",
            a_check_runs_on_its_own_fixture_repository_where_it_has_one,
        ),
    ];
    for (name, test) in plain {
        trials.push(Trial::test(name, move || {
            test();
            Ok(())
        }));
    }
    libtest_mimic::run(&args, trials).exit();
}

/// A supported pair matches its golden; any other is skipped with its reason. A plugin fixture
/// that does not register its check has no pair.
fn golden_or_skip(check: &CheckId, lang: &LangId) -> Result<(), Failed> {
    let registries = Registries::shared();
    let Some(pair) = registries.fixture_pair(check, lang) else {
        return Ok(());
    };
    verify_pair(&Goldens::from_env(), registries, &pair).map_err(Failed::from)
}

/// A supported pair's check runs given exactly the facts its manifest declares; a pair that is
/// not supported has nothing to run.
fn runs_given_only_its_declared_facts(check: &CheckId, lang: &LangId) -> Result<(), Failed> {
    let Some(pair) = Registries::shared()
        .fixture_pair(check, lang)
        .filter(|pair| pair.support == Support::Supported)
    else {
        return Ok(());
    };
    run_with_declared_facts(&pair)
        .map(drop)
        .map_err(|problem| format!("{}: {problem}", pair.golden()).into())
}

/// No golden under a check's directory is left that no pair, check, or survey claims.
fn no_golden_is_stale() -> Result<(), Failed> {
    let registries = Registries::shared();
    let problems = stale_goldens(
        &Goldens::from_env(),
        registries,
        &registries.fixture_pairs(),
    );
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("\n{}", problems.join("\n\n")).into())
    }
}

/// Copies `from` (a directory of goldens) into `into`.
fn copy_dir(from: &Path, into: &Path) {
    fs::create_dir_all(into).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = into.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn deleting_a_golden_fails_its_pair_and_names_it() {
    // A static pair, so the test runs no tool.
    let pair = Registries::shared()
        .fixture_pair(&CheckId::from("static.complexity"), &LangId::from("rust"))
        .filter(|pair| pair.support == Support::Supported)
        .expect("static.complexity is supported for rust");
    let dir = tempfile::tempdir().unwrap();
    copy_dir(Goldens::from_env().dir(), dir.path());
    fs::remove_file(dir.path().join(format!("{}.json", pair.golden()))).unwrap();

    let problem = verify_pair(
        &Goldens::new(dir.path(), GoldenMode::Compare),
        Registries::shared(),
        &pair,
    )
    .expect_err("a pair without its golden fails");

    let missing = format!("{}: golden", pair.golden());
    assert!(
        problem.starts_with(&missing) && problem.contains("does not exist"),
        "{problem}"
    );
}

fn a_stray_golden_is_stale_and_named() {
    let dir = tempfile::tempdir().unwrap();
    copy_dir(Goldens::from_env().dir(), dir.path());
    fs::write(dir.path().join("static.complexity/cobol.json"), "{}\n").unwrap();

    let registries = Registries::shared();
    let problems = stale_goldens(
        &Goldens::new(dir.path(), GoldenMode::Compare),
        registries,
        &registries.fixture_pairs(),
    );

    assert_eq!(
        problems,
        vec![
            "golden/static.complexity/cobol.json is stale: no supported pair, check, or survey claims it"
                .to_owned()
        ]
    );
}

fn a_check_runs_on_its_own_fixture_repository_where_it_has_one() {
    let repos = tempfile::tempdir().unwrap();
    fs::create_dir_all(repos.path().join("x.own/rust")).unwrap();
    let (own, other) = (CheckId::from("x.own"), CheckId::from("x.other"));
    let (rust, go) = (LangId::from("rust"), LangId::from("go"));

    assert_eq!(fixture_repo(repos.path(), &own, &rust), "x.own/rust");
    assert_eq!(fixture_repo(repos.path(), &own, &go), "go");
    assert_eq!(fixture_repo(repos.path(), &other, &rust), "rust");
}
