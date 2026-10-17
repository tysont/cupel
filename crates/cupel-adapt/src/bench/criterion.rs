// ABOUTME: `tool.criterion` (ADR-106): each bench target of a Rust root's packages that use criterion, run with a short
// ABOUTME: measurement into a scratch CRITERION_HOME, whose `new/sample.json` gives each sample's time per iteration.
use std::fs;
use std::path::Path;

use cupel_core::{Benchmark, FactKind, Process, Result};
use serde::Deserialize;
use serde_json::Value;

use super::{BenchTool, Measured, Side, execute, installed_program, tail};

pub static CRITERION: BenchTool = BenchTool {
    id: "tool.criterion",
    langs: &["rust"],
    program: "cargo",
    version: &["cargo", "--version"],
    needs: &[FactKind::ProjectInfo],
    wants: &[],
    per_root: true,
    availability: installed_program,
    measure,
};

/// criterion's own minimum sample count.
const MINIMUM_SAMPLES: u32 = 10;

fn measure(side: &Side<'_>) -> Result<Measured> {
    let root = side.root();
    let metadata = Process::new("cargo", &root).args([
        "metadata",
        "--no-deps",
        "--format-version",
        "1",
        "--offline",
    ]);
    let targets = match execute(side, &metadata)? {
        Ok(captured) if captured.exit_code == Some(0) => bench_targets(&captured.stdout),
        Ok(captured) => return Ok(Measured::failed(tail(&captured))),
        Err(detail) => return Ok(Measured::failed(detail)),
    };
    let home = side.scratch.join("criterion");
    let mut failure = None;
    for target in targets {
        let bench = Process::new("cargo", &root)
            .args([
                "bench",
                "--bench",
                &target,
                "--",
                "--noplot",
                "--sample-size",
            ])
            .args([
                side.samples.max(MINIMUM_SAMPLES).to_string(),
                "--warm-up-time".to_owned(),
                "0.1".to_owned(),
                "--measurement-time".to_owned(),
                "0.5".to_owned(),
            ])
            .env("CRITERION_HOME", &home.to_string_lossy())
            .env("CARGO_TARGET_DIR", &side.build.to_string_lossy());
        match execute(side, &bench)? {
            Ok(captured) if captured.exit_code == Some(0) => {}
            Ok(captured) => failure = Some(format!("{target}: {}", tail(&captured))),
            Err(detail) => failure = Some(detail),
        }
    }
    Ok(Measured {
        benchmarks: read_home(&home),
        failure,
    })
}

/// The bench targets of the packages that depend on criterion, from `cargo metadata`.
fn bench_targets(metadata: &str) -> Vec<String> {
    let Ok(metadata) = serde_json::from_str::<Value>(metadata) else {
        return Vec::new();
    };
    let packages = metadata["packages"].as_array().into_iter().flatten();
    packages
        .filter(|package| {
            package["dependencies"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|dependency| dependency["name"] == "criterion")
        })
        .flat_map(|package| package["targets"].as_array().into_iter().flatten())
        .filter(|target| {
            target["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|k| k == "bench"))
        })
        .filter_map(|target| target["name"].as_str().map(str::to_owned))
        .collect()
}

/// `sample.json`: the iterations each sample ran and how long they took, in ns.
#[derive(Deserialize)]
struct Sample {
    iters: Vec<f64>,
    times: Vec<f64>,
}

/// `benchmark.json`: the benchmark's ID.
#[derive(Deserialize)]
struct Identity {
    full_id: String,
}

/// Every benchmark criterion recorded under `home`, by ID.
fn read_home(home: &Path) -> Vec<Benchmark> {
    let mut found = Vec::new();
    collect(home, &mut found);
    found.sort_by(|a: &Benchmark, b| a.name.cmp(&b.name));
    found
}

/// The benchmark of each `new/` directory under `dir`.
fn collect(dir: &Path, found: &mut Vec<Benchmark>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path.file_name().is_some_and(|name| name == "new") {
            found.extend(read(&path));
        } else {
            collect(&path, found);
        }
    }
}

/// The benchmark one `new/` directory holds: its ID and each sample's time per iteration.
fn read(new: &Path) -> Option<Benchmark> {
    let text = |name: &str| fs::read_to_string(new.join(name)).ok();
    let identity: Identity = serde_json::from_str(&text("benchmark.json")?).ok()?;
    let sample: Sample = serde_json::from_str(&text("sample.json")?).ok()?;
    Some(Benchmark {
        name: identity.full_id,
        unit: "ns".to_owned(),
        file: None,
        line: None,
        samples: sample
            .times
            .iter()
            .zip(&sample.iters)
            .filter(|(_, iters)| **iters > 0.0)
            .map(|(time, iters)| time / iters)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;

    #[test]
    fn a_new_directory_gives_each_sample_s_time_per_iteration() {
        let capture = Capture::of("criterion", "green");
        let dir = tempfile::tempdir().unwrap();
        let new = dir.path().join("sum").join("new");
        fs::create_dir_all(&new).unwrap();
        for name in ["sample.json", "benchmark.json"] {
            fs::write(new.join(name), capture.file(name)).unwrap();
        }
        let benchmarks = read_home(dir.path());
        assert_eq!(benchmarks.len(), 1);
        let benchmark = &benchmarks[0];
        assert_eq!(
            (benchmark.name.as_str(), benchmark.unit.as_str()),
            ("sum", "ns")
        );
        assert_eq!(benchmark.samples.len(), 10);
        assert!(
            (benchmark.samples[0] - 8_867_792.0 / 691_765.0).abs() < 1e-9,
            "{:?}",
            benchmark.samples
        );
    }

    #[test]
    fn only_bench_targets_of_packages_using_criterion_run() {
        let metadata = serde_json::json!({"packages": [
            {"dependencies": [{"name": "criterion"}], "targets": [
                {"name": "units", "kind": ["lib"]},
                {"name": "sum", "kind": ["bench"]},
            ]},
            {"dependencies": [], "targets": [{"name": "other", "kind": ["bench"]}]},
        ]});
        assert_eq!(bench_targets(&metadata.to_string()), ["sum"]);
        assert!(bench_targets("not json").is_empty());
    }
}
