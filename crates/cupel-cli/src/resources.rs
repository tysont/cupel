// ABOUTME: What a `check` or `survey` run may spend (ADR-086): its wall-clock budget from the configuration, its
// ABOUTME: `--jobs` cap on rayon's threads, and the store's recorded durations and check results for the cost model.
use std::num::NonZeroUsize;

use cupel_core::GitInfo;
use cupel_core::budget::{Budget, History, Novelty, Resources};
use cupel_core::config::Config;
use cupel_store::ScoredAt;

use crate::cli::ScopeArgs;
use crate::error::CupelError;
use crate::persist;
use crate::run::Invocation;

/// The run's budget (the effective `budget`, counted from the invocation's start), `--jobs`, and
/// the store's history; a dry run reads the store without creating it.
pub fn of(
    invocation: &Invocation<'_>,
    config: &Config,
    args: &ScopeArgs,
    git: Option<&GitInfo>,
) -> Result<Resources, CupelError> {
    let budget = config.effective.budget.map(|budget| {
        let limit = budget.as_duration();
        Budget {
            limit,
            deadline: invocation.run.deadline_after(limit),
        }
    });
    Ok(Resources {
        budget,
        jobs: args.jobs,
        history: history(invocation, git)?,
    })
}

/// The recorded durations of every task shape and each check's novelty against `head`.
fn history(invocation: &Invocation<'_>, head: Option<&GitInfo>) -> Result<History, CupelError> {
    let mut history = History::default();
    let Some(store) = persist::open_for_reading(invocation.writer)? else {
        return Ok(history);
    };
    let unusable = |error| persist::unusable(&error);
    for (shape, durations) in store.recent_durations().map_err(unusable)? {
        history.insert(shape, durations);
    }
    for (check, scored) in store.latest_scored().map_err(unusable)? {
        history.set_novelty(check, novelty(&scored, head));
    }
    Ok(history)
}

/// A check last scored at `scored` is a repeat when that was this commit with a clean tree and
/// the tree is still clean; otherwise its findings may differ (ADR-086 §4).
fn novelty(scored: &ScoredAt, head: Option<&GitInfo>) -> Novelty {
    let unchanged = head.is_some_and(|head| {
        scored.sha.as_deref() == Some(head.sha.as_str()) && !scored.dirty && !head.dirty
    });
    if unchanged {
        Novelty::Repeated
    } else {
        Novelty::Scored
    }
}

/// Caps rayon's global pool at `jobs` threads before any parallel work starts (ADR-086 §11). The
/// pool can be built once per process; a later cap in the same process is ignored.
pub fn limit_threads(jobs: Option<NonZeroUsize>) {
    if let Some(jobs) = jobs {
        let _ = rayon::ThreadPoolBuilder::new()
            .num_threads(jobs.get())
            .build_global();
    }
}

#[cfg(test)]
mod tests {
    use cupel_core::CheckId;

    use super::*;

    fn head(sha: &str, dirty: bool) -> GitInfo {
        GitInfo {
            sha: sha.to_owned(),
            dirty,
            base: None,
            branch: None,
        }
    }

    fn scored(sha: &str, dirty: bool) -> ScoredAt {
        ScoredAt {
            sha: Some(sha.to_owned()),
            dirty,
        }
    }

    #[test]
    fn a_result_from_this_clean_commit_is_a_repeat() {
        assert_eq!(
            novelty(&scored("a1", false), Some(&head("a1", false))),
            Novelty::Repeated
        );
    }

    #[test]
    fn another_commit_or_a_dirty_tree_on_either_side_may_change_the_findings() {
        for (then, now) in [
            (scored("a1", false), head("b2", false)),
            (scored("a1", true), head("a1", false)),
            (scored("a1", false), head("a1", true)),
        ] {
            assert_eq!(novelty(&then, Some(&now)), Novelty::Scored);
        }
        assert_eq!(novelty(&scored("a1", false), None), Novelty::Scored);
    }

    #[test]
    fn a_check_the_store_does_not_name_stays_unscored() {
        assert_eq!(
            History::default().novelty(&CheckId::from("tests.mutation")),
            Novelty::Unscored
        );
    }
}
