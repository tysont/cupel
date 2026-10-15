// ABOUTME: How the toolchain table says to install a tool producer's program (ADR-081): a command filled with
// ABOUTME: the root's package manager, a hint for toolchains, and one or the other for every registered producer.
use std::path::Path;

use cupel_core::{Ctx, FactKind, Install, Lang, LangFamily, LangId, ProducerId, ProducerRegistry};

/// What the table says installs `program` for the producer `id` in a fresh root holding `files`.
fn install(id: &str, program: &str, files: &[(&str, &str)]) -> Option<Install> {
    let root = tempfile::tempdir().unwrap();
    for (name, text) in files {
        std::fs::write(root.path().join(name), text).unwrap();
    }
    cupel_adapt::toolchain::install(&ProducerId::from(id), program, root.path())
}

/// A producer, its program, and the files of a root.
type Case<'a> = (&'a str, &'a str, &'a [(&'a str, &'a str)]);

/// What the table says installs each case.
fn installs(cases: &[Case<'_>]) -> Vec<Option<Install>> {
    cases
        .iter()
        .map(|(id, program, files)| install(id, program, files))
        .collect()
}

#[test]
fn commands_are_cargo_install_or_a_dev_dependency_through_the_detected_launcher() {
    let commands = installs(&[
        ("tool.cargo-nextest", "cargo-nextest", &[]),
        ("tool.cargo-llvm-cov", "cargo-llvm-cov", &[]),
        ("tool.pytest", "pytest", &[]),
        ("tool.pytest-cov", "pytest", &[("uv.lock", "")]),
        ("tool.coverage-py", "coverage", &[("poetry.lock", "")]),
        ("tool.vitest", "vitest", &[("package.json", "{}")]),
        ("tool.vitest-coverage", "vitest", &[("pnpm-lock.yaml", "")]),
        ("tool.jest", "jest", &[("yarn.lock", "")]),
    ]);
    assert_eq!(
        commands,
        [
            "cargo install cargo-nextest --locked",
            "cargo install cargo-llvm-cov --locked && rustup component add llvm-tools-preview",
            "python -m pip install pytest",
            "uv add --dev pytest-cov",
            "poetry add --group dev coverage",
            "npm install --save-dev vitest",
            "pnpm add --save-dev vitest @vitest/coverage-v8",
            "yarn add --dev jest",
        ]
        .map(|text| Some(Install::Command(text.to_owned())))
    );
}

#[test]
fn toolchains_get_a_hint_rather_than_a_command() {
    let hint = |text: &str| Some(Install::Hint(text.to_owned()));
    let go = hint(
        "go is the Go toolchain, which Cupel does not install: install Go from \
         https://go.dev/dl/ and put go on PATH.",
    );
    assert_eq!(
        installs(&[
            ("tool.go-test", "go", &[]),
            ("tool.go-coverprofile", "go", &[]),
            ("tool.cargo-test", "cargo", &[]),
            ("tool.unittest", "python3", &[]),
        ]),
        [
            go.clone(),
            go,
            hint(
                "cargo is part of the Rust toolchain, which Cupel does not install: install \
                 Rust with rustup (https://rustup.rs) and put cargo on PATH."
            ),
            hint(
                "python3 is the Python toolchain, which Cupel does not install: install \
                 Python 3 from https://www.python.org/downloads/ or a package manager and put \
                 python3 on PATH."
            ),
        ]
    );
}

#[test]
fn a_producer_the_table_does_not_list_has_no_install() {
    assert_eq!(install("tool.unknown", "unknown", &[]), None);
}

/// Every producer registered for `kind` in any language, with the program it runs.
fn producers_of(registry: &ProducerRegistry, kind: FactKind, ctx: &Ctx) -> Vec<(String, String)> {
    [
        ("rust", "rust"),
        ("go", "go"),
        ("python", "python"),
        ("javascript", "ecma"),
    ]
    .into_iter()
    .flat_map(|(id, family)| {
        let lang = Lang {
            id: LangId::from(id),
            family: LangFamily::from(family),
        };
        registry
            .producers_for(kind, &lang)
            .filter_map(|producer| {
                let tool = producer.tool(ctx)?;
                Some((producer.id().to_string(), tool.name.to_string()))
            })
            .collect::<Vec<_>>()
    })
    .collect()
}

#[test]
fn every_runner_and_coverage_producer_says_how_to_install_its_program() {
    let mut registry = ProducerRegistry::default();
    cupel_adapt::register(&mut registry);
    let ctx = Ctx::new(Path::new(".").to_path_buf());
    let producers: Vec<(String, String)> = [FactKind::TestResults, FactKind::CoverageMap]
        .into_iter()
        .flat_map(|kind| producers_of(&registry, kind, &ctx))
        .collect();
    assert!(producers.len() >= 14, "{producers:?}");
    let missing: Vec<&(String, String)> = producers
        .iter()
        .filter(|(id, program)| {
            cupel_adapt::toolchain::install(&ProducerId::from(id.as_str()), program, &ctx.root)
                .is_none()
        })
        .collect();
    assert!(missing.is_empty(), "{missing:?}");
}
