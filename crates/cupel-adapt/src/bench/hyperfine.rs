// ABOUTME: `tool.hyperfine` (ADR-106): times the commands `[lang.<id>] bench_commands` lists, from the repository
// ABOUTME: root, with one warmup run; each run's wall time a sample of the benchmark the command names.
use cupel_core::{Availability, Benchmark, Ctx, LangId, Process, Result};
use serde::Deserialize;
use serde_json::Value;

use super::{BenchTool, Measured, Side, execute, missing};
use crate::runner::installed;

pub static HYPERFINE: BenchTool = BenchTool {
    id: "tool.hyperfine",
    langs: &["javascript", "typescript", "python"],
    program: "hyperfine",
    version: &["hyperfine", "--version"],
    needs: &[],
    wants: &[],
    per_root: false,
    availability,
    measure,
};

/// The `[lang.<id>]` key listing the commands to time.
pub const COMMANDS_KEY: &str = "bench_commands";

/// The commands a `[lang.<id>]` table lists under `bench_commands`.
fn commands(table: Option<&serde_json::Map<String, Value>>) -> Vec<String> {
    table
        .and_then(|table| table.get(COMMANDS_KEY))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// Unconfigured without commands to time; else ready when hyperfine is on `PATH`.
fn availability(tool: &BenchTool, ctx: &Ctx, lang: &LangId) -> Availability {
    if commands(ctx.lang.get(lang)).is_empty() {
        Availability::Unconfigured {
            key: format!("lang.{lang}.{COMMANDS_KEY}"),
        }
    } else if installed(tool.program, false, &ctx.root).is_some() {
        Availability::Ready
    } else {
        missing(tool, tool.program, ctx)
    }
}

fn measure(side: &Side<'_>) -> Result<Measured> {
    let commands = commands(Some(&side.ctx.config));
    if commands.is_empty() {
        return Ok(Measured::default());
    }
    let report = side.scratch.join("report.json");
    let process = Process::new("hyperfine", side.root())
        .args([
            "--runs".to_owned(),
            side.samples.to_string(),
            "--warmup".to_owned(),
            "1".to_owned(),
            "--style".to_owned(),
            "none".to_owned(),
            "--export-json".to_owned(),
            report.to_string_lossy().into_owned(),
        ])
        .args(commands);
    Ok(match execute(side, &process)? {
        Ok(captured) => {
            let text = std::fs::read_to_string(&report).unwrap_or_default();
            Measured::of(parse(&text), &captured)
        }
        Err(detail) => Measured::failed(detail),
    })
}

/// The report: each command and its runs' wall times, in seconds.
#[derive(Deserialize)]
struct Report {
    results: Vec<Timed>,
}

#[derive(Deserialize)]
struct Timed {
    command: String,
    times: Vec<f64>,
}

/// Every command the report `text` timed, each run's time in ns.
pub fn parse(text: &str) -> Vec<Benchmark> {
    let Ok(report) = serde_json::from_str::<Report>(text) else {
        return Vec::new();
    };
    report
        .results
        .into_iter()
        .map(|timed| Benchmark {
            name: timed.command,
            unit: "ns".to_owned(),
            file: None,
            line: None,
            samples: timed.times.iter().map(|seconds| seconds * 1e9).collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;
    use serde_json::json;

    use super::*;

    #[test]
    fn each_run_s_wall_time_is_a_sample_of_its_command_in_nanoseconds() {
        let benchmarks = parse(&Capture::of("hyperfine", "green").file("report.json"));
        assert_eq!(benchmarks.len(), 1);
        assert_eq!(benchmarks[0].name, "node bench.js");
        assert_eq!(benchmarks[0].unit, "ns");
        let samples = &benchmarks[0].samples;
        assert_eq!(samples.len(), 3);
        assert!((samples[1] - 56_055_651.56).abs() < 1e-3, "{samples:?}");
        assert!(parse("").is_empty());
    }

    #[test]
    fn bench_commands_must_be_listed_to_be_timed() {
        let table = |value: Value| value.as_object().cloned().unwrap();
        assert_eq!(
            commands(Some(&table(
                json!({"bench_commands": ["node a.js", 3, "node b.js"]})
            ))),
            ["node a.js", "node b.js"]
        );
        assert!(commands(Some(&table(json!({"bench_commands": "node a.js"})))).is_empty());
        assert!(commands(None).is_empty());
        let mut ctx = Ctx::new(std::path::PathBuf::from("/repo"));
        let lang = LangId::from("javascript");
        assert_eq!(
            availability(&HYPERFINE, &ctx, &lang),
            Availability::Unconfigured {
                key: "lang.javascript.bench_commands".to_owned()
            }
        );
        ctx.lang.insert(
            lang.clone(),
            table(json!({"bench_commands": ["node a.js"]})),
        );
        assert_ne!(
            availability(&HYPERFINE, &ctx, &lang),
            Availability::Unconfigured {
                key: "lang.javascript.bench_commands".to_owned()
            }
        );
    }
}
