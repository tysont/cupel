// ABOUTME: Where mutation slots live and how they are reclaimed (ADR-080 §8, ADR-141 §3): per repository, outside
// ABOUTME: it; removed when the repository is gone, idle, or beyond the total size; never while a production holds them.
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use cupel_mutate::slots::{Claim, Reclaim, claim_in, slot_dir, usage};

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

/// A rule that removes nothing by age or size.
const LENIENT: Reclaim = Reclaim {
    idle: Duration::MAX,
    total: u64::MAX,
};

/// A claim of `repo`'s slots under `root` by `rule`, its slots then holding `bytes`.
fn claimed(root: &Path, repo: &Path, rule: Reclaim, bytes: usize) -> Claim {
    let claim = claim_in(root, repo, rule).unwrap();
    let slot = claim.dir().join("python/0");
    fs::create_dir_all(&slot).unwrap();
    fs::write(slot.join("build.out"), vec![b'x'; bytes]).unwrap();
    claim
}

/// Makes the slots in `dir` look last used `ago`.
fn last_used(dir: &Path, ago: Duration) {
    fs::File::options()
        .write(true)
        .open(dir.join("repository"))
        .unwrap()
        .set_modified(SystemTime::now() - ago)
        .unwrap();
}

#[test]
fn a_slot_lives_outside_the_repository_under_its_language() {
    let repo = tempfile::tempdir().unwrap();
    let dir = slot_dir(repo.path(), "python", 2);
    assert!(!dir.starts_with(repo.path()));
    assert!(dir.ends_with("python/2"), "{}", dir.display());
    assert_eq!(dir, slot_dir(repo.path(), "python", 2));
    let elsewhere = tempfile::tempdir().unwrap();
    assert_ne!(
        dir.parent(),
        slot_dir(elsewhere.path(), "python", 2).parent()
    );
}

#[test]
fn claiming_slots_removes_those_of_repositories_that_are_gone() {
    let root = tempfile::tempdir().unwrap();
    let gone = tempfile::tempdir().unwrap();
    let gone_slots = claimed(root.path(), gone.path(), LENIENT, 1)
        .dir()
        .to_path_buf();
    drop(gone);
    let kept = tempfile::tempdir().unwrap();
    let kept_slots = claimed(root.path(), kept.path(), LENIENT, 1)
        .dir()
        .to_path_buf();
    drop(claim_in(root.path(), kept.path(), LENIENT).unwrap());
    assert_eq!((gone_slots.exists(), kept_slots.exists()), (false, true));
}

#[test]
fn slots_unused_for_longer_than_the_idle_time_are_removed() {
    let root = tempfile::tempdir().unwrap();
    let rule = Reclaim {
        idle: 7 * DAY,
        total: u64::MAX,
    };
    let (stale, recent, own) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let stale_slots = claimed(root.path(), stale.path(), rule, 1)
        .dir()
        .to_path_buf();
    let recent_slots = claimed(root.path(), recent.path(), rule, 1)
        .dir()
        .to_path_buf();
    last_used(&stale_slots, 8 * DAY);
    last_used(&recent_slots, DAY);
    drop(claim_in(root.path(), own.path(), rule).unwrap());
    assert_eq!((stale_slots.exists(), recent_slots.exists()), (false, true));
}

#[test]
fn beyond_the_total_size_the_least_recently_used_slots_of_other_repositories_go_first() {
    let root = tempfile::tempdir().unwrap();
    let rule = Reclaim {
        idle: Duration::MAX,
        total: 10_000,
    };
    let (older, newer, own) = (
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
        tempfile::tempdir().unwrap(),
    );
    let older_slots = claimed(root.path(), older.path(), LENIENT, 6_000)
        .dir()
        .to_path_buf();
    let newer_slots = claimed(root.path(), newer.path(), LENIENT, 3_000)
        .dir()
        .to_path_buf();
    last_used(&older_slots, 2 * DAY);
    last_used(&newer_slots, DAY);
    // Its own 2 000 bytes and the newer 3 000 fit in 10 000; the older 6 000 do not.
    drop(claimed(root.path(), own.path(), LENIENT, 2_000));
    let own_slots = claim_in(root.path(), own.path(), rule).unwrap();
    assert_eq!(
        (
            older_slots.exists(),
            newer_slots.exists(),
            own_slots.dir().exists()
        ),
        (false, true, true)
    );
}

#[test]
fn the_repository_being_graded_keeps_its_slots_whatever_their_size() {
    let root = tempfile::tempdir().unwrap();
    let rule = Reclaim {
        idle: Duration::ZERO,
        total: 1,
    };
    let own = tempfile::tempdir().unwrap();
    drop(claimed(root.path(), own.path(), LENIENT, 5_000));
    let claim = claim_in(root.path(), own.path(), rule).unwrap();
    assert!(claim.dir().join("python/0/build.out").exists());
}

#[test]
fn slots_a_running_production_holds_are_never_removed() {
    let root = tempfile::tempdir().unwrap();
    let rule = Reclaim {
        idle: Duration::ZERO,
        total: 1,
    };
    let running = tempfile::tempdir().unwrap();
    let held = claimed(root.path(), running.path(), LENIENT, 5_000);
    last_used(held.dir(), 30 * DAY);
    let own = tempfile::tempdir().unwrap();
    drop(claim_in(root.path(), own.path(), rule).unwrap());
    assert!(held.dir().join("python/0/build.out").exists());
    drop(held);
    drop(claim_in(root.path(), own.path(), rule).unwrap());
    assert!(
        !slot_dir_under(root.path(), &running),
        "once the production ends, the rule applies"
    );
}

/// Whether `root` still holds a slot directory naming `repo`.
fn slot_dir_under(root: &Path, repo: &tempfile::TempDir) -> bool {
    let named = repo.path().canonicalize().unwrap();
    fs::read_dir(root).unwrap().flatten().any(|entry| {
        fs::read_to_string(entry.path().join("repository")).ok()
            == Some(named.display().to_string())
    })
}

#[test]
fn a_claim_records_the_bytes_its_slots_hold_when_it_ends() {
    let root = tempfile::tempdir().unwrap();
    let repo = tempfile::tempdir().unwrap();
    let claim = claimed(root.path(), repo.path(), LENIENT, 4_096);
    let dir = claim.dir().to_path_buf();
    drop(claim);
    let recorded: u64 = fs::read_to_string(dir.join("size"))
        .unwrap()
        .parse()
        .unwrap();
    // The slot's file and the small records beside it.
    assert!((4_096..4_096 + 1_024).contains(&recorded), "{recorded}");
}

#[test]
fn usage_lists_each_repositorys_slots_with_their_size_most_recently_used_first() {
    let root = tempfile::tempdir().unwrap();
    let (gone, kept) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let gone_path = gone.path().canonicalize().unwrap();
    let gone_slots = claimed(root.path(), gone.path(), LENIENT, 100)
        .dir()
        .to_path_buf();
    drop(claimed(root.path(), kept.path(), LENIENT, 300));
    last_used(&gone_slots, DAY);
    drop(gone);
    let found: Vec<(Option<std::path::PathBuf>, bool)> = usage(root.path())
        .into_iter()
        .map(|slots| (slots.repository, slots.exists))
        .collect();
    assert_eq!(
        found,
        [
            (Some(kept.path().canonicalize().unwrap()), true),
            (Some(gone_path), false)
        ]
    );
    assert!(usage(root.path())[0].bytes >= 300);
    assert!(usage(&root.path().join("none")).is_empty());
}

/// Slots made by a production that takes no lock (an earlier Cupel, perhaps running now) count
/// toward the total but are never removed for it; the gone and idle rules still apply.
#[test]
fn slots_with_no_lock_file_are_removed_only_when_gone_or_idle() {
    let root = tempfile::tempdir().unwrap();
    let rule = Reclaim {
        idle: 7 * DAY,
        total: 1,
    };
    let (unlocked, own) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let slots = claimed(root.path(), unlocked.path(), LENIENT, 5_000)
        .dir()
        .to_path_buf();
    fs::remove_file(slots.join("lock")).unwrap();
    drop(claim_in(root.path(), own.path(), rule).unwrap());
    assert!(
        slots.join("python/0/build.out").exists(),
        "kept over the size"
    );
    assert!(!slots.join("lock").exists(), "and left as it was");
    last_used(&slots, 8 * DAY);
    drop(claim_in(root.path(), own.path(), rule).unwrap());
    assert!(!slots.exists(), "removed when idle");
}
