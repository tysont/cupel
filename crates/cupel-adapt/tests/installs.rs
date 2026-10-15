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

fn command(text: &str) -> Install {
    Install::Command(text.to_owned())
}

#[test]
fn cargo_tools_install_with_cargo_install_locked() {
    assert_eq!(
        install("tool.cargo-nextest", "cargo-nextest", &[]),
        Some(command("cargo install cargo-nextest --locked"))
    );
    assert_eq!(
        install("tool.cargo-llvm-cov", "cargo-llvm-cov", &[]),
        Some(command(
            "cargo install cargo-llvm-cov --locked && rustup component add llvm-tools-preview"
        ))
    );
}

#[test]
fn python_tools_install_through_the_detected_launcher() {
    assert_eq!(
        install("tool.pytest", "pytest", &[]),
        Some(command("python -m pip install pytest"))
    );
    assert_eq!(
        install("tool.pytest-cov", "pytest", &[("uv.lock", "")]),
        Some(command("uv add --dev pytest-cov"))
    );
    assert_eq!(
        install("tool.coverage-py", "coverage", &[("poetry.lock", "")]),
        Some(command("poetry add --group dev coverage"))
    );
}

#[test]
fn javascript_tools_are_dev_dependencies_of_the_root() {
    assert_eq!(
        install("tool.vitest", "vitest", &[("package.json", "{}")]),
        Some(command("npm install --save-dev vitest"))
    );
    assert_eq!(
        install("tool.vitest-coverage", "vitest", &[("pnpm-lock.yaml", "")]),
        Some(command("pnpm add --save-dev vitest @vitest/coverage-v8"))
    );
    assert_eq!(
        install("tool.jest", "jest", &[("yarn.lock", "")]),
        Some(command("yarn add --dev jest"))
    );
}

#[test]
fn toolchains_get_a_hint_rather_than_a_command() {
    for (id, program) in [
        ("tool.go-test", "go"),
        ("tool.go-coverprofile", "go"),
        ("tool.cargo-test", "cargo"),
        ("tool.unittest", "python3"),
    ] {
        match install(id, program, &[]) {
            Some(Install::Hint(hint)) => {
                assert!(hint.contains("toolchain"), "{id}: {hint}");
            }
            other => panic!("{id}: {other:?}"),
        }
    }
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
