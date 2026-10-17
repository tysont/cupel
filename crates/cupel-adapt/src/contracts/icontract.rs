// ABOUTME: `tool.icontract` (ADR-113): icontract's `ViolationError`s in a pytest failure, each placed at its decorator,
// ABOUTME: whose source says the kind and the function; the run turns on contracts declared `enabled=icontract.SLOW`.
use std::sync::OnceLock;

use cupel_core::{Availability, Cancellation, ContractKind, Install, Process, ToolId, ToolProbe};

use super::{Found, Library, Source};
use crate::race::report::masked;

/// icontract: `@require`, `@ensure`, and `@invariant` raise `icontract.errors.ViolationError:
/// File <path>, line <n> in <scope>:` followed by the condition and the values it saw.
pub static ICONTRACT: Library = Library {
    id: "tool.icontract",
    lang: "python",
    package: "icontract",
    manifests: &[
        "pyproject.toml",
        "setup.cfg",
        "setup.py",
        "requirements*.txt",
    ],
    // `icontract.SLOW` is true when this is set as icontract is imported.
    switches: &[("ICONTRACT_SLOW", "1")],
    availability,
    probe: Some(probe),
    read,
};

inventory::submit!(crate::ToolProducer {
    id: "tool.icontract",
    register: |registry| super::register(registry, &ICONTRACT),
});

/// What pytest prints before an icontract violation's header, after its `E` margin.
const HEADER: &str = "icontract.errors.ViolationError: File ";

/// Ready when `python3` imports `icontract`.
fn availability() -> Availability {
    if importable() {
        return Availability::Ready;
    }
    Availability::MissingTool {
        name: ToolId::from("icontract"),
        install: Some(Install::Command(
            "python3 -m pip install icontract".to_owned(),
        )),
    }
}

fn probe() -> ToolProbe {
    ToolProbe {
        name: ToolId::from("icontract"),
        version: [
            "python3",
            "-c",
            "import icontract; print(icontract.__version__)",
        ]
        .map(str::to_owned)
        .to_vec(),
    }
}

/// Whether `python3` imports `icontract`, asked once.
fn importable() -> bool {
    static IMPORTABLE: OnceLock<bool> = OnceLock::new();
    *IMPORTABLE.get_or_init(|| {
        Process::new("python3", ".")
            .args(["-c", "import icontract"])
            .run(&Cancellation::new())
            .is_ok_and(|ran| ran.exit_code == Some(0))
    })
}

/// Every violation in one failed test's `output`: the header with the decorator's file and
/// line, then the `E` lines that follow it (the condition, then the values it saw), joined with
/// `; ` and with addresses masked. `source` reads the decorator's file, whose text says whether
/// it is a `require`, `ensure`, or `invariant` and which function or class it decorates; a
/// violation whose source cannot be read is not reported.
fn read(output: &str, source: &Source<'_>) -> Vec<Found> {
    let lines: Vec<&str> = output.lines().map(unmargined).collect();
    let mut found = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        let Some(header) = line.strip_prefix(HEADER) else {
            continue;
        };
        let Some((file, line)) = placed(header) else {
            continue;
        };
        let Some((kind, function)) = source(&file).and_then(|text| declared(&text, line)) else {
            continue;
        };
        let said: Vec<String> = output
            .lines()
            .skip(index + 1)
            .map_while(|line| line.trim_start().strip_prefix('E'))
            .map(|line| line.trim().trim_end_matches(':').to_owned())
            .filter(|line| !line.is_empty())
            .collect();
        found.push(Found {
            kind,
            function,
            message: masked(&said.join("; ")),
            file: Some(file),
            line,
        });
    }
    found
}

/// A line of pytest's failure text without its `E` margin.
fn unmargined(line: &str) -> &str {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix('E')
        .filter(|rest| rest.starts_with(char::is_whitespace))
        .map_or(trimmed, str::trim_start)
}

/// The file and line of a header's `<path>, line <n> in <scope>:`.
fn placed(header: &str) -> Option<(String, u32)> {
    let (file, rest) = header.rsplit_once(", line ")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    Some((file.to_owned(), digits.parse().ok()?))
}

/// The kind of the decorator at `line` (1-based) of `text`, from the nearest decorator at or
/// above it, and the name of the function or class it decorates.
fn declared(text: &str, line: u32) -> Option<(ContractKind, Option<String>)> {
    let lines: Vec<&str> = text.lines().collect();
    let at = usize::try_from(line).ok()?.checked_sub(1)?;
    let decorator = lines
        .get(..=at)?
        .iter()
        .rev()
        .find(|line| line.trim_start().starts_with('@'))?;
    let kind = [
        ("require", ContractKind::Precondition),
        ("ensure", ContractKind::Postcondition),
        ("invariant", ContractKind::Invariant),
    ]
    .into_iter()
    .find(|(name, _)| decorator.contains(&format!("{name}(")))?
    .1;
    let function = lines[at..].iter().find_map(|line| {
        let line = line.trim_start();
        let rest = line
            .strip_prefix("async def ")
            .or_else(|| line.strip_prefix("def "))
            .or_else(|| line.strip_prefix("class "))?;
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        (!name.is_empty()).then_some(name)
    });
    Some((kind, function))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use cupel_fixtures::{Capture, repo_path};

    use super::*;
    use crate::report::junit;

    /// The capture's files as they are in the fixture it was captured from.
    fn fixture_source(capture: &Capture) -> impl Fn(&str) -> Option<String> + '_ {
        move |file| {
            let relative = Path::new(file).strip_prefix(&capture.ran_in).ok()?;
            fs::read_to_string(repo_path("spec.contracts/python").join(relative)).ok()
        }
    }

    /// Where the capture ran, and each failed test of it by name with what the reader found in its output.
    fn captured() -> (PathBuf, Vec<(String, Vec<Found>)>) {
        let capture = Capture::of("contracts/icontract", "ledger");
        let source = fixture_source(&capture);
        let mut read: Vec<(String, Vec<Found>)> = junit::pytest(&capture.file("junit.xml"))
            .unwrap()
            .into_iter()
            .filter_map(|test| Some((test.name.clone(), read(test.output.as_deref()?, &source))))
            .collect();
        read.sort_by(|a, b| a.0.cmp(&b.0));
        (capture.ran_in.clone(), read)
    }

    #[test]
    fn the_capture_exhibits_every_construct_of_the_conformance_inventory() {
        let found: Vec<Found> = captured()
            .1
            .into_iter()
            .flat_map(|(_, found)| found)
            .collect();
        assert_eq!(
            super::super::tests::missed_constructs(&found),
            Vec::<String>::new()
        );
    }

    #[test]
    fn every_violation_of_the_capture_is_read_with_its_kind_function_and_values() {
        let (ran_in, read) = captured();
        let ledger = ran_in.join("src/ledger.py").display().to_string();
        let found = |kind, function: &str, message: &str, line| {
            vec![Found {
                kind,
                function: Some(function.to_owned()),
                message: message.to_owned(),
                file: Some(ledger.clone()),
                line,
            }]
        };
        assert_eq!(
            read,
            [
                (
                    "test_halves_one".to_owned(),
                    found(
                        ContractKind::Postcondition,
                        "half",
                        "result > 0; amount was 1; result was 0",
                        11
                    )
                ),
                (
                    "test_posts_three_entries".to_owned(),
                    found(
                        ContractKind::Invariant,
                        "Account",
                        "len(self.entries) <= 2; len(self.entries) was 3; \
                         self was <ledger.Account object at 0x…>; self.entries was [1, 2, 3]",
                        21
                    )
                ),
                (
                    "test_shares_nothing_among_no_parts".to_owned(),
                    found(
                        ContractKind::Precondition,
                        "share",
                        "parts > 0; parts was 0; total was 10",
                        6
                    )
                ),
            ]
        );
    }

    #[test]
    fn a_violation_whose_source_cannot_be_read_or_names_no_contract_is_left_out() {
        let output = "E   icontract.errors.ViolationError: File /gone.py, line 3 in <module>:\n\
                      E   x > 0:\n";
        assert_eq!(read(output, &|_| None), []);
        let plain = "def f(x):\n    return x\n";
        assert_eq!(read(output, &|_| Some(plain.to_owned())), []);
    }

    #[test]
    fn a_decorator_over_several_lines_is_found_above_the_reported_line() {
        let text = "@icontract.ensure(\n    lambda result: result > 0,\n)\nasync def pay(x):\n    return x\n";
        assert_eq!(
            declared(text, 2),
            Some((ContractKind::Postcondition, Some("pay".to_owned())))
        );
    }
}
