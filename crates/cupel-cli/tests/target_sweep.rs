// ABOUTME: Tests for scripts/target-sweep.sh, which `make check` runs first: when the checkout's builds (target/ and its
// ABOUTME: mutation slots') are over the cap it runs cargo-sweep under the heavy lock to bring them under, and otherwise does nothing.
#![cfg(unix)]
mod support;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use support::{assert_executable, assert_shellcheck_clean, recipe, workspace};
use tempfile::TempDir;

fn script() -> PathBuf {
    workspace().join("scripts/target-sweep.sh")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A git repository (heavy.sh runs only in one) whose `target/` holds `kib` KiB, and a directory
/// for programs on PATH.
fn checkout(kib: usize) -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(dir.path())
        .status()
        .unwrap();
    assert!(status.success());
    fs::create_dir_all(dir.path().join("target/debug/deps")).unwrap();
    fs::write(
        dir.path().join("target/debug/deps/libbig.rlib"),
        vec![0u8; kib * 1024],
    )
    .unwrap();
    let bin = dir.path().join("bin");
    fs::create_dir(&bin).unwrap();
    (dir, bin)
}

/// A `cargo-sweep` on PATH that records its arguments and the heavy lock it ran under in
/// `swept` in the checkout.
fn fake_sweep(bin: &Path) {
    let program = bin.join("cargo-sweep");
    fs::write(
        &program,
        "#!/bin/sh\necho \"$* held=${CUPEL_HEAVY_HELD:-}\" >> swept\n",
    )
    .unwrap();
    fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
}

/// `scripts/target-sweep.sh CAP` in `dir`, with only `bin` and the system's directories on PATH,
/// the heavy lock in `dir`, and no free-space floor.
fn sweep(dir: &Path, bin: &Path, cap: &str) -> Output {
    Command::new("sh")
        .arg(script())
        .arg(cap)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .env("CUPEL_HEAVY_LOCK", dir.join("heavy.lock"))
        .env("CUPEL_HEAVY_PRIORITY", "default")
        .env("CUPEL_HEAVY_MIN_FREE_GB", "0")
        .env_remove("CUPEL_HEAVY_HELD")
        .current_dir(dir)
        .output()
        .unwrap()
}

/// `make check` runs the script directly, so it is executable; and it passes `shellcheck`.
#[test]
fn the_script_is_executable_and_passes_shellcheck() {
    assert_executable("scripts/target-sweep.sh");
    assert_shellcheck_clean("scripts/target-sweep.sh");
}

/// A build directory under the cap is left alone: cargo-sweep does not run and nothing is printed.
#[test]
fn a_build_directory_under_the_cap_is_left_alone() {
    let (dir, bin) = checkout(512);
    fake_sweep(&bin);
    let output = sweep(dir.path(), &bin, "1MB");
    assert_eq!(
        (
            output.status.code(),
            text(&output.stdout),
            text(&output.stderr)
        ),
        (Some(0), String::new(), String::new())
    );
    assert!(!dir.path().join("swept").exists());
}

/// A build directory over the cap is swept to the cap, under the heavy lock, so no dogfood of the
/// checkout is reading it meanwhile.
#[test]
fn a_build_directory_over_the_cap_is_swept_to_it_under_the_heavy_lock() {
    let (dir, bin) = checkout(2048);
    fake_sweep(&bin);
    let output = sweep(dir.path(), &bin, "1MB");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    assert_eq!(
        fs::read_to_string(dir.path().join("swept")).unwrap(),
        format!(
            "sweep --maxsize 1MB . held={}\n",
            dir.path().join("heavy.lock").display()
        )
    );
    assert!(
        text(&output.stderr)
            .starts_with("target-sweep.sh: the builds hold 2 MB, over the 1MB cap;"),
        "{}",
        text(&output.stderr)
    );
}

/// The mutation slots' builds, in `.cupel/work/slots/<lang>/<n>/target` (ADR-159), count toward the cap with
/// `target/`; over it, each build is swept to its share of the cap, by what it holds.
#[test]
fn the_mutation_slots_builds_count_toward_the_cap_and_each_is_swept_to_its_share() {
    let (dir, bin) = checkout(2048);
    let slot = dir.path().join(".cupel/work/slots/rust/0");
    fs::create_dir_all(slot.join("target/debug")).unwrap();
    fs::write(slot.join("target/debug/cupel"), vec![0u8; 6 * 1024 * 1024]).unwrap();
    fake_sweep(&bin);
    let output = sweep(dir.path(), &bin, "4MB");
    assert_eq!(output.status.code(), Some(0), "{}", text(&output.stderr));
    let held = dir.path().join("heavy.lock").display().to_string();
    assert_eq!(
        fs::read_to_string(dir.path().join("swept")).unwrap(),
        format!(
            "sweep --maxsize 1MB . held={held}\nsweep --maxsize 3MB .cupel/work/slots/rust/0 held={held}\n"
        )
    );
}

/// A checkout that has not built yet has nothing to sweep.
#[test]
fn a_checkout_without_a_build_directory_is_left_alone() {
    let (dir, bin) = checkout(0);
    fs::remove_dir_all(dir.path().join("target")).unwrap();
    let output = sweep(dir.path(), &bin, "1MB");
    assert_eq!(
        (output.status.code(), text(&output.stderr)),
        (Some(0), String::new())
    );
}

/// Over the cap without cargo-sweep, the check fails and says how to install it.
#[test]
fn over_the_cap_without_cargo_sweep_fails_naming_dev_setup() {
    let (dir, bin) = checkout(2048);
    let output = sweep(dir.path(), &bin, "1MB");
    assert_eq!(output.status.code(), Some(1));
    assert!(
        text(&output.stderr).contains("cargo-sweep is not on PATH; run `make dev-setup`"),
        "{}",
        text(&output.stderr)
    );
}

/// A cap must be a whole number of MB or GB, as cargo-sweep reads it.
#[test]
fn a_cap_without_a_unit_is_refused() {
    let (dir, bin) = checkout(0);
    let output = sweep(dir.path(), &bin, "40");
    assert_eq!(output.status.code(), Some(2));
    assert!(text(&output.stderr).contains("usage: scripts/target-sweep.sh CAP"));
}

/// `make check` sweeps first, before anything builds, at `TARGET_CAP`.
#[test]
fn make_check_sweeps_the_build_directory_first() {
    let check = recipe("check");
    assert_eq!(
        check.first().map(String::as_str),
        Some("@scripts/target-sweep.sh $(TARGET_CAP)"),
        "{check:#?}"
    );
}
