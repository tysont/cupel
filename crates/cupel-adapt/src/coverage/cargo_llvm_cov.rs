// ABOUTME: The `cargo llvm-cov` coverage producer for Rust: the configured command (`cargo llvm-cov --lcov
// ABOUTME: --output-path {out}`), with doctests on a nightly toolchain; its LCOV names files by absolute path.
use std::path::Path;
use std::sync::OnceLock;

use cupel_core::coverage::CoverageMap;
use cupel_core::{Cancellation, Process};

use super::tool::{CoverageSpec, CoverageTool, as_written, every_file};

const ID: &str = "tool.cargo-llvm-cov";

static SPEC: CoverageSpec = CoverageSpec {
    id: ID,
    langs: &["rust"],
    tool: "cargo-llvm-cov",
    node_bin: false,
    version: &["cargo", "llvm-cov", "--version"],
    default_report: None,
    resolve: as_written,
    listing: every_file,
    providers: &[],
    uses: None,
    prepare: None,
    examples: Some(with_doctests),
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<CoverageMap, _>(CoverageTool(&SPEC)),
});

/// The command that also runs doctests (ADR-231): the configured one where it passes
/// `--doctests` itself, else, where a nightly toolchain with llvm-tools is installed, the
/// configured `cargo llvm-cov` command under it with `--doctests`; `None` otherwise.
fn with_doctests(command: &str, root: &Path) -> Option<String> {
    doctest_command(command, || nightly_coverage(root))
}

/// [`with_doctests`] with `nightly` saying whether a nightly toolchain can measure coverage. A
/// command other than `cargo llvm-cov` with options (a subcommand such as `nextest`, a
/// toolchain of its own) is left as configured.
fn doctest_command(command: &str, nightly: impl FnOnce() -> bool) -> Option<String> {
    let words: Vec<&str> = command.split_whitespace().collect();
    if words.contains(&"--doctests") {
        return Some(command.to_owned());
    }
    let options = command.trim_start().strip_prefix("cargo llvm-cov")?;
    let plain = options.is_empty() || options.starts_with(char::is_whitespace);
    let subcommand = words.get(2).is_some_and(|word| !word.starts_with('-'));
    (plain && !subcommand && nightly())
        .then(|| format!("cargo +nightly llvm-cov --doctests{options}"))
}

/// Whether `rustup` has the `nightly` toolchain with its llvm-tools component, asked once per
/// process in `root` without the network: `rustup toolchain list` and `rustup component list
/// --toolchain nightly --installed`. A dated nightly alone (`nightly-2026-03-26-…`) is not
/// `+nightly`.
pub fn nightly_coverage(root: &Path) -> bool {
    static ASKED: OnceLock<bool> = OnceLock::new();
    *ASKED.get_or_init(|| {
        let listed = |args: &[&str]| {
            Process::new("rustup", root)
                .args(args.iter().map(|arg| (*arg).to_owned()))
                .run(&Cancellation::new())
                .ok()
                .filter(|captured| captured.exit_code == Some(0))
                .map(|captured| captured.stdout)
                .unwrap_or_default()
        };
        let nightly = listed(&["toolchain", "list"]).lines().any(|line| {
            line.strip_prefix("nightly-")
                .is_some_and(|host| !host.starts_with(|c: char| c.is_ascii_digit()))
        });
        nightly
            && listed(&["component", "list", "--toolchain", "nightly", "--installed"])
                .lines()
                .any(|line| line.starts_with("llvm-tools"))
    })
}

#[cfg(test)]
mod tests {
    use super::doctest_command;

    const DEFAULT: &str = "cargo llvm-cov --lcov --output-path {out}";

    #[test]
    fn a_nightly_toolchain_runs_the_default_command_with_doctests_under_it() {
        assert_eq!(
            [
                doctest_command(DEFAULT, || true),
                doctest_command(DEFAULT, || false),
            ],
            [
                Some("cargo +nightly llvm-cov --doctests --lcov --output-path {out}".to_owned()),
                None,
            ]
        );
    }

    #[test]
    fn a_command_with_doctests_runs_as_configured_and_another_command_without_them() {
        let doctests = "cargo +nightly llvm-cov --doctests --lcov --output-path {out}";
        assert_eq!(
            [
                doctest_command(doctests, || false),
                doctest_command("cargo llvm-cov nextest --lcov --output-path {out}", || true),
                doctest_command("cargo +beta llvm-cov --lcov --output-path {out}", || true),
                doctest_command("cargo llvm-covx --lcov", || true),
            ],
            [Some(doctests.to_owned()), None, None, None]
        );
    }
}
