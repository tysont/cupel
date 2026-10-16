// ABOUTME: What one invocation reads from its process, gathered once by `main` or rebuilt by the daemon from its
// ABOUTME: client's request (ADR-085 §2), and what it writes back: stdout, stderr, and the exit code.
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use cupel_core::Cancellation;

use crate::format::Rendering;
use crate::session::{Session, Stream};

/// Everything an invocation reads from its process. `main` gathers it once; nothing else looks
/// at the process environment or working directory. The daemon builds one per request from what
/// its client sent (ADR-085 §2).
pub struct Environment {
    /// The arguments after the program name.
    pub args: Vec<OsString>,
    /// The process working directory, the root when `--cwd` is absent.
    pub current_dir: Option<PathBuf>,
    pub stdout_is_terminal: bool,
    /// The `CUPEL_*` variables. Configuration reads its settings from them;
    /// `CUPEL_DEBUG_PANIC=1` fails deliberately after parsing to exercise exit 6 (ADR-018), and
    /// `CUPEL_DEBUG_SLEEP_MS` injects a slow task to exercise exit 5 (ADR-020).
    pub variables: BTreeMap<String, String>,
    /// `NO_COLOR` is set to a non-empty value (no-color.org).
    pub no_color_variable: bool,
    /// `PATH`, which decides whether a daemon may run the invocation (ADR-085 §5).
    pub path: Option<String>,
    /// Where `--events stderr` and `serve`'s notices are written as they happen.
    pub stderr: Stream,
    /// The caches the invocation may find warm: its own, or the daemon's.
    pub session: Arc<Session>,
    /// A token the run's own cancellation sits under: the daemon's, for a request it serves.
    pub cancel: Cancellation,
}

impl Environment {
    /// `args` run in `current_dir` with nothing else from a process: stdout not a terminal, no
    /// variables, standard error for streams, and a fresh session.
    pub fn new(args: Vec<OsString>, current_dir: Option<PathBuf>) -> Self {
        Self {
            args,
            current_dir,
            stdout_is_terminal: false,
            variables: BTreeMap::new(),
            no_color_variable: false,
            path: None,
            stderr: Stream::stderr(),
            session: Arc::default(),
            cancel: Cancellation::new(),
        }
    }

    pub fn from_process() -> Self {
        Self {
            stdout_is_terminal: std::io::stdout().is_terminal(),
            variables: std::env::vars_os()
                .filter_map(|(key, value)| {
                    Some((key.into_string().ok()?, value.into_string().ok()?))
                })
                .filter(|(key, _)| key.starts_with("CUPEL_"))
                .collect(),
            no_color_variable: std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()),
            path: std::env::var("PATH").ok(),
            ..Self::new(
                std::env::args_os().skip(1).collect(),
                std::env::current_dir().ok(),
            )
        }
    }

    pub(crate) fn variable(&self, name: &str) -> Option<&str> {
        self.variables.get(name).map(String::as_str)
    }

    pub(crate) fn debug_panic(&self) -> bool {
        self.variable("CUPEL_DEBUG_PANIC") == Some("1")
    }

    /// `CUPEL_DEBUG_SLEEP_MS=<n>`: a slow task of `n` milliseconds that every verb but a dry run
    /// waits on its cancellation token before its own work, so tests can prove `--timeout`
    /// (ADR-020 §5). A value that is not a number is ignored.
    pub(crate) fn debug_sleep(&self) -> Option<Duration> {
        self.variable("CUPEL_DEBUG_SLEEP_MS")
            .and_then(|value| value.parse().ok())
            .map(Duration::from_millis)
    }

    /// Whether `rendering` gets color: text on a terminal with neither `--no-color` nor
    /// `NO_COLOR` (ADR-019 §4).
    pub(crate) fn color(&self, rendering: Rendering, no_color_flag: bool) -> bool {
        rendering == Rendering::Text
            && self.stdout_is_terminal
            && !no_color_flag
            && !self.no_color_variable
    }
}

/// What to write and how to exit.
pub struct Output {
    pub stdout: String,
    pub stderr: String,
    pub code: u8,
}
