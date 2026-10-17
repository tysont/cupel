// ABOUTME: `tool.pytest-benchmark` (ADR-106): `python3 -m pytest --benchmark-only` per Python root with a minimum round
// ABOUTME: count and no warmup, its JSON report's raw round times the samples, storage and bytecode kept out of the tree.
use cupel_core::{Availability, Benchmark, Cancellation, Ctx, FactKind, LangId, Process, Result};
use serde::Deserialize;

use super::{BenchTool, Measured, Side, execute, missing};
use crate::runner::installed;

pub static PYTEST_BENCHMARK: BenchTool = BenchTool {
    id: "tool.pytest-benchmark",
    langs: &["python"],
    program: "python3",
    version: &["python3", "-m", "pytest", "--version"],
    needs: &[FactKind::ProjectInfo],
    wants: &[],
    per_root: true,
    availability,
    measure,
};

/// The plugin as pip names it.
const PLUGIN: &str = "pytest-benchmark";

/// Ready when `python3` imports the plugin; else the plugin is missing.
fn availability(tool: &BenchTool, ctx: &Ctx, _: &LangId) -> Availability {
    let imports = installed(tool.program, false, &ctx.root).is_some()
        && Process::new(tool.program, &ctx.root)
            .args(["-c", "import pytest_benchmark"])
            .run(&Cancellation::new())
            .is_ok_and(|ran| ran.exit_code == Some(0));
    if imports {
        Availability::Ready
    } else {
        missing(tool, PLUGIN, ctx)
    }
}

fn measure(side: &Side<'_>) -> Result<Measured> {
    let report = side.scratch.join("report.json");
    let process = Process::new("python3", side.root())
        .args([
            "-m",
            "pytest",
            "-p",
            "no:cacheprovider",
            "--benchmark-only",
            "-q",
        ])
        .args([
            format!("--benchmark-json={}", report.display()),
            format!("--benchmark-min-rounds={}", side.samples),
            "--benchmark-max-time=0.000001".to_owned(),
            "--benchmark-warmup=off".to_owned(),
            format!(
                "--benchmark-storage=file://{}",
                side.scratch.join("storage").display()
            ),
        ])
        .env("PYTHONDONTWRITEBYTECODE", "1");
    Ok(match execute(side, &process)? {
        Ok(captured) => {
            let text = std::fs::read_to_string(&report).unwrap_or_default();
            let mut benchmarks = parse(&text, side.samples);
            for benchmark in &mut benchmarks {
                place(side, benchmark);
            }
            Measured::of(benchmarks, &captured)
        }
        Err(detail) => Measured::failed(detail),
    })
}

/// The report: each benchmark's node ID and raw round times, in seconds.
#[derive(Deserialize)]
struct Report {
    benchmarks: Vec<Measurement>,
}

#[derive(Deserialize)]
struct Measurement {
    fullname: String,
    stats: Stats,
}

#[derive(Deserialize)]
struct Stats {
    data: Vec<f64>,
}

/// Every benchmark of the report `text`, its round times in ns; at most `samples` of them,
/// the first rounds, so both sides compare the same number.
pub fn parse(text: &str, samples: u32) -> Vec<Benchmark> {
    let Ok(report) = serde_json::from_str::<Report>(text) else {
        return Vec::new();
    };
    report
        .benchmarks
        .into_iter()
        .map(|measured| Benchmark {
            name: measured.fullname,
            unit: "ns".to_owned(),
            file: None,
            line: None,
            samples: measured
                .stats
                .data
                .iter()
                .take(samples as usize)
                .map(|seconds| seconds * 1e9)
                .collect(),
        })
        .collect()
}

/// Places `benchmark` at the test function its node ID names, in the file it names.
fn place(side: &Side<'_>, benchmark: &mut Benchmark) {
    let name = side.file(&benchmark.name);
    let Some((file, test)) = name.split_once("::") else {
        return;
    };
    let function = test.rsplit("::").next().unwrap_or(test);
    let function = function.split('[').next().unwrap_or(function);
    let prefix = format!("def {function}(");
    if let Some((_, line)) = side.define(&prefix, |candidate| candidate == file) {
        benchmark.line = Some(line);
    }
    benchmark.file = Some(file.to_owned());
    benchmark.name = name;
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;

    #[test]
    fn each_round_time_is_a_sample_in_nanoseconds() {
        let capture = Capture::of("pytest-benchmark", "green");
        let benchmarks = parse(&capture.file("report.json"), 5);
        assert_eq!(benchmarks.len(), 1);
        let benchmark = &benchmarks[0];
        assert_eq!(benchmark.name, "tests/test_bench.py::test_total");
        assert_eq!(benchmark.unit, "ns");
        assert_eq!(benchmark.samples.len(), 5);
        assert!(
            (benchmark.samples[0] - 302.103_580_906_987_2).abs() < 1e-6,
            "{:?}",
            benchmark.samples
        );
        assert_eq!(parse(&capture.file("report.json"), 2)[0].samples.len(), 2);
    }

    #[test]
    fn a_missing_or_unreadable_report_has_no_benchmark() {
        assert!(parse("", 5).is_empty());
        assert!(parse("{\"benchmarks\": 3}", 5).is_empty());
    }
}
