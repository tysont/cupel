// ABOUTME: `tool.kani` (ADR-117): `cargo kani` in each Rust project root, its terse report read into each harness's
// ABOUTME: verdict, the checks it failed with their places, and the concrete playback values that refute each.
use std::path::Path;

use cupel_core::{
    Availability, CostClass, Counterexample, Ctx, EventSink, FactKind, HarnessStatus, Install,
    LangId, Langs, Location, ModelCheckResults, ModelHarness, Process, ProduceCtx, Producer,
    ProducerId, RefutedProperty, Result, Scope, ToolProbe, output_tail,
};

use crate::fuzz::{in_scope, sources};
use crate::runner::{on_path, probe, root_run, suite_roots};
use crate::tree::{canonical, relative, under};

const ID: &str = "tool.kani";

/// Checks a Rust root's `#[kani::proof]` harnesses with Kani.
pub struct Kani;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<ModelCheckResults, _>(Kani),
});

impl Producer<ModelCheckResults> for Kani {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    fn languages(&self) -> Langs {
        Langs::Only(vec![LangId::from("rust")])
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Exhaustive
    }

    /// Ready with `cargo-kani` on `PATH`; its first run downloads the rest (`cargo kani setup`).
    fn availability(&self, _: &Ctx) -> Availability {
        on_path(
            "cargo-kani",
            Install::Command("cargo install --locked kani-verifier && cargo kani setup".to_owned()),
        )
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        Some(probe(
            "cargo-kani",
            false,
            &["cargo", "kani", "--version"],
            &ctx.root,
        ))
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<ModelCheckResults> {
        let root = canonical(&ctx.ctx.root);
        let target = ctx.writer.cache_dir().join("model-check").join("kani");
        let mut fact = ModelCheckResults {
            tool: self.id(),
            runs: Vec::new(),
            harnesses: Vec::new(),
        };
        for (dir, _) in suite_roots(ctx) {
            if !in_scope(scope, &dir, &["rs"]) {
                continue;
            }
            ctx.cancel.checkpoint()?;
            let checked = Checked {
                root: &root,
                path: canonical(&root.join(&dir)),
                dir,
            };
            let read = read(&command(&checked.path, &target).run(&ctx.cancel)?.stdout);
            fact.runs.push(root_run(checked.dir.clone(), read.failed));
            let mut harnesses: Vec<ModelHarness> = read
                .harnesses
                .into_iter()
                .map(|raw| checked.harness(raw))
                .filter(|harness| {
                    harness
                        .location
                        .as_ref()
                        .is_none_or(|at| !ctx.ctx.exclude.excludes(&at.file))
                })
                .collect();
            harnesses.sort_by(|a, b| a.name.cmp(&b.name));
            fact.harnesses.extend(harnesses);
        }
        Ok(fact)
    }
}

/// `cargo kani` in the project root `path`, terse, printing concrete playback, building into
/// `target`.
fn command(path: &Path, target: &Path) -> Process {
    Process::new("cargo", path)
        .args([
            "kani",
            "--output-format",
            "terse",
            "-Z",
            "concrete-playback",
            "--concrete-playback=print",
            "--target-dir",
            &target.to_string_lossy(),
        ])
        .merged()
}

/// A project root Kani checked: the repository root, the project root on disk, and relative.
struct Checked<'a> {
    root: &'a Path,
    path: std::path::PathBuf,
    dir: String,
}

impl Checked<'_> {
    /// The harness Kani reported, placed in the repository.
    fn harness(&self, raw: RawHarness) -> ModelHarness {
        ModelHarness {
            root: self.dir.clone(),
            location: defined(&self.path, &raw.name)
                .map(|(file, line)| at(under(&self.dir, &file), line)),
            status: raw.status,
            refuted: raw
                .refuted
                .into_iter()
                .map(|property| RefutedProperty {
                    location: property
                        .file
                        .as_deref()
                        .and_then(|file| located(self.root, &self.path, file, property.line)),
                    description: property.description,
                    function: property.function,
                    counterexample: property.counterexample,
                })
                .collect(),
            name: raw.name,
        }
    }
}

/// A one-line location.
fn at(file: String, line: u32) -> Location {
    Location {
        file,
        line,
        col: None,
        end_line: line,
        end_col: None,
    }
}

/// `file`, as Kani printed it running in `dir`, at `line`, when it is in the repository.
fn located(root: &Path, dir: &Path, file: &str, line: u32) -> Option<Location> {
    relative(root, dir, file).map(|file| at(file, line))
}

/// Where the harness `name` (`proofs::average_never_overflows`) is defined under the project
/// root `dir`: the file, relative to `dir`, and line of a `fn` of its last segment within three
/// lines after a `kani::proof` attribute.
fn defined(dir: &Path, name: &str) -> Option<(String, u32)> {
    let function = name.rsplit("::").next()?;
    let signature = format!("fn {function}(");
    sources(dir, |relative, _| relative.ends_with(".rs"))
        .into_iter()
        .find_map(|(relative, path)| {
            let text = std::fs::read_to_string(path).ok()?;
            let lines: Vec<&str> = text.lines().collect();
            let index = lines.iter().enumerate().position(|(index, line)| {
                line.contains(&signature)
                    && lines[index.saturating_sub(3)..index]
                        .iter()
                        .any(|before| before.contains("kani::proof"))
            })?;
            Some((relative, u32::try_from(index + 1).ok()?))
        })
}

/// A harness as Kani reported it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RawHarness {
    name: String,
    status: HarnessStatus,
    refuted: Vec<RawProperty>,
}

/// A failed check as Kani reported it, with the playback values Kani wrote for it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct RawProperty {
    description: String,
    file: Option<String>,
    line: u32,
    function: Option<String>,
    counterexample: Option<Counterexample>,
}

/// What `cargo kani` printed for one project root.
#[derive(Debug, Default, PartialEq, Eq)]
struct Read {
    harnesses: Vec<RawHarness>,
    /// Why the run did not complete: a build failure, or output without a harness's verdict.
    failed: Option<String>,
}

/// Reads Kani's terse report with concrete playback printed: each `Checking harness`, its
/// `Failed Checks` and their `File:` lines, its `VERIFICATION:-` verdict, and the playback
/// tests after it.
fn read(output: &str) -> Read {
    let mut harnesses: Vec<RawHarness> = Vec::new();
    let mut lines = output.lines();
    while let Some(line) = lines.next() {
        if let Some(name) = line
            .strip_prefix("Checking harness ")
            .and_then(|rest| rest.strip_suffix("..."))
        {
            harnesses.push(RawHarness {
                name: name.to_owned(),
                status: HarnessStatus::Undetermined,
                refuted: Vec::new(),
            });
        } else if let Some(harness) = harnesses.last_mut() {
            harness.read(line, &mut lines);
        }
    }
    let failed = failure(output, !harnesses.is_empty());
    Read { harnesses, failed }
}

impl RawHarness {
    /// Reads a line of this harness's report, and a playback test's lines after it from `rest`.
    fn read<'a>(&mut self, line: &str, rest: &mut impl Iterator<Item = &'a str>) {
        if let Some(verdict) = line.strip_prefix("VERIFICATION:- ") {
            self.status = match verdict.trim() {
                "SUCCESSFUL" => HarnessStatus::Verified,
                "FAILED" => HarnessStatus::Failed,
                _ => HarnessStatus::Undetermined,
            };
        } else if let Some(description) = line.strip_prefix("Failed Checks: ") {
            self.refuted.push(RawProperty {
                description: description.to_owned(),
                file: None,
                line: 0,
                function: None,
                counterexample: None,
            });
        } else if let Some(place) = line.trim_start().strip_prefix("File: ") {
            if let Some(property) = self.refuted.last_mut() {
                (property.file, property.line, property.function) = file_line(place);
            }
        } else if line.starts_with("Concrete playback unit test for ") {
            let (check, counterexample) = playback(rest);
            attach(self, check.as_deref(), counterexample);
        }
    }
}

/// `"src/lib.rs", line 3, in average` as its file, line, and function.
fn file_line(place: &str) -> (Option<String>, u32, Option<String>) {
    let mut parts = place.splitn(3, ", ");
    let file = parts.next().map(|file| file.trim_matches('"').to_owned());
    let line = parts
        .next()
        .and_then(|line| line.strip_prefix("line "))
        .and_then(|line| line.trim().parse().ok())
        .unwrap_or(0);
    let function = parts
        .next()
        .and_then(|function| function.strip_prefix("in "))
        .map(|function| function.trim().to_owned());
    (file, line, function)
}

/// The fenced playback test after `Concrete playback unit test for …:`: the check it is for
/// (`/// Check for `assertion`: "…"`), and its values (the `// …` comments) with the test.
fn playback<'a>(lines: &mut impl Iterator<Item = &'a str>) -> (Option<String>, Counterexample) {
    let mut test = Vec::new();
    let mut fenced = false;
    for line in lines.by_ref() {
        if line.trim() == "```" {
            if fenced {
                break;
            }
            fenced = true;
            continue;
        }
        if fenced {
            test.push(line.trim_end());
        }
    }
    let check = test.iter().find_map(|line| {
        let (_, rest) = line.split_once("/// Check for `")?;
        let (_, description) = rest.split_once("`: ")?;
        Some(unquoted(description).to_owned())
    });
    let values = test
        .iter()
        .map(|line| line.trim())
        .filter(|line| line.starts_with("// "))
        .map(|line| line.trim_start_matches("// ").to_owned())
        .collect();
    let replay = test.join("\n") + "\n";
    (
        check,
        Counterexample {
            values,
            replay: Some(replay),
        },
    )
}

/// `text` without one pair of surrounding double quotes.
fn unquoted(text: &str) -> &str {
    text.strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or(text)
}

/// Gives `counterexample` to the harness's refuted property `check` names, else to its first
/// property without one.
fn attach(harness: &mut RawHarness, check: Option<&str>, counterexample: Counterexample) {
    let without = |property: &&mut RawProperty| property.counterexample.is_none();
    let named = harness
        .refuted
        .iter()
        .position(|property| Some(property.description.as_str()) == check);
    let target = match named {
        Some(index) => harness.refuted.get_mut(index),
        None => harness.refuted.iter_mut().find(|p| without(p)),
    };
    if let Some(property) = target.filter(|p| without(p)) {
        property.counterexample = Some(counterexample);
    }
}

/// Why a run did not complete, from its merged `output`, when it did not: a compiler error, or
/// a run that checked no harness and did not say there were none.
fn failure(output: &str, checked: bool) -> Option<String> {
    let error = output
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("error[E") || line.starts_with("error: could not compile"));
    if let Some(error) = error {
        return Some(error.to_owned());
    }
    if checked || output.contains("No proof harnesses") {
        return None;
    }
    Some(output_tail(output).unwrap_or_else(|| "cargo kani printed nothing".to_owned()))
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;

    fn property(description: &str, line: u32, function: &str, values: &[&str]) -> RawProperty {
        RawProperty {
            description: description.to_owned(),
            file: Some("src/lib.rs".to_owned()),
            line,
            function: Some(function.to_owned()),
            counterexample: Some(Counterexample {
                values: values.iter().map(|value| (*value).to_owned()).collect(),
                replay: None,
            }),
        }
    }

    /// The harnesses of `read` with each replay left out, which the next test reads alone.
    fn without_replays(read: Read) -> Vec<RawHarness> {
        read.harnesses
            .into_iter()
            .map(|mut harness| {
                for property in &mut harness.refuted {
                    if let Some(counterexample) = property.counterexample.as_mut() {
                        counterexample.replay = None;
                    }
                }
                harness
            })
            .collect()
    }

    #[test]
    fn each_harness_has_its_verdict_and_each_failed_check_its_place_and_values() {
        let read = read(&Capture::of("model-check/kani", "harnesses").file("output.txt"));
        assert_eq!(read.failed, None);
        assert_eq!(
            without_replays(read),
            [
                RawHarness {
                    name: "proofs::at_stays_in_bounds".to_owned(),
                    status: HarnessStatus::Failed,
                    refuted: vec![property(
                        "index out of bounds: the length is less than or equal to the given index",
                        13,
                        "at",
                        &["4ul"],
                    )],
                },
                RawHarness {
                    name: "proofs::midpoint_lies_between".to_owned(),
                    status: HarnessStatus::Verified,
                    refuted: Vec::new(),
                },
                RawHarness {
                    name: "proofs::average_never_overflows".to_owned(),
                    status: HarnessStatus::Failed,
                    refuted: vec![property(
                        "attempt to add with overflow",
                        3,
                        "average",
                        &["128", "128"],
                    )],
                },
            ]
        );
    }

    #[test]
    fn a_replay_is_the_playback_test_kani_wrote() {
        let read = read(&Capture::of("model-check/kani", "harnesses").file("output.txt"));
        let replay = read.harnesses[2].refuted[0]
            .counterexample
            .as_ref()
            .and_then(|counterexample| counterexample.replay.clone())
            .unwrap();
        assert_eq!(
            replay,
            "/// Test generated for harness `proofs::average_never_overflows`\n\
             ///\n\
             /// Check for `assertion`: \"attempt to add with overflow\"\n\
             \n\
             #[test]\n\
             fn kani_concrete_playback_average_never_overflows_2036582041107130434() {\n    \
             let concrete_vals: Vec<Vec<u8>> = vec![\n        \
             // 128\n        \
             vec![128],\n        \
             // 128\n        \
             vec![128],\n    \
             ];\n    \
             kani::concrete_playback_run(concrete_vals, average_never_overflows);\n\
             }\n"
        );
    }

    #[test]
    fn each_playback_goes_to_the_check_it_names() {
        let read = read(&Capture::of("model-check/kani", "two-refuted").file("output.txt"));
        let harness = read
            .harnesses
            .iter()
            .find(|harness| harness.name == "proofs::midpoint_lies_between")
            .unwrap();
        let refuted: Vec<(&str, u32, Vec<String>)> = harness
            .refuted
            .iter()
            .map(|property| {
                (
                    property.description.as_str(),
                    property.line,
                    property.counterexample.clone().unwrap().values,
                )
            })
            .collect();
        assert_eq!(
            refuted,
            [
                ("\"below half\"", 32, vec!["0".to_owned(), "254".to_owned()]),
                ("\"positive\"", 33, vec!["0".to_owned(), "0".to_owned()]),
            ]
        );
        assert_eq!(
            harness.refuted[0].function.as_deref(),
            Some("proofs::midpoint_lies_between")
        );
    }

    #[test]
    fn a_build_error_fails_the_run_and_no_harness_is_not_a_failure() {
        let read = read(&Capture::of("model-check/kani", "broken").file("output.txt"));
        assert_eq!(
            read.failed.as_deref(),
            Some("error[E0277]: cannot divide `u8` by `&str`")
        );
        assert!(read.harnesses.is_empty());
        let none = "No proof harnesses (functions with #[kani::proof]) were found to verify.\n";
        assert_eq!(super::read(none), Read::default());
        assert_eq!(
            super::read("Kani Rust Verifier 0.68.0\nsegfault\n")
                .failed
                .as_deref(),
            Some("Kani Rust Verifier 0.68.0\nsegfault")
        );
    }

    #[test]
    fn a_harness_is_found_after_its_proof_attribute() {
        let capture = Capture::of("model-check/kani", "harnesses");
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), capture.file("lib.rs")).unwrap();
        assert_eq!(
            defined(dir.path(), "proofs::midpoint_lies_between"),
            Some(("src/lib.rs".to_owned(), 28))
        );
        assert_eq!(defined(dir.path(), "proofs::midpoint"), None);
    }

    #[test]
    fn a_verdict_kani_cannot_decide_is_undetermined() {
        let read = read("Checking harness proofs::spins...\n\nVERIFICATION:- UNDETERMINED\n");
        assert_eq!(read.harnesses[0].status, HarnessStatus::Undetermined);
        assert_eq!(read.failed, None);
    }
}
