// ABOUTME: Worker overlays (ADR-080 §8): a slot mirrors the tree, links ignored directories, writes mutants as private
// ABOUTME: copies that never reach the original, restores them newer than the mutant, follows the tree's changes, and hashes it.
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use cupel_mutate::overlay::Slot;

/// A repository with a tracked source, an ignored build directory, git and Cupel directories.
fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for (path, text) in [
        (".gitignore", "target/\n"),
        ("src/calc.py", "def add(a, b):\n    return a + b\n"),
        ("src/other.py", "X = 1\n"),
        ("target/build.out", "built\n"),
        (".git/HEAD", "ref: refs/heads/main\n"),
        (".cupel/cupel.db", "store\n"),
        ("fixtures/.cupel/cache/report", "committed\n"),
    ] {
        fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
        fs::write(root.join(path), text).unwrap();
    }
    dir
}

/// A repository and a slot synced with it, keeping `outputs` its own.
struct Synced {
    repo: tempfile::TempDir,
    slot: Slot,
    _work: tempfile::TempDir,
}

fn synced(outputs: &[&str]) -> Synced {
    let repo = repository();
    let work = tempfile::tempdir().unwrap();
    let slot = Slot::at(work.path().join("0"));
    slot.sync(repo.path(), outputs).unwrap();
    Synced {
        repo,
        slot,
        _work: work,
    }
}

fn read(root: &Path, path: &str) -> String {
    fs::read_to_string(root.join(path)).unwrap()
}

fn modified(path: &Path) -> SystemTime {
    fs::metadata(path).unwrap().modified().unwrap()
}

#[test]
fn a_slot_mirrors_the_tree_and_links_what_the_tree_ignores() {
    let Synced { repo, slot, _work } = synced(&[]);
    let root = slot.root();
    assert_eq!(read(root, "src/calc.py"), read(repo.path(), "src/calc.py"));
    assert_eq!(read(root, ".gitignore"), "target/\n");
    let target = fs::symlink_metadata(root.join("target")).unwrap();
    assert!(target.file_type().is_symlink());
    assert_eq!(read(root, "target/build.out"), "built\n");
    assert!(!root.join(".git").exists());
    assert!(!root.join(".cupel").exists());
    // Only the repository's own `.cupel` is left out; one inside it is the tree's.
    assert_eq!(read(root, "fixtures/.cupel/cache/report"), "committed\n");
}

/// A slot lives in the tree's own `.cupel/work/slots/` (ADR-159); the tree's `.cupel/` is the slot's own, so a sync
/// neither walks into the slot nor links the tree's `.cupel/` into it.
#[test]
fn a_slot_inside_the_trees_cupel_directory_mirrors_the_tree_and_not_itself() {
    let repo = repository();
    let slot = Slot::at(repo.path().join(".cupel/work/slots/python/0"));
    slot.sync(repo.path(), &[]).unwrap();
    slot.sync(repo.path(), &[]).unwrap();
    assert_eq!(
        read(slot.root(), "src/calc.py"),
        "def add(a, b):\n    return a + b\n"
    );
    assert!(!slot.root().join(".cupel").exists());
}

#[test]
fn a_mutant_is_a_private_copy_and_the_original_never_changes() {
    let Synced { repo, slot, _work } = synced(&[]);
    let original = read(repo.path(), "src/calc.py");
    slot.write("src/calc.py", b"def add(a, b):\n    return a - b\n")
        .unwrap();
    assert_eq!(read(repo.path(), "src/calc.py"), original);
    assert_eq!(
        read(slot.root(), "src/calc.py"),
        "def add(a, b):\n    return a - b\n"
    );
}

#[test]
fn a_restored_file_is_newer_than_the_mutant_it_replaces() {
    let Synced { repo, slot, _work } = synced(&[]);
    let path = slot.root().join("src/calc.py");
    slot.write("src/calc.py", b"mutant\n").unwrap();
    let mutated = modified(&path);
    std::thread::sleep(Duration::from_millis(20));
    let original = fs::read(repo.path().join("src/calc.py")).unwrap();
    slot.write("src/calc.py", &original).unwrap();
    assert!(modified(&path) > mutated);
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn a_second_sync_follows_the_trees_changes_and_keeps_restored_files() {
    let Synced { repo, slot, _work } = synced(&[]);
    let original = fs::read(repo.path().join("src/calc.py")).unwrap();
    slot.write("src/calc.py", b"mutant\n").unwrap();
    slot.write("src/calc.py", &original).unwrap();
    let restored = modified(&slot.root().join("src/calc.py"));
    fs::remove_file(repo.path().join("src/other.py")).unwrap();
    fs::write(repo.path().join("src/new.py"), "Y = 2\n").unwrap();
    slot.sync(repo.path(), &[]).unwrap();
    let root = slot.root();
    assert!(!root.join("src/other.py").exists());
    assert_eq!(read(root, "src/new.py"), "Y = 2\n");
    // Same bytes as the tree: kept, with the time it was restored, so a build that saw the
    // mutant rebuilds it.
    assert_eq!(modified(&root.join("src/calc.py")), restored);
}

/// A slot file left holding other bytes (a mutant of a stopped run) is replaced by a copy dated
/// after it, never a link carrying the tree's older time, so a build that saw the mutant
/// rebuilds (ADR-141 §3).
#[test]
fn a_sync_replaces_a_file_left_mutated_with_a_copy_dated_now() {
    let Synced { repo, slot, _work } = synced(&[]);
    let tree = repo.path().join("src/calc.py");
    let old = SystemTime::now() - Duration::from_secs(3600);
    fs::File::options()
        .write(true)
        .open(&tree)
        .unwrap()
        .set_modified(old)
        .unwrap();
    slot.write("src/calc.py", b"mutant\n").unwrap();
    let mutated = modified(&slot.root().join("src/calc.py"));
    slot.sync(repo.path(), &[]).unwrap();
    let path = slot.root().join("src/calc.py");
    assert_eq!(fs::read(&path).unwrap(), fs::read(&tree).unwrap());
    assert!(modified(&path) >= mutated, "dated now, not {old:?}");
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let (ours, theirs) = (fs::metadata(&path).unwrap(), fs::metadata(&tree).unwrap());
        assert_ne!(
            ours.ino(),
            theirs.ino(),
            "a copy, not a link to the tree's file"
        );
    }
}

#[test]
fn a_runners_build_outputs_are_the_slots_own_and_kept_between_syncs() {
    let Synced { repo, slot, _work } = synced(&["target"]);
    assert!(!slot.root().join("target").exists());
    fs::create_dir_all(slot.root().join("target/debug")).unwrap();
    fs::write(slot.root().join("target/debug/own"), "slot build\n").unwrap();
    slot.sync(repo.path(), &["target"]).unwrap();
    assert_eq!(read(slot.root(), "target/debug/own"), "slot build\n");
    assert_eq!(read(repo.path(), "target/build.out"), "built\n");
}

#[cfg(unix)]
#[test]
fn a_symlink_in_the_tree_is_the_same_symlink_in_the_slot() {
    let repo = repository();
    std::os::unix::fs::symlink("src", repo.path().join("alias")).unwrap();
    let work = tempfile::tempdir().unwrap();
    let slot = Slot::at(work.path().join("0"));
    slot.sync(repo.path(), &[]).unwrap();
    assert_eq!(
        (
            fs::read_link(slot.root().join("alias")).unwrap(),
            read(slot.root(), "alias/other.py")
        ),
        (Path::new("src").to_path_buf(), "X = 1\n".to_owned())
    );
}

/// The content hash a production's baseline is stored by (ADR-185) covers the mirrored files under its directories:
/// a change to one of them changes it; an ignored file, Cupel's own, or a file outside the directories does not.
#[test]
fn the_content_hash_follows_the_mirrored_files_under_its_directories_only() {
    let repo = repository();
    let root = repo.path();
    let hash = || cupel_mutate::overlay::content_hash(root, &["src"], &[]).unwrap();
    let before = hash();
    fs::write(root.join("target/build.out"), "rebuilt\n").unwrap();
    fs::write(root.join(".cupel/cupel.db"), "another store\n").unwrap();
    fs::write(root.join("fixtures/.cupel/cache/report"), "outside src\n").unwrap();
    assert_eq!(hash(), before);
    fs::write(root.join("src/other.py"), "X = 2\n").unwrap();
    assert_ne!(hash(), before);
    let whole = cupel_mutate::overlay::content_hash(root, &["."], &[]).unwrap();
    fs::write(root.join("fixtures/.cupel/cache/report"), "changed again\n").unwrap();
    assert_ne!(
        cupel_mutate::overlay::content_hash(root, &["."], &[]).unwrap(),
        whole
    );
}
