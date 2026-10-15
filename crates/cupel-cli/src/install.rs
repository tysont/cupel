// ABOUTME: Installing the tools producers need (ADR-081): who may install (`--install`, `--no-install`, a
// ABOUTME: terminal's question), running each install command in the repository root, and the findings.
use std::io::{BufRead, IsTerminal, Write};
use std::path::Path;
use std::process::{Command, Stdio};

use cupel_core::{
    ActionKind, Cancellation, PlannedInstall, Process, Severity, ToolId, output_tail,
};
use serde_json::json;

use crate::problem::Problem;

/// The component install findings come from.
const CHECK: &str = "install";

/// Whether a run may install tools (design §6.1 rule 10).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    /// Install nothing, for the reason given.
    Never(&'static str),
    /// Ask before each install.
    Ask,
    /// Install without asking.
    Granted,
}

impl Permission {
    /// `--no-install` never installs, `--install` installs without asking, and neither asks
    /// when stdout is a terminal and installs nothing otherwise.
    pub fn of(install: bool, no_install: bool, stdout_is_terminal: bool) -> Self {
        if no_install {
            Self::Never("--no-install")
        } else if install {
            Self::Granted
        } else if stdout_is_terminal {
            Self::Ask
        } else {
            Self::Never("stdout is not a terminal; pass --install to install without asking")
        }
    }

    /// The installs a run would attempt: all of `planned` unless it may install nothing.
    pub fn would_install(self, planned: &[Planned]) -> Vec<PlannedInstall> {
        match self {
            Self::Never(_) => Vec::new(),
            Self::Ask | Self::Granted => planned.iter().map(Planned::plan).collect(),
        }
    }
}

/// A missing tool, the command that installs it, and the command that finds it afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub tool: ToolId,
    pub command: String,
    /// Exits 0, printing the tool's version first, once the tool is installed.
    pub probe: Vec<String>,
}

impl Planned {
    fn plan(&self) -> PlannedInstall {
        PlannedInstall {
            tool: self.tool.clone(),
            command: Some(self.command.clone()),
        }
    }
}

/// What became of one planned install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The command exited 0 and the probe now finds the tool, printing this version.
    Installed(String),
    /// The command did not exit 0 (`exit_code` null when it could not run or was cut short), or
    /// exited 0 and the tool is still not found.
    Failed {
        exit_code: Option<i32>,
        stderr: Option<String>,
    },
    /// The command was not run, for this reason.
    Skipped(&'static str),
}

/// The installs `permission` allows of `planned`, run one at a time in `root`; under `Ask`,
/// `ask` decides each. The run's cancellation stops the installs not yet started.
pub fn run(
    planned: &[Planned],
    permission: Permission,
    ask: &mut dyn FnMut(&Planned) -> bool,
    root: &Path,
    cancel: &Cancellation,
) -> Vec<(Planned, Outcome)> {
    planned
        .iter()
        .map(|install| {
            let outcome = match permission {
                Permission::Never(reason) => Outcome::Skipped(reason),
                Permission::Ask if !ask(install) => Outcome::Skipped("declined"),
                Permission::Ask | Permission::Granted => execute(install, root, cancel),
            };
            (install.clone(), outcome)
        })
        .collect()
}

/// Runs `install`'s command through `sh` in `root`, then probes for the tool.
fn execute(install: &Planned, root: &Path, cancel: &Cancellation) -> Outcome {
    let ran = Process::new("sh", root)
        .args(["-c", install.command.as_str()])
        .run(cancel);
    match ran {
        Ok(captured) if captured.exit_code == Some(0) => match probe(&install.probe) {
            Some(version) => Outcome::Installed(version),
            None => Outcome::Failed {
                exit_code: Some(0),
                stderr: output_tail(&captured.stderr),
            },
        },
        Ok(captured) => Outcome::Failed {
            exit_code: captured.exit_code,
            stderr: output_tail(&captured.stderr),
        },
        Err(error) => Outcome::Failed {
            exit_code: None,
            stderr: Some(error.to_string()),
        },
    }
}

/// What `probe` printed first (a tool's version), when it runs and exits 0; `None` when the
/// tool is not installed.
pub fn probe(probe: &[String]) -> Option<String> {
    let (program, args) = probe.split_first()?;
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8_lossy(&output.stdout).into_owned()
        + &String::from_utf8_lossy(&output.stderr);
    Some(
        text.lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default()
            .to_owned(),
    )
}

/// Asks on stderr whether to run `install` and reads the answer from stdin: `y` or `yes`
/// accepts; anything else, end of input, or a stdin that is not a terminal declines.
pub fn ask(install: &Planned) -> bool {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return false;
    }
    let mut stderr = std::io::stderr();
    let _ = write!(
        stderr,
        "cupel: install {} with `{}`? [y/N] ",
        install.tool, install.command
    );
    let _ = stderr.flush();
    let mut answer = String::new();
    if stdin.lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim().to_lowercase().as_str(), "y" | "yes")
}

/// The finding of each attempt.
pub fn problems(attempts: &[(Planned, Outcome)]) -> Vec<Problem> {
    attempts
        .iter()
        .map(|(install, outcome)| problem(install, outcome))
        .collect()
}

fn problem(install: &Planned, outcome: &Outcome) -> Problem {
    let (tool, command) = (&install.tool, &install.command);
    let (kind, severity, title, evidence, action, fix) = match outcome {
        Outcome::Installed(version) => (
            "install.succeeded",
            Severity::Info,
            format!("Installed `{tool}` with `{command}`"),
            json!({"tool": tool, "command": command, "version": version}),
            ActionKind::Review,
            format!("Nothing to do; `{tool}` is installed."),
        ),
        Outcome::Failed { exit_code, stderr } => (
            "install.failed",
            Severity::Fail,
            if *exit_code == Some(0) {
                format!("`{command}` finished, but `{tool}` is still not found on PATH")
            } else {
                format!("`{command}` failed to install `{tool}`")
            },
            json!({"tool": tool, "command": command, "exit_code": exit_code, "stderr": stderr}),
            ActionKind::InstallTool,
            format!(
                "Run `{command}` by hand and fix what stops it, or install `{tool}` another way and put it on PATH."
            ),
        ),
        Outcome::Skipped(reason) => (
            "install.skipped",
            Severity::Info,
            format!("Did not install `{tool}`: {reason}"),
            json!({"tool": tool, "command": command, "reason": reason}),
            ActionKind::InstallTool,
            format!("Run `{command}`, or `cupel doctor --fix --install`."),
        ),
    };
    Problem {
        check: CHECK,
        kind,
        subject: tool.to_string(),
        severity,
        title,
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
    fn no_install_wins_then_install_then_the_terminal_decides() {
        assert_eq!(
            Permission::of(false, true, true),
            Permission::Never("--no-install")
        );
        assert_eq!(Permission::of(true, false, false), Permission::Granted);
        assert_eq!(Permission::of(true, false, true), Permission::Granted);
        assert_eq!(Permission::of(false, false, true), Permission::Ask);
        assert!(matches!(
            Permission::of(false, false, false),
            Permission::Never(reason) if reason.contains("not a terminal")
        ));
    }

    /// An install of `tool` whose command is `command` and whose probe is `probe`.
    fn planned(tool: &str, command: &str, probe: &[&str]) -> Planned {
        Planned {
            tool: ToolId::from(tool),
            command: command.to_owned(),
            probe: probe.iter().map(|word| (*word).to_owned()).collect(),
        }
    }

    /// Each attempt of `planned` under `permission`, answering `answer` when asked, with how
    /// many questions were asked.
    fn attempted(
        planned: &[Planned],
        permission: Permission,
        answer: bool,
    ) -> (Vec<Outcome>, usize) {
        let dir = tempfile::tempdir().unwrap();
        let mut asked = 0;
        let mut ask = |_: &Planned| {
            asked += 1;
            answer
        };
        let outcomes = run(
            planned,
            permission,
            &mut ask,
            dir.path(),
            &Cancellation::new(),
        )
        .into_iter()
        .map(|(_, outcome)| outcome)
        .collect();
        (outcomes, asked)
    }

    #[test]
    fn never_runs_nothing_and_asks_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("ran");
        let install = planned("t", &format!("touch '{}'", marker.display()), &["true"]);
        let (outcomes, asked) = attempted(&[install], Permission::Never("--no-install"), true);
        assert_eq!(
            (outcomes, asked),
            (vec![Outcome::Skipped("--no-install")], 0)
        );
        assert!(!marker.exists());
    }

    #[test]
    fn ask_runs_only_what_is_accepted_and_granted_asks_nothing() {
        let install = planned("t", "true", &["echo", "t 1.0"]);
        let installs = [install.clone(), install];
        assert_eq!(
            attempted(&installs, Permission::Ask, false),
            (vec![Outcome::Skipped("declined"); 2], 2)
        );
        assert_eq!(
            attempted(&installs, Permission::Ask, true),
            (vec![Outcome::Installed("t 1.0".to_owned()); 2], 2)
        );
        assert_eq!(
            attempted(&installs, Permission::Granted, false),
            (vec![Outcome::Installed("t 1.0".to_owned()); 2], 0)
        );
    }

    #[test]
    fn a_failing_command_or_a_tool_still_missing_is_a_failure() {
        let failing = planned("t", "echo out; echo bad >&2; exit 7", &["true"]);
        let silent = planned("t", "true", &["cupel-no-such-program"]);
        assert_eq!(
            attempted(&[failing, silent], Permission::Granted, false).0,
            [
                Outcome::Failed {
                    exit_code: Some(7),
                    stderr: Some("bad".to_owned())
                },
                Outcome::Failed {
                    exit_code: Some(0),
                    stderr: None
                },
            ]
        );
    }

    #[test]
    fn a_dry_run_plans_every_install_unless_it_may_install_nothing() {
        let installs = [planned("t", "true", &["true"])];
        let plan = [PlannedInstall {
            tool: ToolId::from("t"),
            command: Some("true".to_owned()),
        }];
        assert_eq!(Permission::Granted.would_install(&installs), plan);
        assert_eq!(Permission::Ask.would_install(&installs), plan);
        assert!(Permission::Never("x").would_install(&installs).is_empty());
    }

    #[test]
    fn each_attempt_is_one_finding_about_its_tool() {
        let install = planned("t", "true", &["true"]);
        let attempts = [
            (install.clone(), Outcome::Skipped("declined")),
            (install.clone(), Outcome::Installed("t 1".to_owned())),
            (
                install,
                Outcome::Failed {
                    exit_code: None,
                    stderr: None,
                },
            ),
        ];
        let shape: Vec<(&str, String, Severity)> = problems(&attempts)
            .into_iter()
            .map(|problem| (problem.kind, problem.subject, problem.severity))
            .collect();
        assert_eq!(
            shape,
            [
                ("install.skipped", "t".to_owned(), Severity::Info),
                ("install.succeeded", "t".to_owned(), Severity::Info),
                ("install.failed", "t".to_owned(), Severity::Fail),
            ]
        );
    }
}
