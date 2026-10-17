// ABOUTME: `tool.go-fuzz` (ADR-107): a Go module's native fuzz targets (`FuzzXxx` in `_test.go` files), each fuzzed with
// ABOUTME: `go test -fuzz` in a copy of the module under the cache, since Go saves failing inputs into the package's tree.
use std::fs;
use std::path::{Path, PathBuf};

use cupel_core::{
    Availability, Ctx, FuzzResults, FuzzStatus, Location, Process, ProducerId, ToolId, ToolProbe,
    find_program,
};
use serde::Deserialize;

use super::{Ended, Found, FuzzProducer, Fuzzer, Root, TARGET_SECONDS, last_count, located, walk};
use crate::go_test::packages;
use crate::runner::probe;
use crate::toolchain;

const ID: &str = "tool.go-fuzz";

/// What Go prints before the path, relative to the package, of the failing input it saved.
const WRITTEN: &str = "Failing input written to ";

pub struct GoFuzz;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<FuzzResults, _>(FuzzProducer(GoFuzz)),
});

impl Fuzzer for GoFuzz {
    fn id(&self) -> &'static str {
        ID
    }

    fn lang(&self) -> &'static str {
        "go"
    }

    /// Go's fuzzer takes no seed.
    fn seeded(&self) -> bool {
        false
    }

    fn availability(&self, ctx: &Ctx) -> Availability {
        if find_program("go").is_some() {
            return Availability::Ready;
        }
        Availability::MissingTool {
            name: ToolId::from("go"),
            install: toolchain::install(&ProducerId::from(ID), "go", &ctx.root),
        }
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        Some(probe("go", false, &["go", "version"], &ctx.root))
    }

    /// The root's `FuzzXxx` functions, as `go test -list` lists them, each at its definition.
    fn targets(&self, root: &Root<'_>) -> cupel_core::Result<Option<Vec<Found>>> {
        let listed = Process::new("go", &root.path)
            .args(["test", "-list", "^Fuzz", "-json"])
            .args(packages(&root.packages))
            .run(&root.ctx.cancel)?;
        let defined = definitions(&root.path);
        Ok(Some(
            listing(&listed.stdout)
                .into_iter()
                .map(|(package, name)| {
                    let at = defined.iter().find(|(_, function, _)| *function == name);
                    Found {
                        name: format!("{package}.{name}"),
                        file: at.map(|(file, _, _)| file.clone()),
                        line: at.map_or(1, |(_, _, line)| *line),
                    }
                })
                .collect(),
        ))
    }

    /// `go test -run=^$ -fuzz=^<Name>$ -fuzztime=<cases>x` in a fresh copy of the root, whose
    /// paths are put back to the root's in the output; the failing input moves to `area/inputs/`.
    fn fuzz(&self, root: &Root<'_>, target: &Found, area: &Path) -> cupel_core::Result<Ended> {
        let copy = area.join("tree");
        for file in walk(&root.path) {
            if let Ok(relative) = file.strip_prefix(&root.path) {
                root.ctx
                    .writer
                    .write(&copy.join(relative), &fs::read(&file).unwrap_or_default())?;
            }
        }
        let (package, name) = split(&target.name);
        let ran = Process::new("go", &copy)
            .args([
                "test".to_owned(),
                "-run=^$".to_owned(),
                format!("-fuzz=^{name}$"),
                format!("-fuzztime={}x", root.budget.cases),
                "-parallel=1".to_owned(),
                format!("-timeout={TARGET_SECONDS}s"),
                package.to_owned(),
                "-args".to_owned(),
                format!("-test.fuzzcachedir={}", area.join("cache").display()),
            ])
            .merged()
            .run(&root.ctx.cancel)?;
        let output = ran.stdout.replace(
            &copy.to_string_lossy().into_owned(),
            &root.path.to_string_lossy(),
        );
        let artifact = saved(&output, &copy.join(package_dir(target)))
            .map(|saved| keep(root, &saved, area))
            .transpose()?;
        let status = if ran.exit_code == Some(0) {
            FuzzStatus::Passed
        } else if output.contains("--- FAIL:") {
            FuzzStatus::Crashed
        } else {
            FuzzStatus::Failed
        };
        fs::remove_dir_all(&copy).ok();
        Ok(Ended {
            status,
            executions: last_count(&output, "execs: "),
            coverage: None,
            artifact,
            output,
        })
    }

    fn failure(&self, root: &Root<'_>, output: &str) -> (String, Option<Location>) {
        failure(&root.ctx.ctx.root, output)
    }

    /// Go reruns a saved input from the package's `testdata/fuzz/<Name>/`, where it belongs as a
    /// regression case.
    fn reproduce(&self, _: &Root<'_>, target: &Found, input: &str) -> String {
        let (_, name) = split(&target.name);
        let dir = package_dir(target);
        let hash = input.rsplit('/').next().unwrap_or(input);
        format!(
            "mkdir -p {dir}/testdata/fuzz/{name} && cp {input} {dir}/testdata/fuzz/{name}/ && \
             go test -run={name}/{hash} ./{dir}"
        )
    }

    /// A Go corpus file's value: the text of a single `string("…")` or `[]byte("…")` without
    /// escapes, else its encoded lines after the `go test fuzz v1` header.
    fn input(&self, artifact: &Path) -> String {
        decoded(&fs::read_to_string(artifact).unwrap_or_default())
    }
}

/// One `go test -json` event, of which only output lines matter here.
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Event {
    action: String,
    package: Option<String>,
    output: Option<String>,
}

/// The (package, function) of every fuzz target `go test -list ^Fuzz -json` printed.
fn listing(stdout: &str) -> Vec<(String, String)> {
    stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(line).ok())
        .filter(|event| event.action == "output")
        .filter_map(|event| {
            let name = event.output?.trim().to_owned();
            let fuzz = name.starts_with("Fuzz") && !name.contains(char::is_whitespace);
            fuzz.then_some((event.package?, name))
        })
        .collect()
}

/// Every `func FuzzXxx(` in a `_test.go` file under `dir`: (file relative to `dir`, name, line).
fn definitions(dir: &Path) -> Vec<(String, String, u32)> {
    let mut found = Vec::new();
    for path in walk(dir).filter(|path| path.to_string_lossy().ends_with("_test.go")) {
        let text = fs::read_to_string(&path).unwrap_or_default();
        let file = path
            .strip_prefix(dir)
            .map(|relative| relative.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        for (index, line) in text.lines().enumerate() {
            if let Some(rest) = line.strip_prefix("func Fuzz") {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                let number = u32::try_from(index + 1).unwrap_or(u32::MAX);
                found.push((file.clone(), format!("Fuzz{name}"), number));
            }
        }
    }
    found.sort();
    found
}

/// The directory of the target's package, relative to its project root: where the file that
/// defines it is, else the root (`.`).
fn package_dir(target: &Found) -> String {
    target
        .file
        .as_deref()
        .and_then(|file| Path::new(file).parent())
        .map(|dir| dir.to_string_lossy().into_owned())
        .filter(|dir| !dir.is_empty())
        .unwrap_or_else(|| ".".to_owned())
}

/// A target's (package, function) from `<package>.<Function>`.
fn split(target: &str) -> (&str, &str) {
    target.rsplit_once('.').unwrap_or((".", target))
}

/// The saved failing input the output names, relative to the copy's `package` directory.
fn saved(output: &str, package: &Path) -> Option<PathBuf> {
    let written = output
        .lines()
        .find_map(|line| line.trim().strip_prefix(WRITTEN))?
        .trim();
    Some(package.join(written)).filter(|path| path.is_file())
}

/// Copies the saved input to `area/inputs/`, which outlives the copy of the tree.
fn keep(root: &Root<'_>, saved: &Path, area: &Path) -> cupel_core::Result<PathBuf> {
    let name = saved.file_name().map_or(saved, Path::new);
    root.ctx.writer.write(
        &area.join("inputs").join(name),
        &fs::read(saved).unwrap_or_default(),
    )
}

/// What failed and where: the first message under the innermost `--- FAIL:`, without its
/// `<file>.go:<n>: ` prefix; for a panic, the first stack frame in the repository, else the
/// file the message names.
fn failure(repository: &Path, output: &str) -> (String, Option<Location>) {
    let after: Vec<&str> = output
        .lines()
        .skip_while(|line| !line.trim_start().starts_with("--- FAIL:"))
        .skip_while(|line| line.trim_start().starts_with("--- FAIL:"))
        .collect();
    let Some(first) = after.first().map(|line| line.trim()) else {
        return ("the target failed".to_owned(), None);
    };
    let (message, named) = match first.split_once(": ") {
        Some((place, message)) if place.contains(".go:") => (message.to_owned(), Some(place)),
        _ => (first.to_owned(), None),
    };
    let frame = after.iter().find_map(|line| stack_frame(repository, line));
    let location = if message.starts_with("panic: ") {
        frame
    } else {
        None
    };
    (
        message,
        location.or_else(|| named.and_then(|place| named_place(repository, &after, place))),
    )
}

/// The place of one stack line (`\t/path/file.go:12 +0x44`), when it is in the repository.
fn stack_frame(repository: &Path, line: &str) -> Option<Location> {
    let place = line.split_whitespace().next()?;
    let (file, number) = place.rsplit_once(':')?;
    let path = Path::new(file);
    if path.extension().is_none_or(|ext| ext != "go") || !path.is_absolute() {
        return None;
    }
    located(repository, file, number.parse().ok()?, None)
}

/// The place a message names (`first_test.go:13`), found as the stack frame of that file and
/// line when the output has one.
fn named_place(repository: &Path, after: &[&str], place: &str) -> Option<Location> {
    let (file, number) = place.rsplit_once(':')?;
    let line: u32 = number.parse().ok()?;
    after
        .iter()
        .filter_map(|text| stack_frame(repository, text))
        .find(|at| at.line == line && at.file.ends_with(file))
}

/// The value of a Go corpus file (see [`GoFuzz::input`]).
fn decoded(text: &str) -> String {
    let values: Vec<&str> = text.lines().skip(1).collect();
    if let [value] = values.as_slice() {
        for prefix in ["string(\"", "[]byte(\""] {
            if let Some(inner) = value
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_suffix("\")"))
                && !inner.contains(['\\', '"'])
            {
                return inner.to_owned();
            }
        }
    }
    values.join("\n")
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;

    #[test]
    fn the_listing_names_every_fuzz_target_with_its_package() {
        let capture = Capture::of("fuzz/go-fuzz", "list");
        assert_eq!(
            listing(&capture.file("output.txt")),
            [
                (
                    "example.com/units/text".to_owned(),
                    "FuzzFirstByte".to_owned()
                ),
                ("example.com/units/text".to_owned(), "FuzzEven".to_owned()),
            ]
        );
    }

    #[test]
    fn a_panic_is_its_message_at_its_first_frame_in_the_repository() {
        let capture = Capture::of("fuzz/go-fuzz", "crash");
        let repository = tempfile::tempdir().unwrap();
        fs::create_dir(repository.path().join("text")).unwrap();
        fs::write(repository.path().join("text/first.go"), "").unwrap();
        fs::write(repository.path().join("text/first_test.go"), "").unwrap();
        let output = capture.file("output.txt").replace(
            &capture.ran_in.to_string_lossy().into_owned(),
            &repository.path().to_string_lossy(),
        );
        let (message, location) = failure(repository.path(), &output);
        assert_eq!(
            message,
            "panic: runtime error: index out of range [0] with length 0"
        );
        let location = location.unwrap();
        assert_eq!(
            (location.file.as_str(), location.line),
            ("text/first.go", 5)
        );
        assert_eq!(last_count(&output, "execs: "), Some(0));
    }

    #[test]
    fn a_failed_check_is_its_message_at_the_line_it_names() {
        let repository = tempfile::tempdir().unwrap();
        fs::write(repository.path().join("even_test.go"), "").unwrap();
        let root = repository.path().display();
        let output = format!(
            "--- FAIL: FuzzEven (0.01s)\n    --- FAIL: FuzzEven (0.00s)\n        even_test.go:13: odd 3\n\
             \n    Failing input written to testdata/fuzz/FuzzEven/abc\n\t{root}/even_test.go:13 +0x1\n"
        );
        let (message, location) = failure(repository.path(), &output);
        assert_eq!(message, "odd 3");
        assert_eq!(location.map(|at| at.line), Some(13));
        assert_eq!(failure(repository.path(), "ok\n").0, "the target failed");
    }

    #[test]
    fn a_corpus_file_shows_its_one_simple_value_else_its_encoding() {
        let files = [
            "go test fuzz v1\nstring(\"\")\n",
            "go test fuzz v1\n[]byte(\"FUZ\")\n",
            "go test fuzz v1\nstring(\"a\\n\")\nint(3)\n",
        ];
        assert_eq!(
            files.map(decoded),
            ["", "FUZ", "string(\"a\\n\")\nint(3)"].map(str::to_owned)
        );
    }

    #[test]
    fn a_passing_run_reports_its_executions() {
        let capture = Capture::of("fuzz/go-fuzz", "pass");
        assert_eq!(
            last_count(&capture.file("output.txt"), "execs: "),
            Some(200)
        );
    }

    #[test]
    fn the_reproduction_puts_the_input_in_the_package_corpus() {
        let target = Found {
            name: "example.com/units/text.FuzzFirstByte".to_owned(),
            file: Some("text/first_test.go".to_owned()),
            line: 5,
        };
        let dir = tempfile::tempdir().unwrap();
        let ctx = cupel_core::ProduceCtx::new(
            Ctx::new(dir.path().to_path_buf()),
            cupel_core::Writer::new(dir.path(), false)
                .unwrap()
                .confined(),
            cupel_core::Cancellation::new(),
        );
        let root = Root {
            ctx: &ctx,
            dir: ".".to_owned(),
            path: dir.path().to_path_buf(),
            packages: Vec::new(),
            budget: super::super::Budget { cases: 1, seed: 0 },
            cache: dir.path().join(".cupel/cache/fuzz/go/root"),
        };
        assert_eq!(
            GoFuzz.reproduce(&root, &target, ".cupel/in/5838"),
            "mkdir -p text/testdata/fuzz/FuzzFirstByte && cp .cupel/in/5838 \
             text/testdata/fuzz/FuzzFirstByte/ && go test -run=FuzzFirstByte/5838 ./text"
        );
    }
}
