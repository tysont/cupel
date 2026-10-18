// ABOUTME: `tool.dafny` (ADR-118): `dafny verify --json-output` over each project root's `.dfy` files, its diagnostics
// ABOUTME: read into obligations: errors it could not prove are failed; assumptions and exhausted resources are open.
use std::path::Path;

use cupel_core::{
    Availability, CostClass, Ctx, EventSink, FactKind, Install, Langs, Location, Obligation,
    ObligationStatus, Process, ProduceCtx, Producer, ProducerId, ProofResults, RelatedPlace,
    Result, Scope, ToolProbe, output_tail,
};
use serde_json::Value;

use crate::fuzz::{extension_is, sources};
use crate::runner::{on_path, probe, root_run, suite_roots};
use crate::tree::{canonical, relative, under};

const ID: &str = "tool.dafny";

/// Verifies the Dafny proofs of a language's project roots.
pub struct Dafny;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<ProofResults, _>(Dafny),
});

impl Producer<ProofResults> for Dafny {
    fn id(&self) -> ProducerId {
        ProducerId::from(ID)
    }

    /// Every language: a project's Dafny proofs are verified whatever the code beside them is
    /// written in (Dafny compiles to several of them).
    fn languages(&self) -> Langs {
        Langs::Any
    }

    fn needs(&self) -> &[FactKind] {
        &[FactKind::ProjectInfo]
    }

    fn cost(&self) -> CostClass {
        CostClass::Exhaustive
    }

    fn availability(&self, _: &Ctx) -> Availability {
        on_path(
            "dafny",
            Install::Hint(
                "Download a Dafny release (https://github.com/dafny-lang/dafny/releases), which \
                 bundles Z3, and put `dafny` on PATH; or `dotnet tool install --global dafny`."
                    .to_owned(),
            ),
        )
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        Some(probe("dafny", false, &["dafny", "--version"], &ctx.root))
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<ProofResults> {
        let root = canonical(&ctx.ctx.root);
        let mut fact = ProofResults {
            tool: self.id(),
            runs: Vec::new(),
            files: Vec::new(),
            verified: 0,
            obligations: Vec::new(),
        };
        for (dir, _) in suite_roots(ctx) {
            if !holds_scope(scope, &dir) {
                continue;
            }
            let path = canonical(&root.join(&dir));
            let files: Vec<String> = sources(&path, |_, path| extension_is(path, "dfy"))
                .into_iter()
                .map(|(relative, _)| relative)
                .filter(|file| !ctx.ctx.exclude.excludes(&under(&dir, file)))
                .collect();
            if files.is_empty() {
                continue;
            }
            ctx.cancel.checkpoint()?;
            let ran = Process::new("dafny", &path)
                .args(["verify", "--json-output", "--error-limit", "0"])
                .args(files.iter().map(String::as_str))
                .merged()
                .run(&ctx.cancel)?;
            let read = read(&ran.stdout, &|file| relative(&root, &path, file));
            fact.runs.push(root_run(dir.clone(), read.failed));
            fact.files
                .extend(files.iter().map(|file| under(&dir, file)));
            fact.verified += read.verified;
            fact.obligations.extend(read.obligations);
        }
        fact.files.sort();
        // The verifier works in parallel, so its order is not the files'.
        fact.obligations.sort_by(|a, b| {
            let key = |o: &Obligation| (o.location.file.clone(), o.location.line, o.location.col);
            key(a).cmp(&key(b)).then_with(|| a.message.cmp(&b.message))
        });
        Ok(fact)
    }
}

/// Whether the project root `dir` holds a file of the scope; every root does, for an empty
/// scope. Proof files are not the language's, so any scoped file under the root counts.
fn holds_scope(scope: &Scope, dir: &str) -> bool {
    scope.files.is_empty()
        || dir == "."
        || scope
            .files
            .iter()
            .any(|file| Path::new(file).starts_with(dir))
}

/// What one `dafny verify` run said.
#[derive(Debug, Default, PartialEq, Eq)]
struct Read {
    verified: u64,
    obligations: Vec<Obligation>,
    /// Why the run did not verify: the files did not parse or resolve, or its output is not
    /// Dafny's JSON.
    failed: Option<String>,
}

/// Reads `dafny verify --json-output`: one JSON object per line, each a `diagnostic` or the
/// closing `status`. `place` makes a file Dafny names relative to the repository root.
fn read(output: &str, place: &dyn Fn(&str) -> Option<String>) -> Read {
    let mut read = Read::default();
    let mut problems = Vec::new();
    let mut status = None;
    for value in output
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
    {
        match value["type"].as_str() {
            Some("status") => status = value["value"].as_str().map(str::to_owned),
            Some("diagnostic") => {
                let diagnostic = &value["value"];
                let message = message(diagnostic);
                let Some(location) = location(&diagnostic["location"], place) else {
                    continue;
                };
                let verifier = diagnostic["source"].as_str() == Some("Verifier");
                match (diagnostic["severity"].as_u64(), verifier) {
                    (Some(1), true) => read.obligations.push(Obligation {
                        status: if undecided(&message) {
                            ObligationStatus::Open
                        } else {
                            ObligationStatus::Failed
                        },
                        message,
                        location,
                        related: related(diagnostic, place),
                    }),
                    (Some(2), true) if assumed(&message) => read.obligations.push(Obligation {
                        status: ObligationStatus::Open,
                        message,
                        location,
                        related: related(diagnostic, place),
                    }),
                    (Some(1), false) => problems.push(format!(
                        "{}:{}:{}: {message}",
                        location.file,
                        location.line,
                        location.col.unwrap_or(1)
                    )),
                    _ => {}
                }
            }
            _ => {}
        }
    }
    read.verified = status.as_deref().and_then(verified).unwrap_or(0);
    read.failed = if !problems.is_empty() {
        Some(problems.join("; "))
    } else if status.is_none() {
        Some(output_tail(output).unwrap_or_else(|| "dafny printed nothing".to_owned()))
    } else {
        None
    };
    read
}

/// A diagnostic's message: its format with each `{i}` filled from its arguments.
fn message(diagnostic: &Value) -> String {
    let mut message = diagnostic["defaultFormatMessage"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    if let Some(arguments) = diagnostic["arguments"].as_array() {
        for (index, argument) in arguments.iter().enumerate() {
            let text = argument
                .as_str()
                .map_or_else(|| argument.to_string(), str::to_owned);
            message = message.replace(&format!("{{{index}}}"), &text);
        }
    }
    message
}

/// Whether an error says the verifier ran out of time or resources rather than refuting.
fn undecided(message: &str) -> bool {
    message.contains("out of resource") || message.contains("timed out")
}

/// Whether a warning says the proof assumes instead of proving: an `assume` without `{:axiom}`,
/// a postcondition of a body-less method or lemma.
fn assumed(message: &str) -> bool {
    message.contains("assume statement") || message.contains("bodyless")
}

/// A diagnostic `location` (`filePath`, a 1-based line and 0-based characters) in the
/// repository.
fn location(location: &Value, place: &dyn Fn(&str) -> Option<String>) -> Option<Location> {
    let file = place(location["filePath"].as_str()?)?;
    let number = |pointer: &str| {
        location
            .pointer(pointer)
            .and_then(Value::as_u64)
            .and_then(|n| u32::try_from(n).ok())
    };
    let line = number("/range/start/line")?;
    Some(Location {
        file,
        line,
        col: number("/range/start/character").map(|c| c + 1),
        end_line: number("/range/end/line").unwrap_or(line),
        end_col: number("/range/end/character").map(|c| c + 1),
    })
}

fn related(diagnostic: &Value, place: &dyn Fn(&str) -> Option<String>) -> Vec<RelatedPlace> {
    diagnostic["relatedInformation"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|related| {
            Some(RelatedPlace {
                message: message(related),
                location: location(&related["location"], place)?,
            })
        })
        .collect()
}

/// The count before ` verified` in Dafny's closing status.
fn verified(status: &str) -> Option<u64> {
    let (before, _) = status.split_once(" verified")?;
    before.split_whitespace().last()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;

    fn read_capture(case: &str) -> Read {
        let capture = Capture::of("proof/dafny", case);
        read(&capture.file("output.jsonl"), &|file| Some(file.to_owned()))
    }

    fn at(line: u32, col: u32, end_col: u32) -> Location {
        Location {
            file: "proofs/tally.dfy".to_owned(),
            line,
            col: Some(col),
            end_line: line,
            end_col: Some(end_col),
        }
    }

    #[test]
    fn errors_are_failed_and_assumptions_are_open_obligations() {
        let read = read_capture("obligations");
        assert_eq!(read.failed, None);
        assert_eq!(read.verified, 3);
        assert_eq!(
            read.obligations,
            [
                Obligation {
                    status: ObligationStatus::Open,
                    message: "assume statement has no {:axiom} annotation".to_owned(),
                    location: at(31, 3, 9),
                    related: Vec::new(),
                },
                Obligation {
                    status: ObligationStatus::Open,
                    message: "This ensures clause is part of a bodyless method. Add the {:axiom} \
                              attribute to it or the enclosing method to suppress this warning"
                        .to_owned(),
                    location: at(35, 17, 19),
                    related: Vec::new(),
                },
                Obligation {
                    status: ObligationStatus::Failed,
                    message: "a postcondition could not be proved on this return path".to_owned(),
                    location: at(24, 1, 2),
                    related: vec![RelatedPlace {
                        message: "this is the postcondition that could not be proved".to_owned(),
                        location: at(23, 23, 25),
                    }],
                },
            ]
        );
    }

    #[test]
    fn exhausted_resources_leave_an_obligation_open() {
        let read = read_capture("out-of-resource");
        assert_eq!(read.verified, 0);
        let open: Vec<(String, ObligationStatus)> = read
            .obligations
            .iter()
            .skip(2)
            .map(|obligation| (obligation.message.clone(), obligation.status))
            .collect();
        let expected: Vec<(String, ObligationStatus)> = ["Sum", "Total", "Max", "SumOfTwo"]
            .iter()
            .map(|name| {
                (
                    format!("Verification out of resource ({name})"),
                    ObligationStatus::Open,
                )
            })
            .collect();
        assert_eq!(open, expected);
    }

    #[test]
    fn a_file_that_does_not_resolve_fails_the_run_with_its_errors() {
        let read = read_capture("unresolved");
        assert_eq!(
            read.failed.as_deref(),
            Some("proofs/tally.dfy:5:36: unresolved identifier: Summ")
        );
        assert_eq!(read.verified, 0);
        assert!(
            read.obligations
                .iter()
                .all(|obligation| obligation.status == ObligationStatus::Open)
        );
    }

    #[test]
    fn output_that_is_not_dafnys_fails_the_run_and_places_outside_are_dropped() {
        let none = |_: &str| None;
        assert_eq!(
            read("dafny: command crashed\n", &none).failed.as_deref(),
            Some("dafny: command crashed")
        );
        let capture = Capture::of("proof/dafny", "obligations");
        let outside = read(&capture.file("output.jsonl"), &none);
        assert_eq!((outside.verified, outside.obligations.len()), (3, 0));
    }

    #[test]
    fn a_scope_holds_a_root_by_any_file_under_it() {
        let scope = Scope::of_files(vec!["tools/a.go".to_owned()]);
        assert_eq!(
            [
                holds_scope(&scope, "tools"),
                holds_scope(&scope, "web"),
                holds_scope(&scope, "."),
                holds_scope(&Scope::default(), "web"),
            ],
            [true, false, true, true]
        );
    }
}
