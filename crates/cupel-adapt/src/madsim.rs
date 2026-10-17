// ABOUTME: `tool.madsim` (ADR-120): runs the `#[madsim::test]` harnesses of every package depending on madsim over the
// ABOUTME: run's seed range under `--cfg madsim`, then replays each failing seed alone for its trace and to confirm it.
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cupel_core::{
    Availability, Captured, CostClass, Ctx, EventSink, FactKind, LangId, Langs, Location, Process,
    ProduceCtx, Producer, ProducerId, Result, RootRun, RunStatus, Scope, SimulationFailure,
    SimulationHarness, SimulationResults, SimulationStatus, ToolId, ToolProbe,
};
use serde_json::Value;

use crate::analyze::extensions;
use crate::fuzz::cargo_fuzz::panic;
use crate::fuzz::{in_scope, sources};
use crate::race::cargo::{failure, progress};
use crate::runner::{installed, suite_roots};
use crate::{toolchain, tree};

const ID: &str = "tool.madsim";

/// The flag that builds madsim's simulator instead of its tokio passthrough.
const RUSTFLAGS: &str = "--cfg madsim";

/// The most trace lines a failure keeps, the last ones.
const TRACE_LINES: usize = 50;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<SimulationResults, _>(Madsim),
});

/// madsim's simulation harnesses, run for Rust.
pub struct Madsim;

/// The seeds a campaign runs: from the run's seed, as many as `--cases` (else the depth's case
/// budget), kept below `u64::MAX` so madsim's range does not overflow.
fn range(seed: u64, cases: u64) -> (u64, u64) {
    (seed.min(u64::MAX - cases), cases)
}

impl Producer<SimulationResults> for Madsim {
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

    fn availability(&self, ctx: &Ctx) -> Availability {
        if installed("cargo", false, &ctx.root).is_some() {
            Availability::Ready
        } else {
            Availability::MissingTool {
                name: ToolId::from("cargo"),
                install: toolchain::install(&self.id(), "cargo", &ctx.root),
            }
        }
    }

    fn tool(&self, _: &Ctx) -> Option<ToolProbe> {
        Some(ToolProbe {
            name: ToolId::from("cargo"),
            version: vec!["cargo".to_owned(), "--version".to_owned()],
        })
    }

    fn produce(
        &self,
        ctx: &ProduceCtx,
        scope: &Scope,
        _: &mut dyn EventSink,
    ) -> Result<SimulationResults> {
        let (seed, seeds) = range(ctx.seed, u64::from(ctx.focus.case_budget()));
        let mut results = SimulationResults {
            tool: self.id(),
            seed,
            seeds,
            runs: Vec::new(),
            harnesses: Vec::new(),
        };
        let target_dir = ctx
            .writer
            .cache_dir()
            .join("dst")
            .join("madsim")
            .join("target");
        for (dir, _) in suite_roots(ctx) {
            if !in_scope(scope, &dir, extensions(&LangId::from("rust"))) {
                continue;
            }
            ctx.cancel.checkpoint()?;
            let campaign = Campaign {
                ctx,
                path: ctx.ctx.root.join(&dir),
                dir,
                target_dir: target_dir.clone(),
                seed,
                seeds,
            };
            let (run, harnesses) = campaign.run()?;
            results.runs.push(run);
            results.harnesses.extend(harnesses);
        }
        Ok(results)
    }
}

/// One project root's campaign.
struct Campaign<'a> {
    ctx: &'a ProduceCtx,
    /// Relative to the repository root.
    dir: String,
    path: PathBuf,
    target_dir: PathBuf,
    seed: u64,
    seeds: u64,
}

/// A test target of a package that depends on madsim: `--lib`, or `--test <name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Target {
    package: String,
    selector: Vec<String>,
}

/// A `#[madsim::test]` function: its name, and where it is defined relative to the project root.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Defined {
    name: String,
    file: String,
    line: u32,
}

impl Campaign<'_> {
    /// Runs every target of every madsim package of the root over the seed range.
    fn run(&self) -> Result<(RootRun, Vec<SimulationHarness>)> {
        let mut harnesses = Vec::new();
        let mut failed = None;
        let metadata = self.cargo(&[
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ]);
        let captured = metadata.run(&self.ctx.cancel)?;
        if captured.exit_code != Some(0) {
            return Ok((self.root_run(Some(output(&captured))), harnesses));
        }
        for (package_dir, targets) in packages(&captured.stdout) {
            let defined = defined(&self.path, &package_dir);
            for target in targets {
                self.ctx.cancel.checkpoint()?;
                let captured = self
                    .test(&target, None, self.seed, self.seeds)
                    .run(&self.ctx.cancel)?;
                let text = output(&captured);
                let ran = outcomes(&text);
                if let Some(problem) = failure(&text, !ran.is_empty()) {
                    failed = Some(format!("{}: {problem}", target.selector.join(" ")));
                }
                for (name, passed) in ran {
                    let Some(defined) = harness(&defined, &name) else {
                        continue;
                    };
                    let failure = if passed {
                        None
                    } else {
                        Some(self.failure(&target, &name, &text)?)
                    };
                    harnesses.push(SimulationHarness {
                        root: self.dir.clone(),
                        location: Some(self.location(&defined.file, defined.line, None)),
                        name,
                        status: if passed {
                            SimulationStatus::Passed
                        } else {
                            SimulationStatus::Failed
                        },
                        failure,
                    });
                }
            }
        }
        harnesses.sort_by(|a, b| a.name.cmp(&b.name));
        Ok((self.root_run(failed), harnesses))
    }

    fn root_run(&self, failed: Option<String>) -> RootRun {
        RootRun {
            dir: self.dir.clone(),
            status: if failed.is_some() {
                RunStatus::Failed
            } else {
                RunStatus::Ran
            },
            detail: failed,
        }
    }

    /// The failure of harness `name` in the campaign's `text`, replayed alone at its seed.
    fn failure(&self, target: &Target, name: &str, text: &str) -> Result<SimulationFailure> {
        let printed = section(text, name);
        let seed = failing_seed(&printed);
        let mut replayed = None;
        if let Some(seed) = seed {
            let replay = self
                .test(target, Some(name), seed, 1)
                .run(&self.ctx.cancel)?;
            let replay = output(&replay);
            if outcomes(&replay).contains(&(name.to_owned(), false)) {
                replayed = Some(section(&replay, name));
            }
        }
        let reproduced = replayed.is_some();
        let shown = replayed.unwrap_or(printed);
        let (message, location) = panic(&self.path, &shown.join("\n"))
            .unwrap_or_else(|| ("the harness failed".to_owned(), None));
        Ok(SimulationFailure {
            seed,
            message,
            location: location.map(|at| self.location(&at.file, at.line, at.col)),
            trace: trace(&shown, &self.path),
            reproduced,
            reproduce: reproduce(target, name, seed),
        })
    }

    fn location(&self, file: &str, line: u32, col: Option<u32>) -> Location {
        Location {
            file: tree::under(&self.dir, file),
            line,
            col,
            end_line: line,
            end_col: None,
        }
    }

    fn cargo(&self, args: &[&str]) -> Process {
        Process::new("cargo", &self.path)
            .args(args.iter().copied())
            .merged()
    }

    /// `cargo test` of `target` (only the test `name`, when given) over `seeds` seeds from
    /// `seed`, built with madsim's simulator into the cache.
    fn test(&self, target: &Target, name: Option<&str>, seed: u64, seeds: u64) -> Process {
        let mut args = vec!["test".to_owned(), "-p".to_owned(), target.package.clone()];
        args.extend(target.selector.iter().cloned());
        args.push("--no-fail-fast".to_owned());
        if let Some(name) = name {
            args.extend([name.to_owned(), "--".to_owned(), "--exact".to_owned()]);
        }
        Process::new("cargo", &self.path)
            .args(args)
            .env("RUSTFLAGS", RUSTFLAGS)
            .env("CARGO_TARGET_DIR", &self.target_dir.to_string_lossy())
            .env("MADSIM_TEST_SEED", &seed.to_string())
            .env("MADSIM_TEST_NUM", &seeds.to_string())
            .merged()
    }
}

/// What `captured` printed (merged).
fn output(captured: &Captured) -> String {
    format!("{}{}", captured.stdout, captured.stderr)
}

/// Each package of `cargo metadata` output that depends on madsim, with its directory relative
/// to the project root and its library and test targets.
fn packages(metadata: &str) -> Vec<(String, Vec<Target>)> {
    let Ok(metadata) = serde_json::from_str::<Value>(metadata) else {
        return Vec::new();
    };
    let root = metadata["workspace_root"].as_str().unwrap_or_default();
    let list = |value: &Value| value.as_array().cloned().unwrap_or_default();
    list(&metadata["packages"])
        .iter()
        .filter(|package| {
            list(&package["dependencies"])
                .iter()
                .any(|dependency| dependency["name"] == "madsim")
        })
        .map(|package| {
            let name = package["name"].as_str().unwrap_or_default().to_owned();
            let manifest = Path::new(package["manifest_path"].as_str().unwrap_or_default());
            let dir = manifest
                .parent()
                .and_then(|dir| dir.strip_prefix(root).ok())
                .map_or_else(String::new, |dir| dir.to_string_lossy().replace('\\', "/"));
            let targets = list(&package["targets"])
                .iter()
                .filter_map(|target| {
                    let kinds = list(&target["kind"]);
                    let selector = if kinds.iter().any(|kind| kind == "lib") {
                        vec!["--lib".to_owned()]
                    } else if kinds.iter().any(|kind| kind == "test") {
                        vec!["--test".to_owned(), target["name"].as_str()?.to_owned()]
                    } else {
                        return None;
                    };
                    Some(Target {
                        package: name.clone(),
                        selector,
                    })
                })
                .collect();
            (dir, targets)
        })
        .collect()
}

/// Every `#[madsim::test]` function under the package directory `package` of the project root
/// `root`, with its file relative to the root.
fn defined(root: &Path, package: &str) -> Vec<Defined> {
    let dir = root.join(package);
    sources(&dir, |_, path| {
        path.extension().is_some_and(|ext| ext == "rs")
    })
    .into_iter()
    .flat_map(|(relative, path)| {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        harness_fns(&text)
            .into_iter()
            .map(|(name, line)| Defined {
                name,
                file: if package.is_empty() {
                    relative.clone()
                } else {
                    format!("{package}/{relative}")
                },
                line,
            })
            .collect::<Vec<_>>()
    })
    .collect()
}

/// The functions `text` marks `#[madsim::test]`, with the line of each `fn`.
fn harness_fns(text: &str) -> Vec<(String, u32)> {
    let mut found = Vec::new();
    let mut marked = false;
    for (index, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.starts_with("#[madsim::test") {
            marked = true;
        } else if marked && !line.starts_with("#[") && !line.starts_with("//") {
            let name = line
                .split_once("fn ")
                .map(|(_, rest)| rest.split(['(', '<']).next().unwrap_or_default().trim());
            if let Some(name) = name.filter(|name| !name.is_empty()) {
                found.push((
                    name.to_owned(),
                    u32::try_from(index + 1).unwrap_or(u32::MAX),
                ));
            }
            marked = false;
        }
    }
    found
}

/// The definition of the test libtest names `name` (`module::function`), when exactly one
/// `#[madsim::test]` function has its last segment's name.
fn harness<'a>(defined: &'a [Defined], name: &str) -> Option<&'a Defined> {
    let function = name.rsplit("::").next().unwrap_or(name);
    let mut found = defined.iter().filter(|defined| defined.name == function);
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

/// Each test libtest ran in `text` and whether it passed; ignored tests are left out.
fn outcomes(text: &str) -> Vec<(String, bool)> {
    let mut ran: BTreeMap<String, bool> = BTreeMap::new();
    for line in text.lines() {
        if let Some((name, after)) = progress(line) {
            if after.starts_with("ok") {
                ran.insert(name.to_owned(), true);
            } else if after.starts_with("FAILED") {
                ran.insert(name.to_owned(), false);
            }
        }
    }
    ran.into_iter().collect()
}

/// The lines libtest printed for the failed test `name`, between its `---- name stdout ----`
/// header and the next header or the closing `failures:` list.
fn section(text: &str, name: &str) -> Vec<String> {
    let header = format!("---- {name} stdout ----");
    text.lines()
        .skip_while(|line| line.trim() != header)
        .skip(1)
        .take_while(|line| !line.starts_with("---- ") && line.trim() != "failures:")
        .map(str::to_owned)
        .collect()
}

/// The seed madsim's note names: ``run with `MADSIM_TEST_SEED=<n>` environment variable to
/// reproduce this error``.
fn failing_seed(section: &[String]) -> Option<u64> {
    section.iter().find_map(|line| {
        let (_, after) = line.split_once("MADSIM_TEST_SEED=")?;
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

/// `section` as a failure's trace: without libtest's and madsim's notes, thread numbers masked,
/// the project root's paths relative, places outside it dropped, blank edges trimmed; the last
/// [`TRACE_LINES`] lines.
fn trace(section: &[String], root: &Path) -> Vec<String> {
    let root = format!("{}/", root.to_string_lossy());
    let lines: Vec<String> = section
        .iter()
        .filter(|line| !line.starts_with("note: "))
        .map(|line| masked(&line.replace(&root, "")))
        .collect();
    let start = lines.iter().position(|line| !line.trim().is_empty());
    let end = lines.iter().rposition(|line| !line.trim().is_empty());
    let (Some(start), Some(end)) = (start, end) else {
        return Vec::new();
    };
    let kept = &lines[start..=end];
    kept[kept.len().saturating_sub(TRACE_LINES)..].to_vec()
}

/// `line` with a panicking thread's number and an absolute `spawned at` place removed.
fn masked(line: &str) -> String {
    let mut line = line.to_owned();
    if let Some(rest) = line.strip_prefix("thread '")
        && let Some((thread, after)) = rest.split_once("' (")
        && let Some((number, tail)) = after.split_once(") ")
        && number.chars().all(|c| c.is_ascii_digit())
    {
        line = format!("thread '{thread}' {tail}");
    }
    if let Some((before, _)) = line.split_once(" (spawned at /") {
        line = before.to_owned();
    }
    line
}

/// The command, from the project root, that replays `seed` of harness `name` alone.
fn reproduce(target: &Target, name: &str, seed: Option<u64>) -> String {
    let seed = seed.map_or_else(String::new, |seed| {
        format!("MADSIM_TEST_SEED={seed} MADSIM_TEST_NUM=1 ")
    });
    format!(
        "RUSTFLAGS='{RUSTFLAGS}' {seed}cargo test -p {} {} {name} -- --exact",
        target.package,
        target.selector.join(" ")
    )
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;
    use serde_json::json;

    use super::*;

    fn sim() -> Target {
        Target {
            package: "ledger".to_owned(),
            selector: vec!["--test".to_owned(), "sim".to_owned()],
        }
    }

    #[test]
    fn a_campaign_names_each_test_s_outcome_and_the_first_failing_seed() {
        let output = Capture::of("madsim", "campaign").file("output.txt");
        assert_eq!(
            outcomes(&output),
            [
                ("a_withdrawal_waits_for_its_deposit".to_owned(), false),
                ("concurrent_deposits_are_all_kept".to_owned(), false),
                ("sequential_deposits_are_all_kept".to_owned(), true),
            ]
        );
        assert_eq!(
            failing_seed(&section(&output, "concurrent_deposits_are_all_kept")),
            Some(4)
        );
        assert_eq!(
            failing_seed(&section(&output, "a_withdrawal_waits_for_its_deposit")),
            Some(5)
        );
        assert_eq!(failure(&output, true), None);
    }

    #[test]
    fn a_replayed_seed_s_trace_is_what_it_printed_with_notes_and_machine_details_removed() {
        let capture = Capture::of("madsim", "replay");
        let shown = section(
            &capture.file("output.txt"),
            "concurrent_deposits_are_all_kept",
        );
        assert_eq!(
            trace(&shown, &capture.ran_in),
            [
                "deposit 10 reads 0",
                "deposit 20 reads 0",
                "deposit 10 writes 10",
                "deposit 20 writes 20",
                "",
                "thread '<unnamed>' panicked at tests/sim.rs:23:5:",
                "assertion `left == right` failed: a deposit was lost",
                "  left: 20",
                " right: 30",
                "context: node=0 \"madsim-main\", task=0",
            ]
        );
        let (message, _) = panic(&capture.ran_in, &shown.join("\n")).unwrap();
        assert_eq!(
            message,
            "assertion `left == right` failed: a deposit was lost"
        );
    }

    #[test]
    fn a_task_s_spawn_place_inside_the_repository_is_kept() {
        assert_eq!(
            masked("context: node=0 \"madsim-main\", task=16 (spawned at tests/sim.rs:45:22)"),
            "context: node=0 \"madsim-main\", task=16 (spawned at tests/sim.rs:45:22)"
        );
        assert_eq!(
            masked("thread 'main' (12) panicked at x"),
            "thread 'main' panicked at x"
        );
    }

    #[test]
    fn the_repro_replays_the_seed_alone() {
        assert_eq!(
            reproduce(&sim(), "lost", Some(4)),
            "RUSTFLAGS='--cfg madsim' MADSIM_TEST_SEED=4 MADSIM_TEST_NUM=1 cargo test -p ledger \
             --test sim lost -- --exact"
        );
        assert_eq!(
            reproduce(&sim(), "lost", None),
            "RUSTFLAGS='--cfg madsim' cargo test -p ledger --test sim lost -- --exact"
        );
    }

    #[test]
    fn harnesses_are_the_functions_marked_madsim_test() {
        let text = "#[madsim::test]\nasync fn kept() {}\n\n#[test]\nfn plain() {}\n\
                    #[madsim::test]\n#[should_panic]\npub async fn generic<T>() {}\n";
        assert_eq!(
            harness_fns(text),
            [("kept".to_owned(), 2), ("generic".to_owned(), 8)]
        );
        let defined = [
            Defined {
                name: "kept".to_owned(),
                file: "tests/a.rs".to_owned(),
                line: 2,
            },
            Defined {
                name: "twice".to_owned(),
                file: "tests/a.rs".to_owned(),
                line: 5,
            },
            Defined {
                name: "twice".to_owned(),
                file: "tests/b.rs".to_owned(),
                line: 5,
            },
        ];
        assert_eq!(harness(&defined, "sim::kept").map(|d| d.line), Some(2));
        assert_eq!(harness(&defined, "twice"), None);
        assert_eq!(harness(&defined, "plain"), None);
    }

    #[test]
    fn the_packages_are_those_depending_on_madsim_with_their_lib_and_test_targets() {
        let metadata = json!({
            "workspace_root": "/repo",
            "packages": [
                {"name": "ledger", "manifest_path": "/repo/Cargo.toml",
                 "dependencies": [{"name": "madsim"}],
                 "targets": [{"name": "ledger", "kind": ["lib"]}, {"name": "sim", "kind": ["test"]},
                             {"name": "tool", "kind": ["bin"]}]},
                {"name": "other", "manifest_path": "/repo/other/Cargo.toml",
                 "dependencies": [{"name": "serde"}], "targets": [{"name": "other", "kind": ["lib"]}]},
            ],
        });
        assert_eq!(
            packages(&metadata.to_string()),
            [(
                String::new(),
                vec![
                    Target {
                        package: "ledger".to_owned(),
                        selector: vec!["--lib".to_owned()]
                    },
                    sim()
                ]
            )]
        );
    }

    #[test]
    fn the_seed_range_never_overflows() {
        assert_eq!(range(3, 1000), (3, 1000));
        assert_eq!(range(u64::MAX, 10), (u64::MAX - 10, 10));
    }
}
