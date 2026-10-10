// ABOUTME: Findings paging for `--offset`/`--limit`: cuts the envelope's findings to one page and sets
// ABOUTME: `total_findings` and `truncated`, leaving everything that describes the whole run unchanged.
use cupel_core::Envelope;

/// Which findings to keep: skip `offset`, then keep at most `limit` (ADR-019).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Page {
    pub offset: u64,
    pub limit: Option<u64>,
}

impl Page {
    /// Cuts `envelope.findings` to this page. `total_findings` becomes the number of findings
    /// before the cut, and `truncated` whether the page left any out.
    pub fn apply(self, envelope: &mut Envelope) {
        let total = envelope.findings.len();
        let start = usize::try_from(self.offset).map_or(total, |offset| offset.min(total));
        let available = total - start;
        let kept = self.limit.map_or(available, |limit| {
            usize::try_from(limit).map_or(available, |limit| limit.min(available))
        });
        envelope.findings = envelope.findings.drain(start..start + kept).collect();
        envelope.total_findings = total as u64;
        envelope.truncated = kept < total;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CupelError;
    use crate::run::Run;

    /// An envelope holding `count` findings, each distinguishable by its title.
    fn envelope(count: usize) -> Envelope {
        let run = Run::start(Vec::new());
        let mut envelope = run.failed(&CupelError::missing_verb(Vec::new()));
        let template = envelope.findings[0].clone();
        envelope.findings = (0..count)
            .map(|index| {
                let mut finding = template.clone();
                finding.title = index.to_string();
                finding
            })
            .collect();
        envelope.total_findings = count as u64;
        envelope
    }

    fn paged(count: usize, offset: u64, limit: Option<u64>) -> Envelope {
        let mut envelope = envelope(count);
        Page { offset, limit }.apply(&mut envelope);
        envelope
    }

    fn titles(envelope: &Envelope) -> Vec<String> {
        envelope.findings.iter().map(|f| f.title.clone()).collect()
    }

    #[test]
    fn no_paging_keeps_everything_and_is_not_truncated() {
        let envelope = paged(3, 0, None);
        assert_eq!(titles(&envelope), ["0", "1", "2"]);
        assert_eq!(envelope.total_findings, 3);
        assert!(!envelope.truncated);
    }

    #[test]
    fn offset_and_limit_select_a_window_in_envelope_order() {
        let envelope = paged(5, 1, Some(2));
        assert_eq!(titles(&envelope), ["1", "2"]);
        assert_eq!(envelope.total_findings, 5);
        assert!(envelope.truncated);
    }

    #[test]
    fn a_limit_ending_exactly_at_the_last_finding_is_truncated_only_by_the_offset() {
        let tail = paged(5, 3, Some(2));
        assert_eq!(titles(&tail), ["3", "4"]);
        assert!(tail.truncated, "the first three were skipped");
        let whole = paged(5, 0, Some(5));
        assert_eq!(whole.findings.len(), 5);
        assert!(
            !whole.truncated,
            "a limit equal to the total leaves nothing out"
        );
        let short = paged(5, 0, Some(4));
        assert!(short.truncated, "one finding left out");
    }

    #[test]
    fn an_offset_beyond_the_end_yields_an_empty_truncated_page() {
        let envelope = paged(3, 10, None);
        assert!(envelope.findings.is_empty());
        assert_eq!(envelope.total_findings, 3);
        assert!(envelope.truncated);
        let at_end = paged(3, 3, None);
        assert!(at_end.findings.is_empty() && at_end.truncated);
    }

    #[test]
    fn limit_zero_keeps_no_findings_but_counts_them() {
        let envelope = paged(2, 0, Some(0));
        assert!(envelope.findings.is_empty());
        assert_eq!(envelope.total_findings, 2);
        assert!(envelope.truncated);
    }

    #[test]
    fn paging_nothing_is_never_truncated() {
        for (offset, limit) in [(0, Some(0)), (4, None), (4, Some(1))] {
            let envelope = paged(0, offset, limit);
            assert_eq!(envelope.total_findings, 0);
            assert!(!envelope.truncated, "{offset} {limit:?}");
        }
    }

    #[test]
    fn paging_leaves_the_run_summary_alone() {
        let before = envelope(4);
        let after = paged(4, 1, Some(1));
        assert_eq!(after.summary, before.summary);
        assert_eq!(after.checks, before.checks);
        assert_eq!(after.next_actions, before.next_actions);
        assert_eq!(after.exit_code, before.exit_code);
    }

    #[test]
    fn huge_offsets_and_limits_do_not_overflow() {
        let envelope = paged(3, u64::MAX, Some(u64::MAX));
        assert!(envelope.findings.is_empty() && envelope.truncated);
        let all = paged(3, 0, Some(u64::MAX));
        assert_eq!(all.findings.len(), 3);
        assert!(!all.truncated);
    }
}
