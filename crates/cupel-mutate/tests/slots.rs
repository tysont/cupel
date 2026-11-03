// ABOUTME: Where mutation slots live (ADR-159): in the repository's own `.cupel/work/slots/`, one directory per language and
// ABOUTME: slot, kept between runs, and never removed by another repository's production.
use std::fs;
use std::path::Path;
use std::sync::Barrier;

use cupel_mutate::slots::{claim, measure, slot_dir};

/// The bytes of the build a test wrote into the first Python slot of the slots in `dir`; 0 when it is gone.
fn built(dir: &Path) -> u64 {
    fs::metadata(dir.join("python/0/build.out")).map_or(0, |meta| meta.len())
}

/// Slots live in the repository's `.cupel/work/slots/`, which `.cupel/.gitignore` leaves out of version control
/// and the OS never prunes on its own schedule, as it does a temporary directory.
#[test]
fn a_slot_lives_in_the_repository_under_cupel_work_slots_by_language() {
    let repo = tempfile::tempdir().unwrap();
    assert_eq!(
        slot_dir(repo.path(), "python", 2),
        repo.path().join(".cupel/work/slots/python/2")
    );
}

#[test]
fn a_claim_makes_the_repositorys_slot_directory() {
    let repo = tempfile::tempdir().unwrap();
    let claim = claim(repo.path()).unwrap();
    assert_eq!(
        (claim.dir(), claim.dir().is_dir()),
        (repo.path().join(".cupel/work/slots").as_path(), true)
    );
    fs::create_dir_all(claim.dir().join("python/0")).unwrap();
    fs::write(claim.dir().join("python/0/build.out"), vec![b'x'; 4_096]).unwrap();
    assert_eq!(measure(claim.dir()), 4_096);
}

/// What a claim of `repo`'s slots that writes a build into its first slot found once every claim had built: `None`
/// when its build is there, else what went wrong.
fn claimed_and_built(repo: &Path, start: &Barrier, built_all: &Barrier) -> Option<String> {
    start.wait();
    let made = claim(repo).and_then(|claim| {
        let slot = claim.dir().join("python/0");
        fs::create_dir_all(&slot)?;
        fs::write(slot.join("build.out"), b"x")?;
        Ok(claim)
    });
    // Every claim is still held while each looks at its own build.
    built_all.wait();
    match made {
        Ok(claim) if built(claim.dir()) == 1 => None,
        Ok(claim) => Some(format!("{} lost its build", claim.dir().display())),
        Err(error) => Some(format!("{}: {error}", repo.display())),
    }
}

/// What `claims` claims of as many repositories' slots, made at once, lost.
fn claims_at_once(claims: usize) -> Vec<String> {
    let repos: Vec<tempfile::TempDir> = (0..claims).map(|_| tempfile::tempdir().unwrap()).collect();
    let (start, built_all) = (Barrier::new(claims), Barrier::new(claims));
    std::thread::scope(|scope| {
        let handles: Vec<_> = repos
            .iter()
            .map(|repo| scope.spawn(|| claimed_and_built(repo.path(), &start, &built_all)))
            .collect();
        handles
            .into_iter()
            .filter_map(|handle| handle.join().unwrap())
            .collect()
    })
}

/// Productions that start together each keep the slots they claimed: no claim removes a directory another claim
/// has made. Sixteen claims at once, each then writing a build into its first slot, two hundred times over.
#[test]
fn claims_made_at_once_keep_the_slots_each_made() {
    let lost: Vec<String> = (0..200).flat_map(|_| claims_at_once(16)).collect();
    assert_eq!(lost, Vec::<String>::new());
}
