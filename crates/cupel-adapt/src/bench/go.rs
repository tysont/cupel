// ABOUTME: `tool.go-bench` (ADR-106): `go test -bench -benchmem -json` per Go root, each result line's ns/op, B/op, and
// ABOUTME: allocs/op a sample of its benchmark named with its package, placed at the one `_test.go` function of that name.
use cupel_core::{Benchmark, FactKind, Process, Result};
use serde::Deserialize;

use super::{BenchTool, Measured, Side, execute, installed_program};
use crate::go_test::packages;

pub static GO_BENCH: BenchTool = BenchTool {
    id: "tool.go-bench",
    langs: &["go"],
    program: "go",
    version: &["go", "version"],
    needs: &[FactKind::ProjectInfo],
    wants: &[],
    per_root: true,
    availability: installed_program,
    measure,
};

/// Each sample's run time: short, so many samples fit in a run.
const BENCH_TIME: &str = "100ms";

fn measure(side: &Side<'_>) -> Result<Measured> {
    let process = Process::new("go", side.root())
        .args([
            "test".to_owned(),
            "-run=^$".to_owned(),
            "-bench=.".to_owned(),
            "-benchmem".to_owned(),
            format!("-count={}", side.samples),
            format!("-benchtime={BENCH_TIME}"),
            "-json".to_owned(),
        ])
        .args(packages(side.packages));
    Ok(match execute(side, &process)? {
        Ok(captured) => {
            let mut benchmarks = parse(&captured.stdout);
            for benchmark in &mut benchmarks {
                place(side, benchmark);
            }
            Measured::of(benchmarks, &captured)
        }
        Err(detail) => Measured::failed(detail),
    })
}

/// One `go test -json` event.
#[derive(Deserialize)]
struct Event {
    #[serde(rename = "Action")]
    action: String,
    #[serde(rename = "Package", default)]
    package: String,
    #[serde(rename = "Output", default)]
    output: String,
}

/// Every benchmark the events on `stdout` report, in the order first reported: one per
/// benchmark and unit, each result line adding a sample. Only the first result line of a
/// benchmark carries its test name, and test2json may split a line Go printed in parts (the
/// name before the run, the numbers after), so results are read from each package's output
/// text joined.
pub fn parse(stdout: &str) -> Vec<Benchmark> {
    let mut texts: Vec<(String, String)> = Vec::new();
    for event in stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Event>(line).ok())
        .filter(|event| event.action == "output")
    {
        match texts
            .iter_mut()
            .find(|(package, _)| *package == event.package)
        {
            Some((_, text)) => text.push_str(&event.output),
            None => texts.push((event.package, event.output)),
        }
    }
    let mut benchmarks: Vec<Benchmark> = Vec::new();
    for (package, line) in texts
        .iter()
        .flat_map(|(package, text)| text.lines().map(move |line| (package, line)))
    {
        let Some((name, measures)) = result_line(line) else {
            continue;
        };
        for (value, unit) in measures {
            let name = format!("{package}.{name}");
            match benchmarks.iter_mut().find(|b| b.key() == (&name, unit)) {
                Some(benchmark) => benchmark.samples.push(value),
                None => benchmarks.push(Benchmark {
                    name,
                    unit: unit.to_owned(),
                    file: None,
                    line: None,
                    samples: vec![value],
                }),
            }
        }
    }
    benchmarks
}

/// A result line's benchmark, without the `-N` suffix Go adds for GOMAXPROCS, and each of its
/// costs with Cupel's unit.
fn result_line(output: &str) -> Option<(&str, Vec<(f64, &'static str)>)> {
    let mut words = output.split_whitespace();
    let name = words.next().filter(|name| name.starts_with("Benchmark"))?;
    words.next()?.parse::<u64>().ok()?;
    let rest: Vec<&str> = words.collect();
    let measures: Vec<(f64, &'static str)> = rest
        .chunks(2)
        .filter_map(|pair| {
            let unit = match *pair.get(1)? {
                "ns/op" => "ns",
                "B/op" => "bytes",
                "allocs/op" => "allocs",
                _ => return None,
            };
            Some((pair[0].parse().ok()?, unit))
        })
        .collect();
    let bare = match name.rsplit_once('-') {
        Some((bare, procs)) if procs.bytes().all(|byte| byte.is_ascii_digit()) => bare,
        _ => name,
    };
    (!measures.is_empty()).then_some((bare, measures))
}

/// Places `benchmark` at the `_test.go` function its name (after the package) names, when one
/// file under the root defines it.
fn place(side: &Side<'_>, benchmark: &mut Benchmark) {
    let function = benchmark
        .name
        .rsplit('.')
        .next()
        .and_then(|name| name.split('/').next())
        .unwrap_or_default();
    let prefix = format!("func {function}(");
    if let Some((file, line)) = side.define(&prefix, |file| file.ends_with("_test.go")) {
        benchmark.file = Some(file);
        benchmark.line = Some(line);
    }
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;

    #[test]
    fn every_result_line_is_a_sample_of_its_benchmark_in_each_unit() {
        let capture = Capture::of("go-bench", "green");
        let benchmarks = parse(&capture.file("stdout.jsonl"));
        let read: Vec<(&str, &str, &[f64])> = benchmarks
            .iter()
            .map(|b| (b.name.as_str(), b.unit.as_str(), b.samples.as_slice()))
            .collect();
        assert_eq!(
            read,
            [
                ("example.com/units.BenchmarkSum", "ns", &[37.21, 39.49][..]),
                ("example.com/units.BenchmarkSum", "bytes", &[0.0, 0.0][..]),
                ("example.com/units.BenchmarkSum", "allocs", &[0.0, 0.0][..]),
            ]
        );
    }

    #[test]
    fn a_result_line_split_across_events_is_joined_by_package() {
        let event = |package: &str, output: &str| {
            serde_json::json!({"Action": "output", "Package": package, "Output": output})
                .to_string()
        };
        let stdout = [
            event("a", "BenchmarkSum-8   \t"),
            event("b", "BenchmarkSum-8 \t 10\t 7.0 ns/op\n"),
            event("a", " 100\t 5.0 ns/op\n"),
        ]
        .join("\n");
        let read: Vec<(String, Vec<f64>)> = parse(&stdout)
            .into_iter()
            .map(|b| (b.name, b.samples))
            .collect();
        assert_eq!(
            read,
            [
                ("a.BenchmarkSum".to_owned(), vec![5.0]),
                ("b.BenchmarkSum".to_owned(), vec![7.0]),
            ]
        );
    }

    #[test]
    fn a_build_failure_reports_no_benchmark() {
        let capture = Capture::of("go-bench", "broken");
        assert!(parse(&capture.file("stdout.jsonl")).is_empty());
    }

    #[test]
    fn the_gomaxprocs_suffix_is_dropped_and_other_lines_are_not_results() {
        let line = |text| result_line(text).map(|(name, measures)| (name, measures.len()));
        assert_eq!(
            line("BenchmarkSum/size=10-8 \t 100\t 5.0 ns/op\n"),
            Some(("BenchmarkSum/size=10", 1))
        );
        assert_eq!(
            line("BenchmarkSum \t 100\t 5.0 ns/op\n"),
            Some(("BenchmarkSum", 1))
        );
        assert_eq!(line("BenchmarkSum\n"), None);
        assert_eq!(line("=== RUN   BenchmarkSum\n"), None);
        assert_eq!(line("BenchmarkSum-8 \t 100\t 9 MB/s\n"), None);
    }
}
