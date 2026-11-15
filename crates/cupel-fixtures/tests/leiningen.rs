// ABOUTME: A Leiningen project end to end: `project.clj` without `deps.edn` is a Clojure root, and `init` writes
// ABOUTME: Leiningen's own test command and no coverage command without a declared lein-cloverage (G2, ADR-234).
use cupel_core::Status;
use cupel_fixtures::{copy_repo, run_cupel};

#[test]
fn init_makes_a_leiningen_project_a_clojure_root_tested_with_lein_test() {
    let dir = tempfile::tempdir().unwrap();
    copy_repo("leiningen/clojure", dir.path());

    let outcome = run_cupel(&["init", "--dry-run", "--format", "json"], dir.path());

    assert_eq!(outcome.code, Some(0), "{}", outcome.stdout);
    assert_eq!(outcome.envelope().status, Status::Plan);
    outcome.assert_golden("init/leiningen");
}
