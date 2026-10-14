// ABOUTME: The `doctor` verb: checks configuration, `required_version`, write access to `.cupel/`, and
// ABOUTME: the tools the configuration needs; one finding per problem, exit 3 for any `fail` (ADR-022).
use std::path::Path;
use std::process::{Command, Stdio};

use cupel_core::config::Config;
use cupel_core::syntax::{ProjectInfo, ProjectRoot};
use cupel_core::{ActionKind, Envelope, ExitCode, LangId, Severity, Status, ToolId, Writer};
use semver::{Version, VersionReq};
use serde_json::json;

use crate::error::Usage;
use crate::problem::Problem;
use crate::project;
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
    let exclude = config
        .ok()
        .map(|config| config.file.scope.exclude.clone())
        .unwrap_or_default();
    let info = project::detect(writer, &exclude);
    problems.extend(tools(
        &requirements(config.ok(), writer.root(), &info),
        installed,
    ));
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

/// The tools to check: `git`, which `--changed` needs, then the programs the languages' commands
/// run, for the configuration (when it loaded) and the project roots `info` of the repository at
/// `repo`.
fn requirements(config: Option<&Config>, repo: &Path, info: &ProjectInfo) -> Vec<Requirement> {
    let git = Requirement {
        tool: ToolId::from("git"),
        needed_for: "--changed".to_owned(),
        required: false,
        probe: vec!["git".to_owned(), "--version".to_owned()],
    };
    std::iter::once(git)
        .chain(toolchain(config, repo, info))
        .collect()
}

/// The programs of every command a language would use (ADR-038): each configured
/// `[lang.<id>]` `test`, `coverage`, and `bench`, and for keys left unset, the command `init`
/// would choose for the language's first detected root, named with the toolchain producer it
/// comes from. One requirement per program, in language order. None is required until P3's
/// checks run these commands.
fn toolchain(config: Option<&Config>, repo: &Path, info: &ProjectInfo) -> Vec<Requirement> {
    let configured = |lang: &LangId, key: &str| {
        config
            .and_then(|config| config.file.lang.get(lang))
            .and_then(|table| table.get(key))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    let mut langs: Vec<&LangId> = info.roots.iter().map(|root| &root.lang).collect();
    if let Some(config) = config {
        langs.extend(config.file.lang.keys());
    }
    langs.sort();
    langs.dedup();
    let mut uses: Vec<(String, Vec<String>)> = Vec::new();
    for lang in langs {
        let root = info.roots.iter().find(|root| root.lang == *lang);
        for (key, command, producer) in commands(repo, root) {
            let (command, purpose) = match configured(lang, key) {
                Some(command) => (command, format!("[lang.{lang}] {key}")),
                None => match (command, producer) {
                    (Some(command), Some(producer)) => {
                        (command, format!("[lang.{lang}] {key} ({producer})"))
                    }
                    _ => continue,
                },
            };
            for program in programs(&command) {
                match uses.iter_mut().find(|(known, _)| *known == program) {
                    Some((_, needed_for)) => needed_for.push(purpose.clone()),
                    None => uses.push((program, vec![purpose.clone()])),
                }
            }
        }
    }
    uses.into_iter()
        .map(|(program, needed_for)| Requirement {
            probe: cupel_adapt::toolchain::probe(&program),
            tool: ToolId::from(program.as_str()),
            needed_for: needed_for.join(", "),
            required: false,
        })
        .collect()
}

/// The `test`, `coverage`, and `bench` commands the toolchain table chooses for `root`, each
/// with the producer it comes from; all absent without a root.
fn commands(
    repo: &Path,
    root: Option<&ProjectRoot>,
) -> [(&'static str, Option<String>, Option<String>); 3] {
    let selection = root.and_then(|root| project::selection(repo, root));
    let commands = selection
        .as_ref()
        .map(cupel_adapt::toolchain::Selection::commands)
        .unwrap_or_default();
    let producer =
        |tool: Option<&cupel_adapt::toolchain::Tool>| tool.map(|tool| tool.id.to_string());
    let (runner, coverage, bench) = selection.as_ref().map_or((None, None, None), |selection| {
        (
            producer(selection.runner),
            producer(selection.coverage),
            producer(selection.bench),
        )
    });
    [
        ("test", commands.test, runner),
        ("coverage", commands.coverage, coverage),
        ("bench", commands.bench, bench),
    ]
}

/// The programs `command` runs: the first word of each `&&`-separated part that is not an
/// environment assignment (`FOO=1`), in order, without repeats.
fn programs(command: &str) -> Vec<String> {
    let mut programs: Vec<String> = Vec::new();
    for part in command.split("&&") {
        let program = part.split_whitespace().find(|word| !word.contains('='));
        if let Some(program) = program.filter(|program| !programs.iter().any(|p| p == program)) {
            programs.push(program.to_owned());
        }
    }
    programs
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

    /// `cupel.toml` holding `text`, loaded from a fresh directory.
    fn config(text: &str) -> Config {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(cupel_core::config::FILE_NAME), text).unwrap();
        Config::load(&cupel_core::config::ConfigRequest {
            root: dir.path().to_path_buf(),
            ..cupel_core::config::ConfigRequest::default()
        })
        .unwrap()
    }

    /// Each requirement as `(tool, needed_for, required, probe)`.
    fn shape(requirements: &[Requirement]) -> Vec<(String, String, bool, String)> {
        requirements
            .iter()
            .map(|requirement| {
                (
                    requirement.tool.to_string(),
                    requirement.needed_for.clone(),
                    requirement.required,
                    requirement.probe.join(" "),
                )
            })
            .collect()
    }

    #[test]
    fn programs_are_the_first_word_of_each_part_after_any_assignments() {
        assert_eq!(programs("cargo test --workspace"), ["cargo"]);
        assert_eq!(
            programs("python -m coverage run -m unittest && python -m coverage lcov -o {out}"),
            ["python"]
        );
        assert_eq!(programs("FOO=1 BAR=2 go test && node x"), ["go", "node"]);
        assert!(programs("  ").is_empty());
    }

    #[test]
    fn without_configured_or_detected_languages_only_git_is_checked() {
        let repo = tempfile::tempdir().unwrap();
        let names: Vec<String> = requirements(None, repo.path(), &ProjectInfo::default())
            .iter()
            .map(|requirement| requirement.tool.to_string())
            .collect();
        assert_eq!(names, ["git"]);
    }

    #[test]
    fn configured_commands_and_detected_roots_defaults_name_their_programs() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cupel-fixtures/repos/detect");
        let info = ProjectInfo {
            roots: vec![ProjectRoot {
                path: "cargo-workspace".to_owned(),
                lang: LangId::from("rust"),
                build_files: Vec::new(),
                members: vec!["crates/engine".to_owned()],
                packages: Vec::new(),
                skip_dirs: Vec::new(),
            }],
        };
        let config = config(
            "[lang.python]\ntest = \"poetry run pytest\"\n\
             coverage = \"FOO=1 poetry run coverage run -m pytest && go tool cover\"\n",
        );
        let requirements = requirements(Some(&config), &repo, &info);
        assert_eq!(
            shape(&requirements[1..]),
            [
                (
                    "poetry".to_owned(),
                    "[lang.python] test, [lang.python] coverage".to_owned(),
                    false,
                    "poetry --version".to_owned()
                ),
                (
                    "go".to_owned(),
                    "[lang.python] coverage".to_owned(),
                    false,
                    "go version".to_owned()
                ),
                (
                    "cargo".to_owned(),
                    "[lang.rust] test (tool.cargo-nextest), [lang.rust] coverage \
                     (tool.cargo-llvm-cov), [lang.rust] bench (tool.criterion)"
                        .to_owned(),
                    false,
                    "cargo --version".to_owned()
                ),
            ]
        );
    }

    #[test]
    fn a_configured_command_replaces_the_default_and_names_no_producer() {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../cupel-fixtures/repos/detect");
        let info = ProjectInfo {
            roots: vec![ProjectRoot {
                path: "cargo-single".to_owned(),
                lang: LangId::from("rust"),
                build_files: Vec::new(),
                members: Vec::new(),
                packages: Vec::new(),
                skip_dirs: Vec::new(),
            }],
        };
        let config = config("[lang.rust]\ntest = \"just test\"\n");
        let requirements = requirements(Some(&config), &repo, &info);
        assert_eq!(
            shape(&requirements[1..])
                .into_iter()
                .map(|(tool, needed_for, _, _)| (tool, needed_for))
                .collect::<Vec<_>>(),
            [
                ("just".to_owned(), "[lang.rust] test".to_owned()),
                (
                    "cargo".to_owned(),
                    "[lang.rust] coverage (tool.cargo-llvm-cov)".to_owned()
                ),
            ]
        );
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
