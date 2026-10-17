// ABOUTME: Tests for the `BenchSamples` payload (ADR-106): its shape as JSON with the head side's benchmarks and the
// ABOUTME: range's base side measured by the same tool, and how a benchmark is known on both sides.
use cupel_core::{BenchBase, BenchSamples, Benchmark, ProducerId, RootRun, RunStatus};
use serde_json::json;

fn benchmark(samples: Vec<f64>) -> Benchmark {
    Benchmark {
        name: "example.com/units.BenchmarkSum".to_owned(),
        unit: "allocs".to_owned(),
        file: Some("sum_test.go".to_owned()),
        line: Some(7),
        samples,
    }
}

#[test]
fn bench_samples_serializes_every_field() {
    let samples = BenchSamples {
        tool: ProducerId::from("tool.go-bench"),
        samples: 2,
        runs: vec![RootRun {
            dir: ".".to_owned(),
            status: RunStatus::Ran,
            detail: None,
        }],
        benchmarks: vec![benchmark(vec![3.0, 3.0])],
        base: Some(BenchBase {
            commit: "abc123".to_owned(),
            runs: vec![RootRun {
                dir: ".".to_owned(),
                status: RunStatus::Failed,
                detail: Some("build failed".to_owned()),
            }],
            benchmarks: vec![benchmark(vec![1.0, 1.5])],
        }),
    };
    let value = serde_json::to_value(&samples).unwrap();
    let measured = |samples: [f64; 2]| {
        json!({
            "name": "example.com/units.BenchmarkSum", "unit": "allocs",
            "file": "sum_test.go", "line": 7, "samples": samples,
        })
    };
    assert_eq!(
        value,
        json!({
            "tool": "tool.go-bench",
            "samples": 2,
            "runs": [{"dir": ".", "status": "ran", "detail": null}],
            "benchmarks": [measured([3.0, 3.0])],
            "base": {
                "commit": "abc123",
                "runs": [{"dir": ".", "status": "failed", "detail": "build failed"}],
                "benchmarks": [measured([1.0, 1.5])],
            },
        })
    );
    assert_eq!(
        serde_json::from_value::<BenchSamples>(value).unwrap(),
        samples
    );
}

#[test]
fn a_benchmark_is_known_by_its_name_and_unit() {
    let time = Benchmark {
        unit: "ns".to_owned(),
        ..benchmark(vec![1.0])
    };
    assert_eq!(
        benchmark(vec![1.0]).key(),
        ("example.com/units.BenchmarkSum", "allocs")
    );
    assert_ne!(time.key(), benchmark(vec![2.0]).key());
}
