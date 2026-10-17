// ABOUTME: `tool.cargo-fuzz` (ADR-107): a Rust project's cargo-fuzz targets (the `[[bin]]`s of its `fuzz/` crate), each
// ABOUTME: built with `cargo +nightly fuzz build` into the cache and its libFuzzer binary run with the budget.
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use cupel_core::{
    Availability, Cancellation, Ctx, FuzzResults, FuzzStatus, Install, Location, Process, ToolId,
    ToolProbe, find_program,
};

use super::{Ended, Found, FuzzProducer, Fuzzer, Root, libfuzzer, located};
use crate::runner::probe;

const ID: &str = "tool.cargo-fuzz";

pub struct CargoFuzz;

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<FuzzResults, _>(FuzzProducer(CargoFuzz)),
});

impl Fuzzer for CargoFuzz {
    fn id(&self) -> &'static str {
        ID
    }

    fn lang(&self) -> &'static str {
        "rust"
    }

    fn seeded(&self) -> bool {
        true
    }

    /// Ready with cargo-fuzz on `PATH` and a nightly toolchain, which its sanitizer flags need.
    fn availability(&self, _: &Ctx) -> Availability {
        if find_program("cargo-fuzz").is_none() {
            return Availability::MissingTool {
                name: ToolId::from("cargo-fuzz"),
                install: Some(Install::Command(
                    "cargo install cargo-fuzz --locked".to_owned(),
                )),
            };
        }
        if host().is_none() {
            return Availability::MissingTool {
                name: ToolId::from("rustc +nightly"),
                install: Some(Install::Hint(
                    "cargo-fuzz builds with a nightly toolchain: `rustup toolchain install \
                     nightly`."
                        .to_owned(),
                )),
            };
        }
        Availability::Ready
    }

    fn tool(&self, ctx: &Ctx) -> Option<ToolProbe> {
        Some(probe(
            "cargo-fuzz",
            false,
            &["cargo-fuzz", "--version"],
            &ctx.root,
        ))
    }

    /// The `[[bin]]` targets of the root's `fuzz/Cargo.toml` when it is a cargo-fuzz crate;
    /// `None` for a root that is a cargo-fuzz crate itself.
    fn targets(&self, root: &Root<'_>) -> cupel_core::Result<Option<Vec<Found>>> {
        if is_fuzz_crate(&root.path.join("Cargo.toml")) {
            return Ok(None);
        }
        let manifest = root.path.join("fuzz").join("Cargo.toml");
        if !is_fuzz_crate(&manifest) {
            return Ok(Some(Vec::new()));
        }
        Ok(Some(bins(
            &fs::read_to_string(manifest).unwrap_or_default(),
        )))
    }

    fn fuzz(&self, root: &Root<'_>, target: &Found, area: &Path) -> cupel_core::Result<Ended> {
        let cancel = &root.ctx.cancel;
        let target_dir = root.cache.join("target");
        let built = Process::new("cargo", &root.path)
            .args(["+nightly", "fuzz", "build", target.name.as_str()])
            .env("CARGO_TARGET_DIR", &target_dir.to_string_lossy())
            .merged()
            .run(cancel)?;
        let binary = host().map(|host| target_dir.join(host).join("release").join(&target.name));
        let Some(binary) = binary.filter(|_| built.exit_code == Some(0)) else {
            return Ok(Ended {
                status: FuzzStatus::Failed,
                output: built.stdout,
                artifact: None,
                executions: None,
                coverage: None,
            });
        };
        let committed = root.path.join("fuzz").join("corpus").join(&target.name);
        libfuzzer::run(
            libfuzzer::Invocation {
                program: &binary.to_string_lossy(),
                dir: &root.path,
                before: Vec::new(),
                env: Vec::new(),
                separated: false,
            },
            area,
            Some(committed),
            &root.budget,
            cancel,
        )
    }

    fn failure(&self, root: &Root<'_>, output: &str) -> (String, Option<Location>) {
        libfuzzer::reported(panic(&root.ctx.ctx.root, output), output)
    }

    fn reproduce(&self, _: &Root<'_>, target: &Found, input: &str) -> String {
        format!("cargo +nightly fuzz run {} {input}", target.name)
    }
}

/// The nightly toolchain's host triple, which names the directory cargo-fuzz builds into;
/// `None` without a nightly toolchain.
fn host() -> Option<&'static str> {
    static HOST: OnceLock<Option<String>> = OnceLock::new();
    HOST.get_or_init(|| {
        let ran = Process::new("rustc", ".")
            .args(["+nightly", "-vV"])
            .run(&Cancellation::new())
            .ok()?;
        (ran.exit_code == Some(0))
            .then(|| {
                ran.stdout
                    .lines()
                    .find_map(|line| line.strip_prefix("host: "))
                    .map(str::to_owned)
            })
            .flatten()
    })
    .as_deref()
}

/// Whether the manifest at `path` is a cargo-fuzz crate's (`[package.metadata] cargo-fuzz = true`).
fn is_fuzz_crate(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|manifest| {
            manifest
                .get("package")?
                .get("metadata")?
                .get("cargo-fuzz")?
                .as_bool()
        })
        .unwrap_or(false)
}

/// The `[[bin]]` targets of a cargo-fuzz manifest, each at its source file (relative to the
/// project root).
fn bins(manifest: &str) -> Vec<Found> {
    let Ok(manifest) = manifest.parse::<toml::Table>() else {
        return Vec::new();
    };
    manifest
        .get("bin")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|bin| {
            let name = bin.get("name")?.as_str()?.to_owned();
            let file = bin
                .get("path")
                .and_then(toml::Value::as_str)
                .map(|path| format!("fuzz/{path}"));
            Some(Found {
                name,
                file,
                line: 1,
            })
        })
        .collect()
}

/// A Rust panic's message and place: `panicked at <file>:<line>:<col>:` and the line after it.
fn panic(repository: &Path, output: &str) -> Option<(String, Option<Location>)> {
    let mut lines = output.lines();
    let place = lines
        .by_ref()
        .find_map(|line| line.split_once("panicked at ").map(|(_, at)| at))?;
    let message = lines.next().unwrap_or_default().trim().to_owned();
    let mut parts = place.trim_end_matches(':').rsplitn(3, ':');
    let col = parts.next().and_then(|col| col.parse().ok());
    let line = parts.next().and_then(|line| line.parse().ok());
    let location = match (parts.next(), line) {
        (Some(file), Some(line)) => located(repository, file, line, col),
        _ => None,
    };
    Some((message, location))
}

#[cfg(test)]
mod tests {
    use cupel_fixtures::Capture;

    use super::*;

    #[test]
    fn a_panic_is_its_message_at_its_place_in_the_repository() {
        let capture = Capture::of("fuzz/cargo-fuzz", "crash");
        let repository = tempfile::tempdir().unwrap();
        fs::create_dir(repository.path().join("src")).unwrap();
        fs::write(repository.path().join("src/lib.rs"), "").unwrap();
        let output = capture.file("output.txt").replace(
            &capture.ran_in.to_string_lossy().into_owned(),
            &repository.path().to_string_lossy(),
        );
        let (message, location) = panic(repository.path(), &output).unwrap();
        assert_eq!(
            message,
            "index out of bounds: the len is 0 but the index is 0"
        );
        let location = location.unwrap();
        assert_eq!(
            (location.file.as_str(), location.line, location.col),
            ("src/lib.rs", 5, Some(5))
        );
    }

    #[test]
    fn a_panic_outside_the_repository_has_no_place_and_a_hang_has_no_panic() {
        let output =
            "thread '<unnamed>' (7) panicked at /rustc/abc/library/core/src/x.rs:9:1:\nboom\n";
        assert_eq!(
            panic(Path::new("/nowhere"), output),
            Some(("boom".to_owned(), None))
        );
        let hang = Capture::of("fuzz/cargo-fuzz", "timeout").file("output.txt");
        assert_eq!(panic(Path::new("/nowhere"), &hang), None);
    }

    #[test]
    fn the_targets_are_the_fuzz_crates_bins() {
        let manifest = r#"
[package]
name = "units-fuzz"

[package.metadata]
cargo-fuzz = true

[[bin]]
name = "first"
path = "fuzz_targets/first.rs"

[[bin]]
name = "spin"
path = "fuzz_targets/spin.rs"
"#;
        assert_eq!(
            bins(manifest),
            [
                Found {
                    name: "first".to_owned(),
                    file: Some("fuzz/fuzz_targets/first.rs".to_owned()),
                    line: 1,
                },
                Found {
                    name: "spin".to_owned(),
                    file: Some("fuzz/fuzz_targets/spin.rs".to_owned()),
                    line: 1,
                },
            ]
        );
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
        assert!(is_fuzz_crate(&dir.path().join("Cargo.toml")));
        fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname = \"units\"\n",
        )
        .unwrap();
        assert!(!is_fuzz_crate(&dir.path().join("Cargo.toml")));
    }
}
