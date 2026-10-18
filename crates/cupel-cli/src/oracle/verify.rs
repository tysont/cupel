// ABOUTME: `oracle verify` (ADR-090 §3): each oracle's smoke comparison — a ref resolves, a command answers each
// ABOUTME: known input the same way twice, a spec parses — recorded on success and revoked on failure.
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use cupel_core::config::{Config, OracleConfig, OracleKind};
use cupel_core::{
    ActionKind, Cancellation, Envelope, ExitCode, Finding, Process, Severity, Status, find_program,
    oracle_identity,
};
use cupel_store::Store;
use cupel_store::oracles::VerifiedOracle;
use serde_json::{Value, json};

use super::{Unreachable, report, resolve};
use crate::error::CupelError;
use crate::persist;
use crate::problem::Problem;
use crate::run::{Invocation, shell_quote};

/// How a spec language's own parser is run on a file: the program, the arguments before the
/// file (ADR-090 §3), and whether it is a JVM that needs a temporary directory of its own
/// (SANY deletes the standard modules it extracted there on exit, so parsers running at once
/// in the shared one delete each other's).
const PARSERS: [(&str, &str, &[&str], bool); 3] = [
    ("quint", "quint", &["parse"], false),
    ("tla+", "sany", &[], true),
    ("p", "p", &["compile", "--pfiles"], false),
];

/// Tells one parser run's temporary directory from another's in this process.
static PARSER_RUNS: AtomicU64 = AtomicU64::new(0);

/// The exit code `sh` reports when the command's program is not found.
const NOT_FOUND: i32 = 127;

/// Why a smoke comparison did not verify an oracle.
enum Failure {
    /// The oracle cannot be reached: exit 3.
    Unreachable(Unreachable),
    /// The oracle answered, but not as an oracle must: exit 1.
    Disagreed { title: String, evidence: Value },
}

/// Verifies each of `names`, recording each verified oracle and forgetting each that failed.
pub(super) fn run(
    invocation: &Invocation<'_>,
    config: &Config,
    names: &[&String],
) -> Result<Envelope, CupelError> {
    let (run, root) = (invocation.run, invocation.writer.root());
    let store = persist::open(invocation.writer)?;
    let mut failures = Vec::new();
    for name in names {
        let definition = &config.file.oracles[*name];
        let outcome = smoke(root, run.cancellation(), definition).map(|resolved| VerifiedOracle {
            name: (*name).clone(),
            definition: definition.clone(),
            verified_at: run.started(),
            hash: oracle_identity(definition, resolved.as_deref()),
            run: run.id(),
        });
        keep(&store, name, outcome.as_ref().ok())?;
        if let Err(failure) = outcome {
            failures.push((*name, failure));
        }
    }
    let reports = names
        .iter()
        .map(|name| report(root, Some(&store), name, &config.file.oracles[*name]))
        .collect::<Result<_, _>>()?;
    let unreachable = failures
        .iter()
        .any(|(_, failure)| matches!(failure, Failure::Unreachable(_)));
    let findings: Vec<Finding> = failures
        .into_iter()
        .map(|(name, failure)| problem(name, failure).finding(&run.repro()))
        .collect();
    let (status, exit_code) = match (unreachable, findings.is_empty()) {
        (true, _) => (Status::Error, ExitCode::Environment),
        (false, false) => (Status::Fail, ExitCode::Fail),
        (false, true) => (Status::Pass, ExitCode::Pass),
    };
    let mut envelope = run.reported(status, exit_code, findings);
    envelope.oracles = Some(reports);
    Ok(envelope)
}

/// Records `verified`, or forgets the oracle `name` when it was not verified.
fn keep(store: &Store, name: &str, verified: Option<&VerifiedOracle>) -> Result<(), CupelError> {
    match verified {
        Some(verified) => store.record_oracle(verified),
        None => store.forget_oracle(name),
    }
    .map_err(|error| persist::unusable(&error))
}

/// The smoke comparison of `definition`: what it resolves to when it passes.
fn smoke(
    root: &Path,
    cancel: &Cancellation,
    definition: &OracleConfig,
) -> Result<Option<String>, Failure> {
    let resolved = resolve(root, definition).map_err(Failure::Unreachable)?;
    let given = |value: &Option<String>| value.clone().unwrap_or_default();
    match definition.kind {
        OracleKind::Ref => Ok(()),
        OracleKind::Command => {
            answers_alike(root, cancel, &given(&definition.cmd), &definition.inputs)
        }
        OracleKind::Spec => parses(
            root,
            cancel,
            &given(&definition.spec),
            &given(&definition.lang),
        ),
    }?;
    Ok(resolved)
}

/// Runs `cmd` twice on each of `inputs` (one empty input when there are none): every run must
/// exit 0, and the two runs on an input must print the same.
fn answers_alike(
    root: &Path,
    cancel: &Cancellation,
    cmd: &str,
    inputs: &[String],
) -> Result<(), Failure> {
    let empty = [String::new()];
    let inputs = if inputs.is_empty() {
        &empty[..]
    } else {
        inputs
    };
    for input in inputs {
        let line = cmd.replace("{input}", &shell_quote(input));
        let first = answer(root, cancel, &line, input)?;
        let second = answer(root, cancel, &line, input)?;
        if first != second {
            return Err(Failure::Disagreed {
                title: format!(
                    "the command printed different output twice for {}",
                    shown(input)
                ),
                evidence: json!({"input": input, "outputs": [first, second]}),
            });
        }
    }
    Ok(())
}

/// What `line` prints on standard output when it succeeds.
fn answer(root: &Path, cancel: &Cancellation, line: &str, input: &str) -> Result<String, Failure> {
    let ran = Process::new("sh", root)
        .args(["-c", line])
        .run(cancel)
        .map_err(|error| {
            Failure::Unreachable(Unreachable {
                title: format!("the command could not run: {error}"),
                action: ActionKind::InstallTool,
                fix: "Install `sh`, which runs oracle commands.".to_owned(),
            })
        })?;
    match ran.exit_code {
        Some(0) => Ok(ran.stdout),
        Some(NOT_FOUND) => Err(Failure::Unreachable(Unreachable {
            title: format!(
                "the command's program is not installed: {}",
                ran.stderr.trim()
            ),
            action: ActionKind::InstallTool,
            fix: "Install the program the command runs, or fix `cmd`.".to_owned(),
        })),
        exit_code => Err(Failure::Disagreed {
            title: format!("the command failed on {}", shown(input)),
            evidence: json!({"input": input, "exit_code": exit_code, "output": ran.stderr}),
        }),
    }
}

/// An input as a title names it.
fn shown(input: &str) -> String {
    if input.is_empty() {
        "the empty input".to_owned()
    } else {
        format!("the input `{input}`")
    }
}

/// Runs the parser of `lang` on the spec file `spec`, which must accept it.
fn parses(root: &Path, cancel: &Cancellation, spec: &str, lang: &str) -> Result<(), Failure> {
    let (_, program, args, jvm) = PARSERS
        .iter()
        .find(|(name, _, _, _)| *name == lang)
        .expect("a spec oracle's language is validated when configuration loads");
    if find_program(program).is_none() {
        return Err(Failure::Unreachable(Unreachable {
            title: format!("`{program}`, the {lang} parser, is not installed"),
            action: ActionKind::InstallTool,
            fix: format!("Install `{program}` and put it on PATH."),
        }));
    }
    let tmp = jvm.then(parser_tmpdir).flatten();
    let ran = parser(root, program, args, spec, tmp.as_deref())
        .run(cancel)
        .map_err(|error| {
            Failure::Unreachable(Unreachable {
                title: format!("`{program}` could not run: {error}"),
                action: ActionKind::InstallTool,
                fix: format!("Reinstall `{program}`."),
            })
        });
    if let Some(tmp) = &tmp {
        let _ = std::fs::remove_dir_all(tmp);
    }
    let ran = ran?;
    if ran.exit_code == Some(0) {
        return Ok(());
    }
    Err(Failure::Disagreed {
        title: format!("`{program}` does not accept the spec file `{spec}`"),
        evidence: json!({"spec": spec, "exit_code": ran.exit_code, "output": ran.stdout}),
    })
}

/// `program` on `spec` in `root` with `args` before it, output merged; a JVM parser's
/// temporary directory is `tmp`.
fn parser(root: &Path, program: &str, args: &[&str], spec: &str, tmp: Option<&Path>) -> Process {
    let process = Process::new(program, root)
        .args(args.iter().copied().chain([spec]))
        .merged();
    match tmp {
        Some(tmp) => process.java_tmpdir(tmp),
        None => process,
    }
}

/// A temporary directory of this parser run's own, created; `None` when it cannot be, in
/// which case the parser runs in the shared one.
fn parser_tmpdir() -> Option<PathBuf> {
    let tmp = std::env::temp_dir().join(format!(
        "cupel-parser-{}-{}",
        std::process::id(),
        PARSER_RUNS.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&tmp).ok().map(|()| tmp)
}

/// The finding for the oracle `name`'s failure (ADR-016: its subject is the oracle's name).
fn problem(name: &str, failure: Failure) -> Problem {
    let (kind, title, evidence, action, fix) = match failure {
        Failure::Unreachable(unreachable) => (
            "oracle.unreachable",
            unreachable.title,
            json!({"oracle": name}),
            unreachable.action,
            unreachable.fix,
        ),
        Failure::Disagreed {
            title,
            mut evidence,
        } => {
            evidence["oracle"] = json!(name);
            (
                "oracle.smoke-failed",
                title,
                evidence,
                ActionKind::VerifyOracle,
                format!(
                    "Fix the oracle `{name}` so it answers deterministically, then run `cupel oracle verify {name}`."
                ),
            )
        }
    };
    Problem {
        check: "oracle",
        kind,
        subject: name.to_owned(),
        severity: Severity::Fail,
        title: format!("Oracle `{name}` is not verified: {title}"),
        detail: None,
        evidence,
        action,
        fix,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_jvm_parser_gets_a_temporary_directory_of_its_own_and_the_others_do_not() {
        let root = Path::new("/repo");
        let tmp = Path::new("/tmp/cupel-parser-1-0");
        assert_eq!(
            parser(root, "sany", &[], "spec/Counter.tla", Some(tmp)),
            Process::new("sany", root)
                .args(["spec/Counter.tla"])
                .merged()
                .java_tmpdir(tmp)
        );
        assert_eq!(
            parser(root, "quint", &["parse"], "spec/counter.qnt", None),
            Process::new("quint", root)
                .args(["parse", "spec/counter.qnt"])
                .merged()
        );
        let made = parser_tmpdir().unwrap();
        assert!(
            made.is_dir()
                && made
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("cupel-parser-"))
        );
        std::fs::remove_dir_all(made).unwrap();
    }
}
