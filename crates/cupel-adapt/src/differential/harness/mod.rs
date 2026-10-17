// ABOUTME: The differential engine's harnesses (ADR-093 §6): per language, a generated program that loads one
// ABOUTME: function and answers each input line; and the batch protocol every harness speaks, crashes included.
mod ecma;
mod go;
mod python;
mod rust;

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use cupel_core::{Answer, Cancellation, Captured, Error, Process};
use serde_json::Value;

use super::entry::{Entry, Resolved};
use super::generate::Input;
use super::types::{self, Signature, Unsupported};

/// The variable naming the file a harness reads its inputs from, one JSON array per line.
const INPUTS: &str = "CUPEL_DIFFERENTIAL_IN";
/// The variable naming the file a harness appends its answers to, after its `ready` line.
const ANSWERS: &str = "CUPEL_DIFFERENTIAL_OUT";
/// The line a harness writes once its function is loaded.
const READY: &str = r#"{"ready":true}"#;

/// How long one batch may run before its first unanswered input is `crashed`.
const BATCH_TIMEOUT: Duration = Duration::from_secs(60);

/// Lines of a harness's output kept in a skip reason.
const TAIL_LINES: usize = 5;

/// The languages the engine has a harness for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Python,
    JavaScript,
    TypeScript,
    Go,
    Rust,
}

impl Language {
    pub const ALL: [Self; 5] = [
        Self::Python,
        Self::JavaScript,
        Self::TypeScript,
        Self::Go,
        Self::Rust,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::JavaScript => "javascript",
            Self::TypeScript => "typescript",
            Self::Go => "go",
            Self::Rust => "rust",
        }
    }

    /// The program every harness of the language runs or builds with.
    pub fn program(self) -> &'static str {
        match self {
            Self::Python => "python3",
            Self::JavaScript | Self::TypeScript => "node",
            Self::Go => "go",
            Self::Rust => "cargo",
        }
    }

    /// The command that prints the program's version.
    pub fn version(self) -> &'static [&'static str] {
        match self {
            Self::Python => &["python3", "--version"],
            Self::JavaScript | Self::TypeScript => &["node", "--version"],
            Self::Go => &["go", "version"],
            Self::Rust => &["cargo", "--version"],
        }
    }

    /// The parameters of a function with the normalized signature `text`.
    pub fn signature(self, text: &str) -> Result<Signature, Unsupported> {
        match self {
            Self::Python => types::python(text),
            Self::JavaScript | Self::TypeScript => types::ecma(text),
            Self::Go => types::go(text),
            Self::Rust => types::rust(text),
        }
    }

    /// The harness of `call` in `site`, built and ready to answer batches.
    pub fn prepare(self, call: &Call<'_>, site: &Site<'_>) -> Result<Prepared, Problem> {
        fs::create_dir_all(&site.work).map_err(|error| io_problem(&site.work, &error))?;
        match self {
            Self::Python => python::prepare(call, site),
            Self::JavaScript | Self::TypeScript => ecma::prepare(call, site),
            Self::Go => go::prepare(call, site),
            Self::Rust => rust::prepare(call, site),
        }
    }
}

/// One entry to call: what it names, how it resolved, and its parameters.
pub struct Call<'a> {
    pub entry: &'a Entry,
    pub resolved: &'a Resolved,
    pub signature: &'a Signature,
}

/// Where a harness runs: the tree holding the entry's file, an empty directory for the
/// harness's own files, a build directory for compiled harnesses, and the run's cancellation.
pub struct Site<'a> {
    pub tree: &'a Path,
    pub work: PathBuf,
    pub target: PathBuf,
    pub cancel: &'a Cancellation,
}

/// Why an entry could not be compared, or why the whole comparison stopped.
#[derive(Debug)]
pub enum Problem {
    /// The entry is skipped with this reason.
    Skip(String),
    /// The comparison cannot go on: cancelled, or the oracle is unreachable.
    Fatal(Error),
}

/// A harness ready to answer batches of inputs.
pub struct Prepared {
    process: Process,
    work: PathBuf,
}

impl Prepared {
    pub fn new(process: Process, work: PathBuf) -> Self {
        Self { process, work }
    }

    /// The answer to each of `inputs`, in order. A process that ends early leaves its first
    /// unanswered input `crashed` and runs again on the rest.
    pub fn answers(&self, inputs: &[Input], cancel: &Cancellation) -> Result<Vec<Answer>, Problem> {
        let mut answers = Vec::with_capacity(inputs.len());
        while answers.len() < inputs.len() {
            let (answered, ended) = self.batch(&inputs[answers.len()..], cancel)?;
            answers.extend(answered);
            if answers.len() < inputs.len() {
                answers.push(Answer::Crashed(ended));
            }
        }
        Ok(answers)
    }

    /// One run on `inputs`: the answers it wrote, and how it ended.
    fn batch(
        &self,
        inputs: &[Input],
        cancel: &Cancellation,
    ) -> Result<(Vec<Answer>, String), Problem> {
        let (path_in, path_out) = (
            self.work.join("inputs.jsonl"),
            self.work.join("answers.jsonl"),
        );
        let lines: String = inputs
            .iter()
            .flat_map(|input| [Value::Array(input.clone()).to_string(), "\n".to_owned()])
            .collect();
        fs::write(&path_in, lines).map_err(|error| io_problem(&path_in, &error))?;
        let _ = fs::remove_file(&path_out);
        let ran = self
            .process
            .clone()
            .env(INPUTS, &path_in.to_string_lossy())
            .env(ANSWERS, &path_out.to_string_lossy())
            .run(&cancel.child(BATCH_TIMEOUT));
        if cancel.is_cancelled() {
            return Err(Problem::Fatal(Error::Cancelled));
        }
        let written = fs::read_to_string(&path_out).unwrap_or_default();
        let mut lines = written.lines();
        let ended = match &ran {
            Ok(captured) => ending(captured),
            Err(Error::Cancelled) => format!("timed out after {} s", BATCH_TIMEOUT.as_secs()),
            Err(error) => return Err(Problem::Skip(error.to_string())),
        };
        if lines.next() != Some(READY) {
            let said = ran
                .map(|captured| tail(&captured.stderr))
                .unwrap_or_default();
            return Err(Problem::Skip(format!(
                "the harness did not start ({ended}): {said}"
            )));
        }
        let answers = lines.map(answer).collect::<Result<_, _>>()?;
        Ok((answers, ended))
    }
}

/// How a finished harness process ended, for a `crashed` answer.
fn ending(captured: &Captured) -> String {
    match captured.exit_code {
        Some(code) => format!("exit {code}"),
        None => "killed by a signal".to_owned(),
    }
}

/// One answer line: `{"returned": text}` or `{"raised": text}`.
fn answer(line: &str) -> Result<Answer, Problem> {
    let parsed: Value = serde_json::from_str(line)
        .map_err(|_| Problem::Skip(format!("the harness wrote `{line}`, not an answer")))?;
    let text = |key: &str| parsed.get(key).and_then(Value::as_str).map(str::to_owned);
    text("returned")
        .map(Answer::Returned)
        .or_else(|| text("raised").map(Answer::Raised))
        .ok_or_else(|| Problem::Skip(format!("the harness wrote `{line}`, not an answer")))
}

/// The last non-blank lines of `text`, joined, for a reason.
pub fn tail(text: &str) -> String {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    lines[lines.len().saturating_sub(TAIL_LINES)..].join(" | ")
}

fn io_problem(path: &Path, error: &std::io::Error) -> Problem {
    Problem::Skip(format!("cannot write `{}`: {error}", path.display()))
}

/// Writes `contents` to `name` in the harness's work directory.
fn write(site: &Site<'_>, name: &str, contents: &str) -> Result<PathBuf, Problem> {
    let path = site.work.join(name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| io_problem(parent, &error))?;
    }
    fs::write(&path, contents).map_err(|error| io_problem(&path, &error))?;
    Ok(path)
}

/// Runs a harness's build step, which must exit 0.
fn build(process: &Process, cancel: &Cancellation) -> Result<(), Problem> {
    let ran = process.run(cancel).map_err(|error| match error {
        Error::Cancelled => Problem::Fatal(Error::Cancelled),
        other => Problem::Skip(other.to_string()),
    })?;
    if ran.exit_code == Some(0) {
        Ok(())
    } else {
        Err(Problem::Skip(format!(
            "the harness did not build: {}",
            tail(&format!("{}\n{}", ran.stdout, ran.stderr))
        )))
    }
}
