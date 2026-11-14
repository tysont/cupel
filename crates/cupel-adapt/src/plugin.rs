// ABOUTME: Python modules a command loads as plugins, such as pytest-cov for pytest's `--cov` options: which ones a
// ABOUTME: command needs, the probe that asks its interpreter for each, and the toolchain row's install.
use std::io::Read;
use std::path::Path;

use cupel_core::{Install, ProducerId, ToolId, find_program};

use crate::toolchain;

/// A Python module a program loads when a command passes it certain options.
struct Plugin {
    /// The program that loads it, such as `pytest`.
    program: &'static str,
    /// The prefix of the options that need it, such as `--cov`.
    options: &'static str,
    /// The module's import name, such as `pytest_cov`.
    module: &'static str,
    /// The distribution that provides it, named as the missing tool.
    package: &'static str,
    /// The toolchain producer whose row's `install` adds it.
    row: &'static str,
}

/// The plugins the toolchain table's commands rely on.
const PLUGINS: &[Plugin] = &[Plugin {
    program: "pytest",
    options: "--cov",
    module: "pytest_cov",
    package: "pytest-cov",
    row: "tool.pytest-cov",
}];

/// A plugin a command needs: the program that loads it, the tool to report when it is missing,
/// a probe that exits 0 when the command's interpreter finds it, and how to install it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Needed {
    pub program: &'static str,
    pub tool: ToolId,
    pub probe: Vec<String>,
    pub install: Option<Install>,
}

/// The plugins `command` needs, run from `root` (whose launcher fills the install): each whose
/// program the command runs, through `<python> -m <program>` or as the program on `PATH`, with
/// one of its options. A command whose interpreter cannot be told (a launcher such as
/// `uv run`, which may change the environment to answer, or a program that is not a Python
/// script) needs none that Cupel can probe.
pub fn needed(command: &str, root: &Path) -> Vec<Needed> {
    let words: Vec<&str> = command.split_whitespace().collect();
    PLUGINS
        .iter()
        .filter_map(|plugin| {
            let at = words.iter().position(|word| *word == plugin.program)?;
            words[at + 1..]
                .iter()
                .any(|word| word.starts_with(plugin.options))
                .then_some(())?;
            let interpreter = interpreter(&words[..at], plugin.program)?;
            let code = format!(
                "import importlib.util, sys; sys.exit(importlib.util.find_spec('{}') is None)",
                plugin.module
            );
            Some(Needed {
                program: plugin.program,
                tool: ToolId::from(plugin.package),
                probe: vec![interpreter, "-c".to_owned(), code],
                install: toolchain::install(&ProducerId::from(plugin.row), plugin.package, root),
            })
        })
        .collect()
}

/// The interpreter that runs `program` after `launcher`: the interpreter of `<python> -m`, or,
/// with no launcher, the one `program` on `PATH` names in its `#!` line.
fn interpreter(launcher: &[&str], program: &str) -> Option<String> {
    match launcher {
        [python, "-m"] => is_python(python).then(|| (*python).to_owned()),
        [] => shebang(program),
        _ => None,
    }
}

/// The Python interpreter `program` on `PATH` names in its `#!` line, directly or through
/// `env`.
fn shebang(program: &str) -> Option<String> {
    let mut text = Vec::new();
    std::fs::File::open(find_program(program)?)
        .ok()?
        .take(256)
        .read_to_end(&mut text)
        .ok()?;
    let line = text.split(|byte| *byte == b'\n').next()?;
    let line = std::str::from_utf8(line).ok()?.strip_prefix("#!")?;
    let mut words = line.split_whitespace();
    let mut first = words.next()?;
    if first.rsplit('/').next() == Some("env") {
        first = words.find(|word| !word.starts_with('-'))?;
    }
    is_python(first).then(|| first.to_owned())
}

/// Whether `program` (a name or a path) is a Python interpreter, such as `python3.12`.
fn is_python(program: &str) -> bool {
    program
        .rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with("python"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::needed;

    /// The probe of the first plugin `command` needs, or `None` when it needs none Cupel can
    /// probe.
    fn probe(command: &str) -> Option<Vec<String>> {
        needed(command, Path::new("."))
            .first()
            .map(|need| need.probe.clone())
    }

    /// The probe `python` runs for pytest-cov.
    fn asked_of(python: &str) -> Vec<String> {
        let code =
            "import importlib.util, sys; sys.exit(importlib.util.find_spec('pytest_cov') is None)";
        vec![python.to_owned(), "-c".to_owned(), code.to_owned()]
    }

    #[test]
    fn a_pytest_command_with_cov_options_needs_pytest_cov_asked_of_its_interpreter() {
        assert_eq!(
            [
                "python -m pytest --cov=. --cov-report=lcov:{out}",
                "python3 -m pytest --cov-report=lcov:{out}",
                "python -m pytest",
                "uv run python -m pytest --cov=src --cov-report=lcov:{out}",
                "python -m coverage run -m unittest",
            ]
            .map(probe),
            [
                Some(asked_of("python")),
                Some(asked_of("python3")),
                None,
                None,
                None
            ]
        );
    }
}
