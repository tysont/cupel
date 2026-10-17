// ABOUTME: `tool.atheris` (ADR-107): a Python project's atheris fuzz scripts (`fuzz_*.py` calling `atheris.Fuzz()`), each run
// ABOUTME: by `python3` with the budget as libFuzzer flags, and the uncaught Python exception that crashed it read back.
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use cupel_core::{
    Availability, Cancellation, Ctx, FuzzResults, Install, Location, Process, ToolId, ToolProbe,
};

use super::{Ended, Found, FuzzProducer, Fuzzer, Root, libfuzzer, located, sources};

const ID: &str = "tool.atheris";

/// What starts the fuzzer in a script, which makes a `fuzz_*.py` file a target.
const STARTS: &str = "atheris.Fuzz(";

/// The header atheris prints before an uncaught exception's type and message.
const UNCAUGHT: &str = "=== Uncaught Python exception: ===";

pub struct Atheris;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<FuzzResults, _>(FuzzProducer(Atheris)),
});

impl Fuzzer for Atheris {
    fn id(&self) -> &'static str {
        ID
    }

    fn lang(&self) -> &'static str {
        "python"
    }

    fn seeded(&self) -> bool {
        true
    }

    /// Ready when `python3` imports `atheris`.
    fn availability(&self, _: &Ctx) -> Availability {
        if importable() {
            return Availability::Ready;
        }
        Availability::MissingTool {
            name: ToolId::from("atheris"),
            install: Some(Install::Command(
                "python3 -m pip install atheris".to_owned(),
            )),
        }
    }

    fn tool(&self, _: &Ctx) -> Option<ToolProbe> {
        Some(ToolProbe {
            name: ToolId::from("atheris"),
            version: ["python3", "-c", "import atheris"]
                .map(str::to_owned)
                .to_vec(),
        })
    }

    /// Every `fuzz_*.py` file under the root that starts the fuzzer.
    fn targets(&self, root: &Root<'_>) -> cupel_core::Result<Option<Vec<Found>>> {
        Ok(Some(
            sources(&root.path, is_script)
                .into_iter()
                .map(|(file, path)| Found {
                    name: file.clone(),
                    line: entry_line(&fs::read_to_string(path).unwrap_or_default()),
                    file: Some(file),
                })
                .collect(),
        ))
    }

    /// `python3 <script> <corpora> <flags>`: atheris hands its arguments to libFuzzer.
    fn fuzz(&self, root: &Root<'_>, target: &Found, area: &Path) -> cupel_core::Result<Ended> {
        libfuzzer::run(
            libfuzzer::Invocation {
                program: "python3",
                dir: &root.path,
                before: vec![target.name.clone()],
                env: vec![("PYTHONDONTWRITEBYTECODE", "1")],
                separated: false,
            },
            area,
            None,
            &root.budget,
            &root.ctx.cancel,
        )
    }

    fn failure(&self, root: &Root<'_>, output: &str) -> (String, Option<Location>) {
        libfuzzer::reported(exception(&root.ctx.ctx.root, output), output)
    }

    fn reproduce(&self, _: &Root<'_>, target: &Found, input: &str) -> String {
        format!("python3 {} {input}", target.name)
    }
}

/// Whether `python3` imports `atheris`, asked once.
fn importable() -> bool {
    static IMPORTABLE: OnceLock<bool> = OnceLock::new();
    *IMPORTABLE.get_or_init(|| {
        Process::new("python3", ".")
            .args(["-c", "import atheris"])
            .run(&Cancellation::new())
            .is_ok_and(|ran| ran.exit_code == Some(0))
    })
}

/// Whether the file at `path` (`file` relative to its root) is an atheris fuzz script.
fn is_script(file: &str, path: &Path) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file);
    name.starts_with("fuzz_")
        && Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("py"))
        && fs::read_to_string(path).is_ok_and(|text| text.contains(STARTS))
}

/// The line that starts the fuzzer, 1-based; 1 when there is none.
fn entry_line(text: &str) -> u32 {
    text.lines()
        .position(|line| line.contains(STARTS))
        .and_then(|index| u32::try_from(index + 1).ok())
        .unwrap_or(1)
}

/// An uncaught exception's `Type: message` (the line after atheris's header) and the innermost
/// frame of its traceback in the repository (`File "<path>", line <n>`).
fn exception(repository: &Path, output: &str) -> Option<(String, Option<Location>)> {
    let mut lines = output
        .lines()
        .skip_while(|line| !line.contains(UNCAUGHT))
        .skip(1);
    let message = libfuzzer::masked(lines.next()?.trim());
    let location = lines
        .take_while(|line| !line.trim().is_empty())
        .filter_map(|line| frame(repository, line))
        .last();
    Some((message, location))
}

/// The place of one `File "<path>", line <n>, in <function>` traceback line, when it is in the
/// repository.
fn frame(repository: &Path, line: &str) -> Option<Location> {
    let rest = line.trim().strip_prefix("File \"")?;
    let (file, rest) = rest.split_once('"')?;
    let number = rest.strip_prefix(", line ")?;
    let number = number.split(',').next()?.trim().parse().ok()?;
    located(repository, file, number, None)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// atheris's report of an uncaught exception, in the shape its README's example shows: the
    /// header, the exception as `Type: message`, then Python's traceback. Written by hand, not
    /// captured: atheris does not build where this was written (ADR-107 open items).
    fn report(repository: &Path) -> String {
        let root = repository.display();
        format!(
            "INFO: Seed: 3\n\n === Uncaught Python exception: ===\nIndexError: string index out of range\n\
             Traceback (most recent call last):\n  File \"{root}/fuzz_first.py\", line 9, in TestOneInput\n    \
             first(data.decode())\n  File \"{root}/units.py\", line 3, in first\n    return text[0]\n\n\
             ==31== ERROR: libFuzzer: fuzz target exited\n"
        )
    }

    #[test]
    fn an_uncaught_exception_is_its_message_at_its_innermost_frame_in_the_repository() {
        let repository = tempfile::tempdir().unwrap();
        for file in ["fuzz_first.py", "units.py"] {
            fs::write(repository.path().join(file), "").unwrap();
        }
        let (message, location) = exception(repository.path(), &report(repository.path())).unwrap();
        assert_eq!(message, "IndexError: string index out of range");
        let location = location.unwrap();
        assert_eq!((location.file.as_str(), location.line), ("units.py", 3));
    }

    #[test]
    fn without_the_header_libfuzzer_has_the_last_word() {
        assert_eq!(
            exception(Path::new("/"), "==31== ERROR: libFuzzer: deadly signal\n"),
            None
        );
        assert_eq!(
            libfuzzer::error("==31== ERROR: libFuzzer: deadly signal\n").as_deref(),
            Some("deadly signal")
        );
    }

    #[test]
    fn a_target_is_a_fuzz_script_that_starts_the_fuzzer() {
        let dir = tempfile::tempdir().unwrap();
        let script = "import atheris\n\ndef main():\n    atheris.Fuzz()\n";
        fs::write(dir.path().join("fuzz_first.py"), script).unwrap();
        fs::write(dir.path().join("fuzz_helper.py"), "import atheris\n").unwrap();
        fs::write(dir.path().join("first.py"), script).unwrap();
        let found: Vec<String> = sources(dir.path(), is_script)
            .into_iter()
            .map(|(file, _)| file)
            .collect();
        assert_eq!(found, ["fuzz_first.py"]);
        assert_eq!(entry_line(script), 4);
    }
}
