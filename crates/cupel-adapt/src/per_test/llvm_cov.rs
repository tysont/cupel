// ABOUTME: The Rust `PerTestMap` producer (ADR-077, ADR-083): one `cargo llvm-cov nextest` run whose target runner gives
// ABOUTME: each test process its own profile directory, then each test's profiles merged and exported to LCOV alone.
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use cupel_core::{CoveredLines, Error, LangId, Langs, PerTestMap, Process, Result, TestCoverage};

use super::cache::own_file;
use super::{Measured, PerTest, PerTestSpec, Runs, Site, covered, executed_lines, failed};
use crate::cargo_nextest::filterset;
use crate::coverage::{parse_lcov, placed};

const ID: &str = "tool.llvm-cov-per-test";

static LLVM_COV: PerTestSpec = PerTestSpec {
    id: ID,
    languages: || Langs::Only(vec![LangId::from("rust")]),
    runner: "tool.cargo-nextest",
    programs: &["cargo-llvm-cov", "cargo-nextest"],
    version: &["cargo", "llvm-cov", "--version"],
    runs: Runs::Suite,
    outputs: &["target"],
    whole: &[
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        "rust-toolchain.toml",
        "rust-toolchain",
    ],
    sources: &["rs"],
    owns: |_: &Path, test, file| own_file(test, file),
    measure,
};

inventory::submit!(crate::ToolProducer {
    id: ID,
    register: |registry| registry.register::<PerTestMap, _>(PerTest(&LLVM_COV)),
});

/// The default nextest profile, whatever profile a nextest run around this one handed its tests
/// (`NEXTEST_PROFILE`), as when Cupel's own suite runs under Cupel's nextest runner.
const ENV: &[(&str, &str)] = &[
    ("CARGO_TERM_COLOR", "never"),
    ("NEXTEST_PROFILE", "default"),
];

/// Files no report needs: dependencies and the standard library.
const IGNORED: &str = r"/\.cargo/registry/|/\.cargo/git/|/\.rustup/|^/rustc/";

fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(|word| (*word).to_owned()).collect()
}

/// One instrumented run of every test but `keep`, each test process writing its profiles into
/// a directory of its own through the target runner, then each test's coverage exported alone.
/// `--no-report` keeps the instrumented build between runs, so a refresh builds incrementally.
fn measure(site: &Site<'_>, keep: &BTreeSet<String>) -> Result<Measured> {
    let llvm = Llvm::find(site)?;
    let profiles = site.report("profiles");
    let _ = std::fs::remove_dir_all(&profiles);
    site.ctx.writer.create_dir(&profiles)?;
    let script = site.report("runner.sh");
    site.ctx
        .writer
        .write(&script, runner_script(&llvm.runner, &profiles).as_bytes())?;
    executable(&script)?;
    let mut args = words(&[
        "llvm-cov",
        "nextest",
        "--no-report",
        "--workspace",
        "--no-fail-fast",
        "--no-tests=pass",
    ]);
    if !keep.is_empty() {
        let kept: Vec<String> = keep.iter().cloned().collect();
        args.extend(["-E".to_owned(), format!("not ({})", filterset(&kept))]);
    }
    let script = script.to_string_lossy().into_owned();
    let mut env = ENV.to_vec();
    env.push((&llvm.runner, &script));
    // A failing test still measures what it ran; a run that started no test process failed,
    // unless it had no test to run.
    let captured = site.run("cargo", &args, &env)?;
    let processes = processes(&profiles);
    if processes.is_empty() && captured.exit_code != Some(0) {
        return Err(failed("cargo", &captured));
    }
    let tests = export(site, &llvm, &processes)?;
    let _ = std::fs::remove_dir_all(&profiles);
    Ok(Measured {
        tests,
        listed: None,
    })
}

/// The target runner nextest runs every test binary through: a test process gets a profile
/// directory named by its process ID, which its own children inherit, and records which test
/// it is. The runner variable is unset so that cargo runs the tests start do not use it.
fn runner_script(variable: &str, profiles: &Path) -> String {
    let dir = profiles.display().to_string().replace('\'', r"'\''");
    format!(
        "#!/bin/sh\n\
         # Written by Cupel (ADR-083): one profile directory per nextest test process.\n\
         unset {variable}\n\
         if [ \"$NEXTEST_TEST_PHASE\" = run ]; then\n\
         \x20 dir='{dir}'/$$\n\
         \x20 mkdir -p \"$dir\"\n\
         \x20 printf '%s\\n%s\\n%s\\n' \"$NEXTEST_BINARY_ID\" \"$NEXTEST_TEST_NAME\" \"$1\" > \"$dir/test\"\n\
         \x20 LLVM_PROFILE_FILE=\"$dir/%p-%m.profraw\"\n\
         \x20 export LLVM_PROFILE_FILE\n\
         fi\n\
         exec \"$@\"\n"
    )
}

#[cfg(unix)]
fn executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).map_err(|error| {
        Error::Process {
            program: "sh".to_owned(),
            message: format!("cannot make {} executable: {error}", path.display()),
        }
    })
}

#[cfg(not(unix))]
fn executable(_: &Path) -> Result<()> {
    Err(Error::Process {
        program: "sh".to_owned(),
        message: "per-test coverage for Rust needs a Unix shell".to_owned(),
    })
}

/// The LLVM tools of the root's toolchain and the target runner variable of its host.
struct Llvm {
    profdata: PathBuf,
    cov: PathBuf,
    /// `CARGO_TARGET_<HOST>_RUNNER`.
    runner: String,
}

impl Llvm {
    /// The tools as `cargo llvm-cov` finds them: `LLVM_PROFDATA` and `LLVM_COV`, else the
    /// `llvm-tools` component of the toolchain the root uses.
    fn find(site: &Site<'_>) -> Result<Self> {
        let version = site.succeed("rustc", &words(&["-vV"]), &[])?.stdout;
        let host = version
            .lines()
            .find_map(|line| line.strip_prefix("host: "))
            .unwrap_or_default()
            .trim()
            .to_owned();
        let sysroot = site.succeed("rustc", &words(&["--print", "sysroot"]), &[])?;
        let bin = Path::new(sysroot.stdout.trim())
            .join("lib/rustlib")
            .join(&host)
            .join("bin");
        let tool = |variable: &str, name: &str| {
            std::env::var_os(variable).map_or_else(|| bin.join(name), PathBuf::from)
        };
        Ok(Self {
            profdata: tool("LLVM_PROFDATA", "llvm-profdata"),
            cov: tool("LLVM_COV", "llvm-cov"),
            runner: format!(
                "CARGO_TARGET_{}_RUNNER",
                host.to_uppercase().replace(['-', '.'], "_")
            ),
        })
    }
}

/// One test process's profiles: its test, the binary it ran, and the profile files it and its
/// children wrote.
struct TestProcess {
    id: String,
    binary: PathBuf,
    profiles: Vec<PathBuf>,
}

/// Every test process that wrote its directory under `profiles`.
fn processes(profiles: &Path) -> Vec<TestProcess> {
    let mut found = Vec::new();
    for dir in crate::fresh::entries(profiles) {
        let Ok(test) = std::fs::read_to_string(dir.join("test")) else {
            continue;
        };
        let mut lines = test.lines();
        let (Some(binary_id), Some(name), Some(binary)) =
            (lines.next(), lines.next(), lines.next())
        else {
            continue;
        };
        let profiles: Vec<PathBuf> = crate::fresh::entries(&dir)
            .into_iter()
            .filter(|path| path.extension().is_some_and(|ext| ext == "profraw"))
            .collect();
        found.push(TestProcess {
            id: format!("{binary_id} {name}"),
            binary: PathBuf::from(binary),
            profiles,
        });
    }
    found
}

/// A test and lines it covered.
type Covered = (String, Vec<CoveredLines>);

/// Each process's coverage, exported on as many threads as there are cores; a test run more
/// than once (a retry) covers what any of its runs covered.
fn export(site: &Site<'_>, llvm: &Llvm, processes: &[TestProcess]) -> Result<Vec<TestCoverage>> {
    let next = AtomicUsize::new(0);
    let found: Mutex<Vec<Result<Covered>>> = Mutex::new(Vec::new());
    let workers = std::thread::available_parallelism().map_or(1, usize::from);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                while let Some(process) = processes.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let covered = covered_by(site, llvm, process);
                    let mut found = found.lock().unwrap_or_else(PoisonError::into_inner);
                    found.push(covered.map(|lines| (process.id.clone(), lines)));
                }
            });
        }
    });
    let mut by_test: BTreeMap<String, Vec<CoveredLines>> = BTreeMap::new();
    for result in found.into_inner().unwrap_or_else(PoisonError::into_inner) {
        let (id, lines) = result?;
        by_test.entry(id).or_default().extend(lines);
    }
    Ok(by_test
        .into_iter()
        .map(|(id, lines)| TestCoverage::new(&id, None, lines))
        .collect())
}

/// The lines `process` executed: its profiles merged, then exported for its binary (and, when
/// it started children, every executable the build made, which they may have run).
fn covered_by(site: &Site<'_>, llvm: &Llvm, process: &TestProcess) -> Result<Vec<CoveredLines>> {
    let Some(dir) = process.profiles.first().and_then(|path| path.parent()) else {
        return Ok(Vec::new());
    };
    let merged = dir.join("merged.profdata");
    let mut merge = vec![
        "merge".to_owned(),
        "-sparse".to_owned(),
        "-o".to_owned(),
        merged.to_string_lossy().into_owned(),
    ];
    merge.extend(
        process
            .profiles
            .iter()
            .map(|path| path.to_string_lossy().into_owned()),
    );
    let program = llvm.profdata.to_string_lossy();
    let captured = run(site, &program, &merge)?;
    if captured.exit_code != Some(0) {
        return Err(failed(&program, &captured));
    }
    let mut export = vec![
        "export".to_owned(),
        "-format=lcov".to_owned(),
        "-skip-functions".to_owned(),
        format!("-instr-profile={}", merged.display()),
        format!("-ignore-filename-regex={IGNORED}"),
        process.binary.to_string_lossy().into_owned(),
    ];
    if process.profiles.len() > 1 {
        for object in executables(&process.binary) {
            export.extend(["-object".to_owned(), object.to_string_lossy().into_owned()]);
        }
    }
    let program = llvm.cov.to_string_lossy();
    let captured = run(site, &program, &export)?;
    if captured.exit_code != Some(0) {
        return Err(failed(&program, &captured));
    }
    let lines = parse_lcov(&captured.stdout)
        .ok()
        .map(|map| executed_lines(&placed(map, &[&site.path], ".")))
        .unwrap_or_default();
    Ok(covered(lines, |file| Some(file.to_owned())))
}

fn run(site: &Site<'_>, program: &str, args: &[String]) -> Result<cupel_core::Captured> {
    Process::new(program, &site.path)
        .args(args.iter().cloned())
        .run(&site.ctx.cancel)
}

/// The programs the build of `binary` (`<profile>/deps/<test>`) made in `<profile>/`: its
/// executable files with no extension, not hidden (cargo's `.cargo-lock`).
fn executables(binary: &Path) -> Vec<PathBuf> {
    let Some(profile) = binary.parent().and_then(Path::parent) else {
        return Vec::new();
    };
    crate::fresh::entries(profile)
        .into_iter()
        .filter(|path| {
            let hidden = path
                .file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with('.'));
            !hidden && path.extension().is_none() && is_executable(path)
        })
        .collect()
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::{executables, runner_script};

    #[cfg(unix)]
    #[test]
    fn the_executables_beside_a_test_binary_are_the_programs_the_build_made() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let profile = dir.path().join("debug");
        std::fs::create_dir_all(profile.join("deps")).unwrap();
        for (name, mode) in [
            ("cupel", 0o755),
            ("cupel.d", 0o644),
            (".cargo-lock", 0o644),
            ("libcupel.rlib", 0o644),
            ("deps/tests-0123", 0o755),
        ] {
            let path = profile.join(name);
            std::fs::write(&path, "").unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        std::fs::create_dir_all(profile.join("build")).unwrap();
        assert_eq!(
            executables(&profile.join("deps/tests-0123")),
            [profile.join("cupel")]
        );
    }

    #[test]
    fn the_runner_gives_each_test_process_its_own_profile_directory() {
        let script = runner_script(
            "CARGO_TARGET_X_RUNNER",
            std::path::Path::new("/a/it's/profiles"),
        );
        assert!(script.starts_with("#!/bin/sh\n"), "{script}");
        for line in [
            "unset CARGO_TARGET_X_RUNNER\n",
            "  dir='/a/it'\\''s/profiles'/$$\n",
            "  LLVM_PROFILE_FILE=\"$dir/%p-%m.profraw\"\n",
            "exec \"$@\"\n",
        ] {
            assert!(script.contains(line), "{line:?} in {script}");
        }
    }
}
