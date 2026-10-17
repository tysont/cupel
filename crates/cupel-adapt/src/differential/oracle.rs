// ABOUTME: The oracle's side of a differential comparison (ADR-093 §7): a ref's commit exported with `git archive`
// ABOUTME: and run through the same harness, or a command run by `sh -c` once per input; and the first disagreement.
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::config::OracleKind;
use cupel_core::git::Git;
use cupel_core::{Answer, Cancellation, ComparedOracle, Error, Process, RunOracle};

use super::compare::agree;
use super::generate::{Input, command_input};
use super::harness::{Prepared, Problem, tail};
use crate::runner::quote;

/// The exit code `sh` reports when the command's program is not found.
const NOT_FOUND: i32 = 127;

/// The run's oracle, opened for comparisons.
pub enum Opened {
    /// A commit exported into `tree`.
    Ref { commit: String, tree: PathBuf },
    /// A shell command with an `{input}` placeholder, and its known inputs.
    Command { cmd: String, inputs: Vec<String> },
    /// A specification, which the engine does not compare against (ADR-094 §3).
    Spec,
}

impl Opened {
    /// Opens `oracle`, exporting a ref's commit under `scratch`.
    pub fn open(
        root: &Path,
        oracle: &RunOracle,
        scratch: &Path,
        cancel: &Cancellation,
    ) -> cupel_core::Result<Self> {
        let definition = &oracle.definition;
        let given = |value: &Option<String>| value.clone().unwrap_or_default();
        Ok(match definition.kind {
            OracleKind::Ref => {
                let commit = resolve(root, &given(&definition.reference))?;
                let tree = scratch.join("tree");
                export(root, &commit, scratch, &tree, cancel)?;
                Self::Ref { commit, tree }
            }
            OracleKind::Command => Self::Command {
                cmd: given(&definition.cmd),
                inputs: definition.inputs.clone(),
            },
            OracleKind::Spec => Self::Spec,
        })
    }

    /// The oracle as the fact names it.
    pub fn compared(&self, oracle: &RunOracle) -> ComparedOracle {
        ComparedOracle {
            name: oracle.name.clone(),
            kind: oracle.definition.kind,
            commit: match self {
                Self::Ref { commit, .. } => Some(commit.clone()),
                _ => None,
            },
        }
    }

    /// A command oracle's known inputs; none for the other kinds.
    pub fn known(&self) -> &[String] {
        match self {
            Self::Command { inputs, .. } => inputs,
            _ => &[],
        }
    }
}

/// The commit `reference` names in the repository at `root`.
fn resolve(root: &Path, reference: &str) -> cupel_core::Result<String> {
    let git = Git::open(root).ok_or_else(|| git_error("the repository is not a git work tree"))?;
    git.commit(reference)
        .map_err(|error| git_error(&error.message))
}

/// Writes the tree of `commit` into `tree` through an archive under `scratch`.
fn export(
    root: &Path,
    commit: &str,
    scratch: &Path,
    tree: &Path,
    cancel: &Cancellation,
) -> cupel_core::Result<()> {
    let archive = scratch.join("tree.tar");
    fs::create_dir_all(tree).map_err(|error| git_error(&error.to_string()))?;
    let archive_text = archive.to_string_lossy().into_owned();
    let steps = [
        Process::new("git", root).args(["archive", "--format=tar", "-o", &archive_text, commit]),
        Process::new("tar", scratch).args(["-xf", &archive_text, "-C", &tree.to_string_lossy()]),
    ];
    for step in steps {
        let ran = step.run(cancel)?;
        if ran.exit_code != Some(0) {
            return Err(git_error(&format!(
                "cannot export {commit}: {}",
                tail(&ran.stderr)
            )));
        }
    }
    Ok(())
}

fn git_error(message: &str) -> Error {
    Error::Process {
        program: "git".to_owned(),
        message: message.to_owned(),
    }
}

/// What answers for the oracle in one entry's comparison.
pub enum Side<'a> {
    /// The entry's harness in the ref's tree.
    Harness(Prepared),
    /// The command, run in the repository root.
    Command { cmd: &'a str, root: &'a Path },
}

/// How far a comparison of one batch got: inputs both sides answered, and the first input
/// that diverged with the oracle's answer to it.
pub struct Disagreement {
    pub compared: u64,
    pub first: Option<(usize, Answer)>,
}

impl Side<'_> {
    /// Compares the oracle's answers to `inputs` with `ours`, the implementation's, in order,
    /// up to the first disagreement; a command stops running there.
    pub fn first_disagreement(
        &self,
        inputs: &[Input],
        ours: &[Answer],
        kind: OracleKind,
        cancel: &Cancellation,
    ) -> Result<Disagreement, Problem> {
        let theirs: Vec<Answer> = match self {
            Self::Harness(prepared) => prepared.answers(inputs, cancel)?,
            Self::Command { cmd, root } => {
                for (at, (input, ours)) in inputs.iter().zip(ours).enumerate() {
                    let answer = run_command(cmd, root, input, cancel)?;
                    if !agree(ours, &answer, kind) {
                        return Ok(Disagreement {
                            compared: at as u64 + 1,
                            first: Some((at, answer)),
                        });
                    }
                }
                return Ok(Disagreement {
                    compared: inputs.len() as u64,
                    first: None,
                });
            }
        };
        Ok(Disagreement {
            compared: inputs.len() as u64,
            first: ours
                .iter()
                .zip(theirs)
                .enumerate()
                .find(|(_, (ours, theirs))| !agree(ours, theirs, kind))
                .map(|(at, (_, theirs))| (at, theirs)),
        })
    }
}

/// The command's answer to `input`: its output on exit 0, its exit and error output otherwise.
fn run_command(
    cmd: &str,
    root: &Path,
    input: &Input,
    cancel: &Cancellation,
) -> Result<Answer, Problem> {
    let line = cmd.replace("{input}", &quote(&command_input(input)));
    let ran = Process::new("sh", root)
        .args(["-c", &line])
        .run(cancel)
        .map_err(Problem::Fatal)?;
    Ok(match ran.exit_code {
        Some(0) => Answer::Returned(
            ran.stdout
                .strip_suffix('\n')
                .unwrap_or(&ran.stdout)
                .to_owned(),
        ),
        Some(NOT_FOUND) => {
            return Err(Problem::Fatal(Error::Process {
                program: "sh".to_owned(),
                message: format!(
                    "the oracle's program is not installed: {}",
                    tail(&ran.stderr)
                ),
            }));
        }
        Some(code) => Answer::Raised(format!("exit {code}: {}", tail(&ran.stderr))),
        None => Answer::Crashed("killed by a signal".to_owned()),
    })
}
