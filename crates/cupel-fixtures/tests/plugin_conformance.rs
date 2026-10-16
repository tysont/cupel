// ABOUTME: The protocol conformance suite (ADR-092 §2): the Rust, Python, and TypeScript reference plugins, driven
// ABOUTME: through cupel-plugin's host as the CLI drives them, must describe, plan, produce, and report identically.
use std::path::Path;

use cupel_core::config::PluginEntry;
use cupel_core::{
    Cancellation, CheckCtx, CheckRegistry, CheckStatus, CostClass, CoveredLines, Ctx, Error, Event,
    FactKind, Facts, Lang, LangFamily, LangId, PerTestMap, Plan, ProduceCtx, ProducerId,
    ProducerRegistry, RunId, Scope, TestCoverage, Writer, find_program,
};
use cupel_fixtures::copy_repo;
use cupel_plugin::{Discovery, Plugins, discover};
use serde_json::json;

/// One language's pair of reference plugins, as `[plugins]` would run them.
struct Reference {
    lang: &'static str,
    check: Vec<String>,
    produce: Vec<String>,
}

fn plugin_file(path: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("plugins")
        .join(path)
        .display()
        .to_string()
}

/// Whether `node` runs TypeScript by stripping types: from 22.18 and 23.6 on.
fn node_strips_types() -> bool {
    let Ok(output) = std::process::Command::new("node").arg("--version").output() else {
        return false;
    };
    let version = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<u32> = version
        .trim()
        .trim_start_matches('v')
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect();
    match parts.as_slice() {
        [22, minor, ..] => *minor >= 18,
        [23, minor, ..] => *minor >= 6,
        [major, ..] => *major > 23,
        [] => false,
    }
}

/// Every reference implementation whose interpreter is installed; the others say they were
/// skipped.
fn references() -> Vec<Reference> {
    let mut references = vec![Reference {
        lang: "rust",
        check: vec![env!("CARGO_BIN_EXE_cupel-check-ref-rust").to_owned()],
        produce: vec![env!("CARGO_BIN_EXE_cupel-produce-ref-rust").to_owned()],
    }];
    if find_program("python3").is_some() {
        references.push(Reference {
            lang: "python",
            check: vec!["python3".to_owned(), plugin_file("python/check.py")],
            produce: vec!["python3".to_owned(), plugin_file("python/produce.py")],
        });
    } else {
        eprintln!("SKIPPED: python3 not installed; the Python reference plugins did not run");
    }
    if node_strips_types() {
        references.push(Reference {
            lang: "typescript",
            check: vec!["node".to_owned(), plugin_file("typescript/check.mts")],
            produce: vec!["node".to_owned(), plugin_file("typescript/produce.mts")],
        });
    } else {
        eprintln!(
            "SKIPPED: node 22.18 or later not installed; the TypeScript reference plugins did not run"
        );
    }
    references
}

/// A copy of the plugin fixture: TypeScript sources and the committed per-test LCOV file.
fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("ref.untested-python/typescript", dir.path());
    dir
}

fn discovered(reference: &Reference, root: &Path) -> Plugins {
    let entries = [("check", &reference.check), ("produce", &reference.produce)]
        .into_iter()
        .map(|(name, command)| (name.to_owned(), PluginEntry::Command(command.clone())))
        .collect();
    let languages = ["javascript", "python", "typescript"].map(LangId::from);
    discover(&Discovery {
        root,
        entries: &entries,
        path: None,
        languages: &languages,
        taken: Vec::new(),
        cancel: &Cancellation::new(),
    })
    .unwrap_or_else(|error| panic!("{}: {error}", reference.lang))
}

fn lang(id: &str, family: &str) -> Lang {
    Lang {
        id: LangId::from(id),
        family: LangFamily::from(family),
    }
}

/// The map the committed LCOV file describes.
fn expected_map() -> PerTestMap {
    let covered = |file: &str, lines: &[u32]| CoveredLines {
        file: file.to_owned(),
        lines: lines.to_vec(),
    };
    PerTestMap::new(
        ProducerId::from("tool.vitest"),
        vec![
            TestCoverage::new(
                "test/ledger.test.ts > total > adds the entries",
                None,
                vec![covered("src/ledger.ts", &[1, 2, 3])],
            ),
            TestCoverage::new(
                "test/format.test.ts > pad > pads on the left",
                None,
                vec![covered("src/format.ts", &[1, 2, 3])],
            ),
            TestCoverage::new(
                "test/format.test.ts > pad > leaves long text alone",
                None,
                vec![covered("src/format.ts", &[1, 2])],
            ),
        ],
    )
}

fn produce(
    reference: &Reference,
    root: &Path,
    config: serde_json::Map<String, serde_json::Value>,
) -> cupel_core::Result<PerTestMap> {
    let mut producers = ProducerRegistry::default();
    discovered(reference, root).register_producers(&mut producers);
    let producer = producers
        .producers_for(FactKind::PerTestMap, &lang("typescript", "ecma"))
        .next()
        .unwrap();
    let mut ctx = ProduceCtx::new(
        Ctx::new(root.to_path_buf()),
        Writer::new(root, false).unwrap().confined(),
        Cancellation::new(),
    );
    ctx.config = config;
    producer
        .produce(&ctx, &Scope::of_files(Vec::new()), &mut Vec::new())
        .map(|made| made.downcast::<PerTestMap>().unwrap())
}

/// Runs `reference`'s check over `files` with the expected map, returning its plan, result,
/// and the findings it reported.
fn check(
    reference: &Reference,
    root: &Path,
    files: &[&str],
) -> (Plan, cupel_core::CheckResult, Vec<String>) {
    let mut checks = CheckRegistry::default();
    discovered(reference, root).register_checks(&mut checks);
    let check = checks.iter().next().unwrap();
    let scope = Scope::of_files(files.iter().map(|file| (*file).to_owned()).collect());
    let ctx = CheckCtx {
        scope: scope.clone(),
        config: serde_json::Map::new(),
        writer: Writer::new(root, false)
            .unwrap()
            .scratch(&RunId::generate()),
        cancel: Cancellation::new(),
    };
    let mut facts = Facts::for_manifest(check.manifest());
    facts.insert(expected_map()).unwrap();
    let plan = check.plan(&ctx, &scope).unwrap();
    let mut events = Vec::new();
    let result = check.run(&ctx, &facts, &plan, &mut events).unwrap();
    let findings = events
        .iter()
        .filter_map(|event| match event {
            Event::Finding(finding) => {
                Some(format!("{} {} {}", finding.id, finding.kind, finding.title))
            }
            _ => None,
        })
        .collect();
    (plan, result, findings)
}

#[test]
fn every_reference_plugin_describes_a_manifest_the_host_admits() {
    let root = repository();
    for reference in references() {
        let plugins = discovered(&reference, root.path());
        assert_eq!(
            plugins.check_ids(),
            [cupel_core::CheckId::from(format!(
                "ref.untested-{}",
                reference.lang
            ))]
        );
        let mut producers = ProducerRegistry::default();
        plugins.register_producers(&mut producers);
        let ids = |lang: &Lang| -> Vec<String> {
            producers
                .producers_for(FactKind::PerTestMap, lang)
                .map(|producer| producer.id().to_string())
                .collect()
        };
        let id = format!("plugin.ref-{}", reference.lang);
        assert_eq!(ids(&lang("typescript", "ecma")), std::slice::from_ref(&id));
        assert_eq!(ids(&lang("javascript", "ecma")), [id]);
        assert!(ids(&lang("python", "python")).is_empty());
    }
}

#[test]
fn every_reference_producer_reads_the_per_test_lcov_into_the_same_map() {
    let root = repository();
    for reference in references() {
        let map = produce(&reference, root.path(), serde_json::Map::new())
            .unwrap_or_else(|error| panic!("{}: {error}", reference.lang));
        assert_eq!(map, expected_map(), "{}", reference.lang);
    }
}

#[test]
fn a_reference_producer_without_its_lcov_file_fails_with_its_reason() {
    let root = repository();
    let config = serde_json::Map::from_iter([("per_test_lcov".to_owned(), json!("missing.lcov"))]);
    for reference in references() {
        match produce(&reference, root.path(), config.clone()) {
            Err(Error::Plugin { problem, .. }) => assert!(
                problem.contains("exited with code 1 without a fact")
                    && problem.contains("cannot read the per-test LCOV file"),
                "{}: {problem}",
                reference.lang
            ),
            other => panic!("{}: expected a plugin error, got {other:?}", reference.lang),
        }
    }
}

#[test]
fn every_reference_check_reports_the_files_no_test_executes() {
    let root = repository();
    let files = ["src/format.ts", "src/ledger.ts", "src/report.ts"];
    for reference in references() {
        let (plan, result, findings) = check(&reference, root.path(), &files);
        let lang = reference.lang;
        assert_eq!(
            plan,
            Plan {
                targets: 3,
                est_ms: 3
            },
            "{lang}"
        );
        assert_eq!(
            (result.id.to_string(), result.status, result.findings),
            (format!("ref.untested-{lang}"), CheckStatus::Fail, 1),
            "{lang}"
        );
        assert_eq!(result.metrics.get("untested").copied(), Some(1.0), "{lang}");
        let score = result.score.unwrap();
        assert_eq!(
            (score.unit.as_str(), score.n),
            ("tested_files", 3),
            "{lang}"
        );
        assert!(
            (score.value - 2.0 / 3.0).abs() < 1e-9,
            "{lang}: {}",
            score.value
        );
        assert_eq!(findings.len(), 1, "{lang}");
        assert!(
            findings[0].ends_with(" ref.untested-file No test executes src/report.ts"),
            "{lang}: {findings:?}"
        );
        assert_eq!(result.cost, CostClass::Quick);
    }
}

#[test]
fn every_reference_check_derives_one_finding_id_per_check_and_file() {
    let root = repository();
    let ids: Vec<(String, String)> = references()
        .iter()
        .map(|reference| {
            let (_, _, findings) = check(reference, root.path(), &["src/report.ts"]);
            let id = findings[0].split(' ').next().unwrap().to_owned();
            (reference.lang.to_owned(), id)
        })
        .collect();
    let expected = |check: &str| {
        let digest = format!("{check}\0ref.untested-file\0src/report.ts")
            .bytes()
            .fold(0x811c_9dc5_u32, |digest, byte| {
                (digest ^ u32::from(byte)).wrapping_mul(0x0100_0193)
            });
        format!("fnd_{digest:08x}")[..10].to_owned()
    };
    for (lang, id) in ids {
        assert_eq!(id, expected(&format!("ref.untested-{lang}")), "{lang}");
    }
}

#[test]
fn a_reference_check_passes_when_every_file_in_scope_is_tested() {
    let root = repository();
    for reference in references() {
        let (_, result, findings) =
            check(&reference, root.path(), &["src/ledger.ts", "src/format.ts"]);
        assert_eq!(
            (result.status, findings.len()),
            (CheckStatus::Pass, 0),
            "{}",
            reference.lang
        );
        assert_eq!(result.metrics.get("untested").copied(), Some(0.0));
    }
}
