// ABOUTME: The `doctor` verb: checks configuration, `required_version`, write access to `.cupel/`, and
// ABOUTME: the tools the configuration needs; one finding per problem, exit 3 for any `fail` (ADR-022).
use std::process::{Command, Stdio};

use cupel_core::config::Config;
use cupel_core::{ActionKind, Envelope, ExitCode, Severity, Status, ToolId, Writer};
use semver::{Version, VersionReq};
use serde_json::json;

use crate::error::Usage;
use crate::problem::Problem;
use crate::run::Run;

/// The configuration key `required_version` findings are about.
const REQUIRED_VERSION: &str = "required_version";

/// A tool something needs, and how to tell whether it is installed.
pub struct Requirement {
    pub tool: ToolId,
    /// What needs it, such as `--changed`.
    pub needed_for: String,
    /// Whether the configured checks cannot run without it.
    pub required: bool,
    /// A command that exits 0 when the tool is installed.
    pub probe: Vec<String>,
}

/// Runs every check. `config` is the loaded configuration, or why it did not load.
pub fn run(
    run: &Run,
    config: Result<&Config, &cupel_core::Error>,
    writer: &Writer,
    fix: bool,
) -> Envelope {
    let mut problems = Vec::new();
    match config {
        Ok(config) => problems.extend(version(
            config.file.required_version.as_deref(),
            &own_version(),
        )),
        Err(error) => {
            problems.push(Usage::from_core(error).problem("doctor", "doctor.config-invalid"));
        }
    }
    problems.extend(cupel_dir(writer));
    problems.extend(tools(&requirements(config.ok()), installed));
    if fix {
        problems.push(fix_pending());
    }
    let required_missing = problems
        .iter()
        .any(|problem| problem.severity == Severity::Fail);
    let findings = problems
        .into_iter()
        .map(|problem| problem.finding(&run.repro()))
        .collect();
    if required_missing {
        run.reported(Status::Error, ExitCode::Environment, findings)
    } else {
        run.reported(Status::Pass, ExitCode::Pass, findings)
    }
}

/// This binary's version.
fn own_version() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("the crate version is semver")
}

/// The problem with `required` (a Cargo version requirement) for a binary at `version`.
fn version(required: Option<&str>, version: &Version) -> Option<Problem> {
    let required = required?;
    let problem = |kind, title: String, action, fix: String| Problem {
        check: "doctor",
        kind,
        subject: REQUIRED_VERSION.to_owned(),
        severity: Severity::Fail,
        title,
        detail: Some("This binary's version is the envelope's `cupel.version`.".to_owned()),
        evidence: json!({"required": required}),
        action,
        fix,
    };
    match VersionReq::parse(required) {
        Ok(requirement) if requirement.matches(version) => None,
        Ok(_) => Some(problem(
            "doctor.version-unsatisfied",
            format!("This cupel does not satisfy required_version `{required}`"),
            ActionKind::InstallTool,
            format!("Install a cupel matching `{required}`, or change required_version."),
        )),
        Err(error) => Some(problem(
            "doctor.version-invalid",
            format!("required_version `{required}` is not a version requirement: {error}"),
            ActionKind::Configure,
            "Write required_version as a Cargo version requirement, such as \">=0.4\".".to_owned(),
        )),
    }
}

/// The problem with writing under `.cupel/`, if there is one.
fn cupel_dir(writer: &Writer) -> Option<Problem> {
    let error = writer.probe().err()?;
    Some(Problem {
        check: "doctor",
        kind: "doctor.cupel-unwritable",
        subject: Writer::DIR.to_owned(),
        severity: Severity::Fail,
        title: "Cupel cannot write to .cupel/".to_owned(),
        detail: Some(
            "Run records, caches, and scratch files live in .cupel/; every run needs it."
                .to_owned(),
        ),
        evidence: json!({"error": error.to_string()}),
        action: ActionKind::Configure,
        fix: "Make .cupel/ a writable directory in the repository root.".to_owned(),
    })
}

/// The tools to check: `git`, which `--changed` needs, then what the configuration's toolchain
/// needs.
fn requirements(config: Option<&Config>) -> Vec<Requirement> {
    let git = Requirement {
        tool: ToolId::from("git"),
        needed_for: "--changed".to_owned(),
        required: false,
        probe: vec!["git".to_owned(), "--version".to_owned()],
    };
    std::iter::once(git)
        .chain(config.map(toolchain).unwrap_or_default())
        .collect()
}

/// The runner, coverage, and bench tools `config` selects. Empty until the toolchain table
/// (P1.9) and its tool producers (P3) exist.
fn toolchain(_config: &Config) -> Vec<Requirement> {
    Vec::new()
}

/// One problem per requirement `installed` says is missing: `fail` when required, else `warn`.
fn tools(requirements: &[Requirement], installed: impl Fn(&Requirement) -> bool) -> Vec<Problem> {
    requirements
        .iter()
        .filter(|requirement| !installed(requirement))
        .map(|requirement| Problem {
            check: "doctor",
            kind: "doctor.tool-missing",
            subject: requirement.tool.to_string(),
            severity: if requirement.required {
                Severity::Fail
            } else {
                Severity::Warn
            },
            title: format!(
                "`{}` is not installed; {} needs it",
                requirement.tool, requirement.needed_for
            ),
            detail: None,
            evidence: json!({
                "tool": requirement.tool,
                "needed_for": requirement.needed_for,
                "required": requirement.required,
                "probe": requirement.probe.join(" "),
            }),
            action: ActionKind::InstallTool,
            fix: format!("Install `{}` and put it on PATH.", requirement.tool),
        })
        .collect()
}

/// Whether `requirement`'s probe command runs and exits 0.
fn installed(requirement: &Requirement) -> bool {
    let Some((program, args)) = requirement.probe.split_first() else {
        return false;
    };
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// `--fix` parses, but installing arrives with the first tool producers.
fn fix_pending() -> Problem {
    Problem {
        check: "doctor",
        kind: "doctor.fix-pending",
        subject: "--fix".to_owned(),
        severity: Severity::Info,
        title: "--fix installs nothing yet".to_owned(),
        detail: Some("Installing tools is implemented in plan step P3.1.".to_owned()),
        evidence: json!({"step": "P3.1"}),
        action: ActionKind::Review,
        fix: "Install the tools named by doctor.tool-missing findings by hand.".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requirement(tool: &str, required: bool) -> Requirement {
        Requirement {
            tool: ToolId::from(tool),
            needed_for: "tests.coverage".to_owned(),
            required,
            probe: vec![format!("{tool}-is-not-a-real-program")],
        }
    }

    #[test]
    fn a_missing_required_tool_fails_and_an_optional_one_warns() {
        let requirements = [
            requirement("fake-cov", true),
            requirement("fake-bench", false),
        ];
        let problems = tools(&requirements, |_| false);
        let shape: Vec<(String, Severity, ActionKind)> = problems
            .iter()
            .map(|problem| (problem.subject.clone(), problem.severity, problem.action))
            .collect();
        assert_eq!(
            shape,
            [
                (
                    "fake-cov".to_owned(),
                    Severity::Fail,
                    ActionKind::InstallTool
                ),
                (
                    "fake-bench".to_owned(),
                    Severity::Warn,
                    ActionKind::InstallTool
                ),
            ]
        );
        assert_eq!(problems[0].evidence["needed_for"], "tests.coverage");
    }

    #[test]
    fn installed_tools_are_not_problems() {
        assert!(tools(&[requirement("fake-cov", true)], |_| true).is_empty());
    }

    #[test]
    fn a_probe_that_cannot_run_means_not_installed() {
        assert!(!installed(&requirement("fake-cov", true)));
        let empty = Requirement {
            probe: Vec::new(),
            ..requirement("fake-cov", true)
        };
        assert!(!installed(&empty));
    }

    #[test]
    fn the_toolchain_contributes_nothing_until_tool_producers_exist() {
        let names: Vec<String> = requirements(None)
            .iter()
            .map(|requirement| requirement.tool.to_string())
            .collect();
        assert_eq!(names, ["git"]);
    }

    #[test]
    fn version_requirements_follow_cargo_syntax() {
        let at = |v: &str| Version::parse(v).unwrap();
        let kind = |required: &str, binary: &str| {
            version(Some(required), &at(binary)).map(|problem| problem.kind)
        };
        assert_eq!(kind(">=0.4", "0.4.0"), None);
        assert_eq!(kind("^0.4", "0.4.9"), None);
        assert_eq!(kind("^0.4", "0.5.0"), Some("doctor.version-unsatisfied"));
        assert_eq!(
            kind(">=0.1, <0.2", "0.2.0"),
            Some("doctor.version-unsatisfied")
        );
        assert_eq!(
            kind("0.1", "0.1.7"),
            None,
            "a bare version is a caret requirement"
        );
        assert_eq!(kind("~0.1.2", "0.1.9"), None);
        assert_eq!(kind("whenever", "0.1.0"), Some("doctor.version-invalid"));
        assert!(version(None, &at("0.1.0")).is_none());
    }
}
